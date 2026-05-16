use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::task_graph::definition::types::TaskGraphError;
use crate::task_graph::nodes::eval;

pub(in crate::task_graph::nodes) const STAGING_DIR_NAME: &str = ".staging";

pub(in crate::task_graph::nodes) fn kb_output_dir_from_inputs(
    inputs: &eval::NodeInputs,
    workspace_root: &Path,
) -> Option<PathBuf> {
    string_input(inputs, "kb-output-dir")
        .or_else(|| string_input(inputs, "kb_output_dir"))
        .or_else(|| nested_string(inputs.get("intake"), &["kb_output_dir"]))
        .or_else(|| nested_string(inputs.get("manifest"), &["kb_output_dir"]))
        .or_else(|| nested_string(inputs.get("manifest"), &["staging", "kb_output_dir"]))
        .or_else(|| nested_string(inputs.get("wiki_plan"), &["kb_output_dir"]))
        .map(|raw| resolve_path(&raw, workspace_root))
}

pub(in crate::task_graph::nodes) fn staging_dir(kb_output_dir: &Path) -> PathBuf {
    kb_output_dir.join(STAGING_DIR_NAME)
}

pub(in crate::task_graph::nodes) fn resolve_path(raw: &str, workspace_root: &Path) -> PathBuf {
    let path = PathBuf::from(raw.trim());
    if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    }
}

pub(in crate::task_graph::nodes) fn path_string(path: &Path) -> String {
    path.display().to_string()
}

pub(in crate::task_graph::nodes) fn write_json_file(
    path: &Path,
    value: &Value,
) -> Result<(), TaskGraphError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    fs::write(path, content.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(in crate::task_graph::nodes) fn read_json_file(path: &Path) -> Result<Value, TaskGraphError> {
    let content = fs::read_to_string(path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| TaskGraphError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub(in crate::task_graph::nodes) fn string_input(
    inputs: &eval::NodeInputs,
    key: &str,
) -> Option<String> {
    inputs
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(in crate::task_graph::nodes) fn nested_string(
    value: Option<&Value>,
    path: &[&str],
) -> Option<String> {
    let mut current = value?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(in crate::task_graph::nodes) fn sanitize_filename(raw: &str) -> String {
    let mut value = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    while value.contains("--") {
        value = value.replace("--", "-");
    }
    if value.is_empty() {
        "output".to_string()
    } else {
        value
    }
}
