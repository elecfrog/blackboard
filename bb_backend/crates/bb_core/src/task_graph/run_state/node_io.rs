use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use super::super::types::TaskGraphError;
use super::model::{ArtifactContentType, OutputArtifact, TaskGraphRunNode};
use super::{
    artifacts_dir, node_log_path, node_output_path, node_state_path, read_json, run_dir, write_json,
};

/// Update a single node's run state.
pub fn update_node_state(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    state: &TaskGraphRunNode,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    write_json(&node_state_path(&dir, &state.node_id), state)
}

/// Update only a node's live log tail while preserving its execution state.
pub fn update_node_log_tail(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    log_tail: &str,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let path = node_state_path(&dir, node_id);
    let mut state: TaskGraphRunNode = read_json(&path)?;
    state.log_tail = Some(log_tail.to_string());
    write_json(&path, &state)
}

/// Append a line to a node's log file.
pub fn append_node_log(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    line: &str,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let log_path = node_log_path(&dir, node_id);

    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| TaskGraphError::Io {
            path: log_path.clone(),
            source,
        })?;

    writeln!(file, "{line}").map_err(|source| TaskGraphError::Io {
        path: log_path,
        source,
    })?;

    Ok(())
}

/// Write an artifact file and return its path relative to the run directory.
pub fn write_artifact(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    artifact_id: &str,
    content: &str,
    content_type: ArtifactContentType,
) -> Result<OutputArtifact, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let ext = match content_type {
        ArtifactContentType::Markdown => "md",
        ArtifactContentType::Json => "json",
        ArtifactContentType::Text => "txt",
    };
    let filename = format!("{artifact_id}.{ext}");
    let artifact_path = artifacts_dir(&dir).join(&filename);

    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&artifact_path, content.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: artifact_path,
        source,
    })?;

    Ok(OutputArtifact {
        id: artifact_id.to_string(),
        path: format!("artifacts/{filename}"),
        content_type,
    })
}

/// Set a node output — writes to an independent file `node_outputs/{node_id}.json`
/// so that parallel threads never contend on `run.json`.
pub fn set_node_output(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    output: serde_json::Value,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let out_file = node_output_path(&dir, node_id);
    write_json(&out_file, &output)?;
    Ok(())
}

/// Read a single node's output from its independent file.
pub fn get_node_output(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
) -> Result<Option<serde_json::Value>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let out_file = node_output_path(&dir, node_id);
    if out_file.exists() {
        let val: serde_json::Value = read_json(&out_file)?;
        Ok(Some(val))
    } else {
        Ok(None)
    }
}

/// Load all node outputs from independent files and merge them into
/// `run.context.node_outputs` (for serialisation / API responses).
pub fn load_all_node_outputs(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let outputs_dir = dir.join("node_outputs");
    let mut map = serde_json::Map::new();
    if outputs_dir.is_dir() {
        for entry in fs::read_dir(&outputs_dir).map_err(|source| TaskGraphError::Io {
            path: outputs_dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| TaskGraphError::Io {
                path: outputs_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let val: serde_json::Value = read_json(&path)?;
                    map.insert(stem.to_string(), val);
                }
            }
        }
    }
    Ok(map)
}
