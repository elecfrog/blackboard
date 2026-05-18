use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::task_graph::definition::types::{
    NodeType, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::{eval, kb_staging, runtime};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};

const RUNTIME_NAME: &str = "plan";

#[derive(Debug, Clone, Default, Deserialize)]
struct PlanConfig {
    #[serde(default)]
    inputs: Option<Value>,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    static_data: Option<Value>,
    #[serde(default)]
    output: PlanOutputConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PlanOutputConfig {
    #[serde(default)]
    artifact_path: Option<String>,
    #[serde(default)]
    schema_name: Option<String>,
}

pub fn execute_plan_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let started = Instant::now();
    let start_time = Utc::now().to_rfc3339();
    let config: PlanConfig = match serde_json::from_value(node.config.clone()) {
        Ok(config) => config,
        Err(source) => {
            return Ok(failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_config",
                format!("plan node config parse error: {source}"),
            ));
        }
    };

    run_state::append_node_log(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node.id,
        &format!("[{start_time}] plan node starting"),
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
            Some("plan node starting".to_string()),
        ),
    )?;

    let inputs = eval::resolve_node_inputs(
        config.inputs.as_ref(),
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
    );

    let plan_data = if let Some(data) = config.static_data.as_ref().or(config.data.as_ref()) {
        resolve_config_value(
            data,
            &opts.project,
            &opts.workspace_root,
            &opts.scripts_dir,
            &run.context,
        )
    } else {
        let mut llm_node = node.clone();
        llm_node.node_type = NodeType::Llm;
        let outcome = runtime::execute_llm_node(opts, &llm_node, run, edge_map)?;
        if outcome.status != NodeRunStatus::Succeeded {
            return Ok(outcome);
        }
        outcome.output.unwrap_or(Value::Null)
    };

    let artifact_path = match plan_artifact_path(opts, node, run, &config, &inputs) {
        Ok(path) => path,
        Err(message) => {
            return Ok(failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_artifact_path",
                message,
            ));
        }
    };
    kb_staging::write_json_file(&artifact_path, &plan_data)?;

    let mut output = Map::new();
    output.insert("data".to_string(), plan_data.clone());
    output.insert(
        "artifact_path".to_string(),
        Value::String(kb_staging::path_string(&artifact_path)),
    );
    output.insert(
        "artifact_type".to_string(),
        Value::String("json".to_string()),
    );
    if let Some(schema_name) = config.output.schema_name.as_deref() {
        output.insert(
            "schema_name".to_string(),
            Value::String(schema_name.to_string()),
        );
    }
    if let Some(summary) = summary_from_plan(&plan_data) {
        output.insert("summary".to_string(), Value::String(summary));
    }

    let output_value = Value::Object(output);
    run_state::append_node_log(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        &node.id,
        &format!(
            "[{}] plan node wrote artifact_path={}",
            Utc::now().to_rfc3339(),
            kb_staging::path_string(&artifact_path)
        ),
    )?;

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(output_value),
        node_state: node_state(
            node,
            NodeRunStatus::Succeeded,
            start_time,
            Some(Utc::now().to_rfc3339()),
            Some(started.elapsed().as_millis() as u64),
            Some(0),
            None,
            Some(format!("wrote {}", kb_staging::path_string(&artifact_path))),
        ),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

fn plan_artifact_path(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    config: &PlanConfig,
    inputs: &eval::NodeInputs,
) -> Result<PathBuf, String> {
    if let Some(raw) = config
        .output
        .artifact_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let resolved = resolve_config_value(
            &Value::String(raw.to_string()),
            &opts.project,
            &opts.workspace_root,
            &opts.scripts_dir,
            &run.context,
        );
        let Some(path) = resolved
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err("plan output.artifact_path resolved to an empty value".to_string());
        };
        return Ok(kb_staging::resolve_path(path, &opts.workspace_root));
    }

    if let Some(kb_output_dir) = kb_staging::kb_output_dir_from_inputs(inputs, &opts.workspace_root)
    {
        return Ok(kb_staging::staging_dir(&kb_output_dir)
            .join(format!("{}.json", kb_staging::sanitize_filename(&node.id))));
    }

    Ok(opts
        .workspace_root
        .join(".staging")
        .join("task-graph-runs")
        .join(&opts.run_id)
        .join(format!("{}.json", kb_staging::sanitize_filename(&node.id))))
}

fn resolve_config_value(
    value: &Value,
    project: &str,
    root: &std::path::Path,
    scripts_dir: &std::path::Path,
    context: &crate::task_graph::run_state::RunContext,
) -> Value {
    let mut binding = Map::new();
    binding.insert("value".to_string(), value.clone());
    eval::resolve_node_inputs(
        Some(&Value::Object(binding)),
        project,
        root,
        scripts_dir,
        context,
    )
    .remove("value")
    .unwrap_or(Value::Null)
}

fn summary_from_plan(value: &Value) -> Option<String> {
    value
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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

fn failed_outcome(
    node: &TaskGraphNode,
    start_time: String,
    duration_ms: u64,
    code: impl Into<String>,
    message: impl Into<String>,
) -> NodeOutcome {
    let message = message.into();
    NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        output: None,
        node_state: node_state(
            node,
            NodeRunStatus::Failed,
            start_time,
            Some(Utc::now().to_rfc3339()),
            Some(duration_ms),
            Some(1),
            Some(NodeError {
                code: code.into(),
                message: message.clone(),
            }),
            Some(message),
        ),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::time::Duration;

    use serde_json::{json, Map};
    use tempfile::TempDir;

    use super::*;
    use crate::task_graph::definition::types::{NodePin, TaskGraphScope};
    use crate::task_graph::run_state::{GraphRef, RunContext, RunStatus};

    #[test]
    fn static_plan_writes_json_artifact_and_returns_path() {
        let tmp = TempDir::new().unwrap();
        let artifact_path = tmp.path().join(".staging/wiki-plan.json");
        let opts = RunnerOptions {
            workspace_root: tmp.path().to_path_buf(),
            scripts_dir: tmp.path().join("scripts"),
            project: "blackboard".to_string(),
            run_id: "run-plan".to_string(),
            codex_path: "codex".to_string(),
            codebuddy_path: "codebuddy".to_string(),
            opencode_path: "opencode".to_string(),
            opencode_config_content: None,
            pi_path: "pi".to_string(),
            model: None,
            node_timeout: Duration::from_secs(1),
            run_timeout: Duration::from_secs(1),
            dry_run: true,
            custom_env: BTreeMap::new(),
            custom_args: vec![],
            mcp_servers: vec![],
            skills: vec![],
        };
        let run = TaskGraphRun {
            id: "run-plan".to_string(),
            project: "blackboard".to_string(),
            graph_ref: GraphRef {
                scope: TaskGraphScope::Project,
                id: "graph".to_string(),
                version: 1,
            },
            status: RunStatus::Running,
            created_at: "2026-05-17T00:00:00Z".to_string(),
            queued_at: None,
            queue_deadline_at: None,
            started_at: None,
            updated_at: "2026-05-17T00:00:00Z".to_string(),
            completed_at: None,
            current_superstep: 0,
            last_checkpoint_id: None,
            pregel_checkpoint: None,
            current_graph_revision: 0,
            active_nodes: vec!["plan".to_string()],
            paused: None,
            context: RunContext {
                input: json!({}),
                node_outputs: Map::new(),
                branch_decisions: vec![],
                loop_iterations: vec![],
                loop_stack: vec![],
                completed_branches: HashMap::new(),
            },
            parent_run_id: None,
            checkpoint_ns: None,
        };
        let node = TaskGraphNode {
            id: "plan".to_string(),
            node_type: NodeType::Plan,
            label: "Plan".to_string(),
            description: None,
            position: None,
            config: json!({
                "data": { "summary": "ok", "writer_plan": { "writers": [] } },
                "output": {
                    "schema_name": "writer_plan",
                    "artifact_path": artifact_path.display().to_string()
                }
            }),
            pins: Vec::<NodePin>::new(),
        };

        let outcome = execute_plan_node(&opts, &node, &run, &HashMap::new()).unwrap();

        assert_eq!(outcome.status, NodeRunStatus::Succeeded);
        assert_eq!(
            outcome.output.as_ref().unwrap()["artifact_path"],
            artifact_path.display().to_string()
        );
        let file_value: Value =
            serde_json::from_str(&std::fs::read_to_string(&artifact_path).unwrap()).unwrap();
        assert_eq!(file_value["summary"], "ok");
    }
}
