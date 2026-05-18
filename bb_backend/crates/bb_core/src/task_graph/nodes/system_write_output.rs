use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde_json::{json, Value};

use crate::fs_util::resolve_slash;
use crate::task_graph::definition::types::{
    SystemWriteOutputConfig, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::eval;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, ArtifactContentType, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};

const RUNTIME_NAME: &str = "system_write_output";

pub fn execute_system_write_output_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let started = Instant::now();
    let start_time = Utc::now().to_rfc3339();
    let config: SystemWriteOutputConfig =
        serde_json::from_value(node.config.clone()).map_err(|source| TaskGraphError::Parse {
            path: PathBuf::from(format!("node:{}", node.id)),
            source,
        })?;

    run_state::append_node_log(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node.id,
        &format!("[{start_time}] system_write_output starting"),
    )?;
    run_state::update_node_state(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node_state(
            node,
            NodeRunStatus::Running,
            start_time.clone(),
            None,
            None,
            None,
            None,
            Some("system_write_output starting".to_string()),
        ),
    )?;

    let inputs = eval::resolve_node_inputs(
        config.inputs.as_ref(),
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
    );
    let inputs_value = Value::Object(inputs);
    let output_path = match render_output_path(&config, opts, run, &inputs_value)
        .and_then(|path| workspace_scoped_output_path(&opts.workspace_root, &path))
    {
        Ok(path) => path,
        Err(message) => {
            return failed_outcome(
                opts,
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_output_path",
                message,
            );
        }
    };
    let content = match render_content(&config, opts, run, &inputs_value) {
        Ok(content) => content,
        Err(message) => {
            return failed_outcome(
                opts,
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_content",
                message,
            );
        }
    };

    if !config.overwrite && output_path.exists() {
        return failed_outcome(
            opts,
            node,
            start_time,
            started.elapsed().as_millis() as u64,
            "output_exists",
            format!(
                "output file already exists: {}",
                resolve_slash(&output_path)
            ),
        );
    }
    if config.create_parent_dirs {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }
    fs::write(&output_path, content.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: output_path.clone(),
        source,
    })?;

    let output = json!({
        "ok": true,
        "path": resolve_slash(&output_path),
        "bytes": content.len(),
        "artifact_type": config.artifact_type,
    });
    let artifact_content =
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string());
    let artifact = run_state::write_artifact(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &format!("{}-write-output", node.id),
        &artifact_content,
        ArtifactContentType::Json,
    )?;
    let end_time = Utc::now().to_rfc3339();
    let path_for_log = output
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    run_state::append_node_log(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node.id,
        &format!("[{end_time}] wrote {path_for_log}"),
    )?;

    let mut succeeded_state = node_state(
        node,
        NodeRunStatus::Succeeded,
        start_time,
        Some(end_time),
        Some(started.elapsed().as_millis() as u64),
        Some(0),
        None,
        Some(format!("wrote {path_for_log}")),
    );
    succeeded_state.output_artifact = Some(artifact);

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(output),
        node_state: succeeded_state,
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

fn render_output_path(
    config: &SystemWriteOutputConfig,
    opts: &RunnerOptions,
    run: &TaskGraphRun,
    inputs: &Value,
) -> Result<String, String> {
    if config.output_path.trim().is_empty() {
        return Err("system_write_output.output_path must not be empty".to_string());
    }
    let rendered = eval::render_prompt_template(
        &config.output_path,
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
        Some(inputs),
    );
    let trimmed = rendered.trim();
    if trimmed.is_empty() {
        Err("system_write_output.output_path rendered to an empty path".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn render_content(
    config: &SystemWriteOutputConfig,
    opts: &RunnerOptions,
    run: &TaskGraphRun,
    inputs: &Value,
) -> Result<String, String> {
    let template = if config.content.trim().is_empty() {
        "{{inputs.content}}"
    } else {
        &config.content
    };
    let rendered = eval::render_prompt_template(
        template,
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
        Some(inputs),
    );
    if rendered.is_empty() {
        Err("system_write_output content rendered to an empty string".to_string())
    } else {
        Ok(rendered)
    }
}

fn workspace_scoped_output_path(workspace_root: &Path, raw: &str) -> Result<PathBuf, String> {
    let root = workspace_root
        .canonicalize()
        .map_err(|source| format!("failed to canonicalize workspace root: {source}"))
        .map(|path| PathBuf::from(crate::fs_util::path_to_string(&path)))?;
    let requested = PathBuf::from(raw);
    let joined = if requested.is_absolute() {
        requested
    } else {
        root.join(requested)
    };
    let normalized = normalize_lexical(&joined);
    if !normalized.starts_with(&root) {
        return Err(format!(
            "output path escapes workspace root: {}",
            resolve_slash(&normalized)
        ));
    }
    Ok(normalized)
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn failed_outcome(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    start_time: String,
    duration_ms: u64,
    code: &str,
    message: String,
) -> Result<NodeOutcome, TaskGraphError> {
    run_state::append_node_log(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node.id,
        &format!("[{code}] {message}"),
    )?;
    let output = json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message,
        }
    });
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        output: Some(output),
        node_state: node_state(
            node,
            NodeRunStatus::Failed,
            start_time,
            Some(Utc::now().to_rfc3339()),
            Some(duration_ms),
            Some(1),
            Some(NodeError {
                code: code.to_string(),
                message: message.clone(),
            }),
            Some(message),
        ),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

#[allow(clippy::too_many_arguments)]
fn node_state(
    node: &TaskGraphNode,
    status: NodeRunStatus,
    started_at: String,
    completed_at: Option<String>,
    duration_ms: Option<u64>,
    exit_code: Option<i32>,
    error: Option<NodeError>,
    log_tail: Option<String>,
) -> TaskGraphRunNode {
    TaskGraphRunNode {
        node_id: node.id.clone(),
        status,
        started_at: Some(started_at),
        completed_at,
        duration_ms,
        iteration: None,
        exit_code,
        error,
        output_artifact: None,
        log_tail,
        child_run_id: None,
        runtime: Some(RUNTIME_NAME.to_string()),
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn workspace_scoped_output_path_accepts_workspace_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let raw = tmp.path().join("report.md");
        let path = workspace_scoped_output_path(tmp.path(), &raw.display().to_string()).unwrap();
        assert!(path.ends_with("report.md"));
    }

    #[test]
    fn workspace_scoped_output_path_rejects_parent_escape() {
        let tmp = TempDir::new().unwrap();
        let err = workspace_scoped_output_path(tmp.path(), "../escape.md").unwrap_err();
        assert!(err.contains("escapes workspace root"));
    }
}
