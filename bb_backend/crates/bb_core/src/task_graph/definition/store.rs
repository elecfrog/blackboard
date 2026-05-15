//! Task Graph storage layer — System Graph registry (read-only) + Project Graph store (CRUD).

use std::fs;
use std::path::{Path, PathBuf};

use crate::task_graph::validation::validate_graph_source;

use super::types::*;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn system_graphs_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("task_graphs").join("system")
}

fn project_graphs_dir(workspace_root: &Path, project: &str) -> PathBuf {
    workspace_root
        .join("projects")
        .join(project)
        .join("task_graphs")
}

fn read_graph_file(path: &Path) -> Result<TaskGraphDefinition, TaskGraphError> {
    let contents = fs::read_to_string(path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    validate_graph_source(contents.as_bytes()).map_err(|errors| TaskGraphError::ValidationFailed {
        count: errors.len(),
        errors,
    })
}

fn write_graph_file(path: &Path, def: &TaskGraphDefinition) -> Result<(), TaskGraphError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json =
        serde_json::to_string_pretty(def).expect("TaskGraphDefinition is always serializable");
    fs::write(path, json.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn summarize(def: &TaskGraphDefinition) -> TaskGraphSummary {
    let source = match def.scope {
        TaskGraphScope::System => "builtin".to_string(),
        TaskGraphScope::Project => "project".to_string(),
    };
    TaskGraphSummary {
        scope: def.scope,
        id: def.id.clone(),
        title: def.title.clone(),
        description: def.description.clone(),
        version: def.version,
        readonly: def.readonly,
        source,
        origin: def.origin.clone(),
        node_count: def.nodes.len(),
        edge_count: def.edges.len(),
        compile_error: None,
    }
}

/// Build a placeholder summary for a graph file that failed to parse.
fn error_summary(path: &Path, scope: TaskGraphScope, error: &str) -> TaskGraphSummary {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let source = match scope {
        TaskGraphScope::System => "builtin".to_string(),
        TaskGraphScope::Project => "project".to_string(),
    };
    TaskGraphSummary {
        scope,
        id: id.clone(),
        title: id,
        description: None,
        version: 0,
        readonly: true,
        source,
        origin: None,
        node_count: 0,
        edge_count: 0,
        compile_error: Some(error.to_string()),
    }
}

// ─── System Graph Registry (read-only) ──────────────────────────────────────

/// List all system graphs available in `{workspace_root}/task_graphs/system/`.
pub fn list_system_graphs(workspace_root: &Path) -> Result<Vec<TaskGraphSummary>, TaskGraphError> {
    let dir = system_graphs_dir(workspace_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|source| TaskGraphError::Io {
        path: dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| TaskGraphError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match read_graph_file(&path) {
                Ok(def) => results.push(summarize(&def)),
                Err(e) => {
                    eprintln!("bb: compile error in system graph {:?}: {}", path, e);
                    results.push(error_summary(&path, TaskGraphScope::System, &e.to_string()));
                }
            }
        }
    }
    results.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(results)
}

/// Read a single system graph by id.
pub fn read_system_graph(
    workspace_root: &Path,
    graph_id: &str,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    let path = system_graphs_dir(workspace_root).join(format!("{}.json", graph_id));
    if !path.exists() {
        return Err(TaskGraphError::NotFound {
            scope: "system".to_string(),
            id: graph_id.to_string(),
        });
    }
    read_graph_file(&path)
}

// ─── Project Graph Store (CRUD) ─────────────────────────────────────────────

/// List all project graphs for a given project.
pub fn list_project_graphs(
    workspace_root: &Path,
    project: &str,
) -> Result<Vec<TaskGraphSummary>, TaskGraphError> {
    let dir = project_graphs_dir(workspace_root, project);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|source| TaskGraphError::Io {
        path: dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| TaskGraphError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match read_graph_file(&path) {
                Ok(def) => results.push(summarize(&def)),
                Err(e) => {
                    eprintln!("bb: compile error in project graph {:?}: {}", path, e);
                    results.push(error_summary(
                        &path,
                        TaskGraphScope::Project,
                        &e.to_string(),
                    ));
                }
            }
        }
    }
    results.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(results)
}

/// Read a single project graph by id.
pub fn read_project_graph(
    workspace_root: &Path,
    project: &str,
    graph_id: &str,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    let path = project_graphs_dir(workspace_root, project).join(format!("{}.json", graph_id));
    if !path.exists() {
        return Err(TaskGraphError::NotFound {
            scope: "project".to_string(),
            id: graph_id.to_string(),
        });
    }
    read_graph_file(&path)
}

/// Save (create or update) a project graph.
///
/// - If creating (no existing file), version must be 1.
/// - If updating, `expected_version` must match the on-disk version; the saved version is incremented.
///
/// Returns the saved definition (with updated version).
pub fn save_project_graph(
    workspace_root: &Path,
    project: &str,
    mut def: TaskGraphDefinition,
    expected_version: Option<u32>,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    if def.scope == TaskGraphScope::System {
        return Err(TaskGraphError::ReadonlyGraph(def.id.clone()));
    }

    let dir = project_graphs_dir(workspace_root, project);
    let path = dir.join(format!("{}.json", &def.id));

    if let Some(expected) = expected_version {
        // Update path: file must exist and version must match
        if !path.exists() {
            return Err(TaskGraphError::NotFound {
                scope: "project".to_string(),
                id: def.id.clone(),
            });
        }
        let existing = read_graph_file(&path)?;
        if existing.version != expected {
            return Err(TaskGraphError::StaleVersion {
                expected,
                found: existing.version,
            });
        }
        def.version = existing.version + 1;
    } else {
        // Create path: file must NOT exist
        if path.exists() {
            return Err(TaskGraphError::DuplicateGraphId(def.id.clone()));
        }
        def.version = 1;
    }

    // Ensure scope and readonly are correct for project graphs
    def.scope = TaskGraphScope::Project;
    def.readonly = false;

    write_graph_file(&path, &def)?;
    Ok(def)
}

/// Save (create or update) a system graph (only allowed for the blackboard project itself).
///
/// Writes to `{workspace_root}/task_graphs/system/{graph_id}.json`.
pub fn save_system_graph(
    workspace_root: &Path,
    mut def: TaskGraphDefinition,
    expected_version: Option<u32>,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    let dir = system_graphs_dir(workspace_root);
    let path = dir.join(format!("{}.json", &def.id));

    if let Some(expected) = expected_version {
        if !path.exists() {
            return Err(TaskGraphError::NotFound {
                scope: "system".to_string(),
                id: def.id.clone(),
            });
        }
        let existing = read_graph_file(&path)?;
        if existing.version != expected {
            return Err(TaskGraphError::StaleVersion {
                expected,
                found: existing.version,
            });
        }
        def.version = existing.version + 1;
    } else {
        if path.exists() {
            return Err(TaskGraphError::DuplicateGraphId(def.id.clone()));
        }
        def.version = 1;
    }

    // 保持 system scope
    def.scope = TaskGraphScope::System;

    write_graph_file(&path, &def)?;
    Ok(def)
}

/// Delete a project graph by id.
pub fn delete_project_graph(
    workspace_root: &Path,
    project: &str,
    graph_id: &str,
) -> Result<(), TaskGraphError> {
    let path = project_graphs_dir(workspace_root, project).join(format!("{}.json", graph_id));
    if !path.exists() {
        return Err(TaskGraphError::NotFound {
            scope: "project".to_string(),
            id: graph_id.to_string(),
        });
    }
    fs::remove_file(&path).map_err(|source| TaskGraphError::Io { path, source })?;
    Ok(())
}
