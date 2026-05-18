//! Task Graph storage layer — System Graph registry (read-only) + Project Graph store (CRUD).

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::task_graph::validation::validate_graph_source;

use super::types::{
    TaskGraphCatalog, TaskGraphCatalogEntry, TaskGraphCatalogGroup, TaskGraphCatalogGroupKind,
    TaskGraphCatalogIndex, TaskGraphDefinition, TaskGraphError, TaskGraphScope, TaskGraphSummary,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn system_graphs_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("task_graphs").join("system")
}

fn project_graphs_dir(workspace_root: &Path, project: &str) -> PathBuf {
    project_root(workspace_root, project).join("task_graphs")
}

fn project_root(workspace_root: &Path, project: &str) -> PathBuf {
    workspace_root.join("projects").join(project)
}

fn graph_catalog_path(workspace_root: &Path, project: &str) -> PathBuf {
    project_root(workspace_root, project).join("__graphs__.json")
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

fn file_updated_at(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(DateTime::<Utc>::from)
        .map_or_else(|_| Utc::now().to_rfc3339(), |date| date.to_rfc3339())
}

fn summarize(def: &TaskGraphDefinition, path: &Path) -> TaskGraphSummary {
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
        updated_at: file_updated_at(path),
        group_id: None,
        favorite: false,
        sort_order: 0,
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
        updated_at: file_updated_at(path),
        group_id: None,
        favorite: false,
        sort_order: 0,
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
                Ok(def) => results.push(summarize(&def, &path)),
                Err(e) => {
                    eprintln!("bb: compile error in system graph {path:?}: {e}");
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
    let path = system_graphs_dir(workspace_root).join(format!("{graph_id}.json"));
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
                Ok(def) => results.push(summarize(&def, &path)),
                Err(e) => {
                    eprintln!("bb: compile error in project graph {path:?}: {e}");
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

const GRAPH_CATALOG_SCHEMA_VERSION: u32 = 1;
const DEFAULT_SYSTEM_GROUP_ID: &str = "system";
const DEFAULT_PROJECT_GROUP_ID: &str = "project-ungrouped";

fn default_groups() -> Vec<TaskGraphCatalogGroup> {
    vec![
        TaskGraphCatalogGroup {
            id: DEFAULT_SYSTEM_GROUP_ID.to_string(),
            title: "System".to_string(),
            kind: TaskGraphCatalogGroupKind::System,
            sort_order: 0,
        },
        TaskGraphCatalogGroup {
            id: DEFAULT_PROJECT_GROUP_ID.to_string(),
            title: "未分组".to_string(),
            kind: TaskGraphCatalogGroupKind::Project,
            sort_order: 0,
        },
    ]
}

fn default_catalog_index() -> TaskGraphCatalogIndex {
    TaskGraphCatalogIndex {
        schema_version: GRAPH_CATALOG_SCHEMA_VERSION,
        groups: default_groups(),
        entries: Vec::new(),
    }
}

fn read_graph_catalog_index(
    workspace_root: &Path,
    project: &str,
) -> Result<TaskGraphCatalogIndex, TaskGraphError> {
    let path = graph_catalog_path(workspace_root, project);
    if !path.exists() {
        return Ok(default_catalog_index());
    }
    let contents = fs::read_to_string(&path).map_err(|source| TaskGraphError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| TaskGraphError::Parse { path, source })
}

fn write_graph_catalog_index(
    workspace_root: &Path,
    project: &str,
    index: &TaskGraphCatalogIndex,
) -> Result<(), TaskGraphError> {
    let path = graph_catalog_path(workspace_root, project);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json =
        serde_json::to_string_pretty(index).expect("TaskGraphCatalogIndex is always serializable");
    fs::write(&path, json.as_bytes()).map_err(|source| TaskGraphError::Io { path, source })
}

fn graph_key(scope: TaskGraphScope, id: &str) -> String {
    let scope = match scope {
        TaskGraphScope::System => "system",
        TaskGraphScope::Project => "project",
    };
    format!("{scope}:{id}")
}

fn default_group_for_scope(scope: TaskGraphScope) -> String {
    match scope {
        TaskGraphScope::System => DEFAULT_SYSTEM_GROUP_ID.to_string(),
        TaskGraphScope::Project => DEFAULT_PROJECT_GROUP_ID.to_string(),
    }
}

const fn kind_for_scope(scope: TaskGraphScope) -> TaskGraphCatalogGroupKind {
    match scope {
        TaskGraphScope::System => TaskGraphCatalogGroupKind::System,
        TaskGraphScope::Project => TaskGraphCatalogGroupKind::Project,
    }
}

const fn scope_sort_key(scope: TaskGraphScope) -> u8 {
    match scope {
        TaskGraphScope::System => 0,
        TaskGraphScope::Project => 1,
    }
}

fn normalize_catalog_index(
    mut index: TaskGraphCatalogIndex,
    graphs: &[TaskGraphSummary],
) -> TaskGraphCatalogIndex {
    index.schema_version = GRAPH_CATALOG_SCHEMA_VERSION;

    let mut groups_by_id = BTreeMap::<String, TaskGraphCatalogGroup>::new();
    for group in default_groups().into_iter().chain(index.groups.into_iter()) {
        let id = group.id.trim();
        if id.is_empty() {
            continue;
        }
        groups_by_id.insert(
            id.to_string(),
            TaskGraphCatalogGroup {
                id: id.to_string(),
                title: if group.title.trim().is_empty() {
                    id.to_string()
                } else {
                    group.title.trim().to_string()
                },
                kind: group.kind,
                sort_order: group.sort_order,
            },
        );
    }
    let groups: Vec<_> = groups_by_id.into_values().collect();
    let group_kind_by_id: HashMap<_, _> = groups
        .iter()
        .map(|group| (group.id.clone(), group.kind))
        .collect();

    let valid_graph_keys: HashSet<_> = graphs
        .iter()
        .map(|graph| graph_key(graph.scope, &graph.id))
        .collect();
    let mut entries_by_key = HashMap::<String, TaskGraphCatalogEntry>::new();
    for entry in index.entries {
        let key = graph_key(entry.scope, &entry.id);
        if valid_graph_keys.contains(&key) {
            entries_by_key.insert(key, entry);
        }
    }

    let mut entries = Vec::new();
    for (order, graph) in graphs.iter().enumerate() {
        let key = graph_key(graph.scope, &graph.id);
        let mut entry = entries_by_key
            .remove(&key)
            .unwrap_or_else(|| TaskGraphCatalogEntry {
                scope: graph.scope,
                id: graph.id.clone(),
                group_id: Some(default_group_for_scope(graph.scope)),
                favorite: false,
                sort_order: order as i64,
            });
        entry.scope = graph.scope;
        entry.id = graph.id.clone();
        let target_kind = kind_for_scope(graph.scope);
        let group_ok = entry
            .group_id
            .as_ref()
            .and_then(|id| group_kind_by_id.get(id))
            .is_some_and(|kind| *kind == target_kind);
        if !group_ok {
            entry.group_id = Some(default_group_for_scope(graph.scope));
        }
        entries.push(entry);
    }

    entries.sort_by(|a, b| {
        scope_sort_key(a.scope)
            .cmp(&scope_sort_key(b.scope))
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.id.cmp(&b.id))
    });

    TaskGraphCatalogIndex {
        schema_version: GRAPH_CATALOG_SCHEMA_VERSION,
        groups,
        entries,
    }
}

fn apply_catalog_metadata(graphs: &mut [TaskGraphSummary], index: &TaskGraphCatalogIndex) {
    let entries_by_key: HashMap<_, _> = index
        .entries
        .iter()
        .map(|entry| (graph_key(entry.scope, &entry.id), entry))
        .collect();
    for graph in graphs {
        if let Some(entry) = entries_by_key.get(&graph_key(graph.scope, &graph.id)) {
            graph.group_id = entry.group_id.clone();
            graph.favorite = entry.favorite;
            graph.sort_order = entry.sort_order;
        }
    }
}

pub fn list_graph_catalog(
    workspace_root: &Path,
    project: &str,
) -> Result<TaskGraphCatalog, TaskGraphError> {
    let mut graphs = list_system_graphs(workspace_root)?;
    let mut project_graphs = list_project_graphs(workspace_root, project)?;
    graphs.append(&mut project_graphs);
    let index =
        normalize_catalog_index(read_graph_catalog_index(workspace_root, project)?, &graphs);
    write_graph_catalog_index(workspace_root, project, &index)?;
    apply_catalog_metadata(&mut graphs, &index);
    graphs.sort_by(|a, b| {
        scope_sort_key(a.scope)
            .cmp(&scope_sort_key(b.scope))
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(TaskGraphCatalog {
        graphs,
        groups: index.groups,
    })
}

pub fn save_graph_catalog_index(
    workspace_root: &Path,
    project: &str,
    index: TaskGraphCatalogIndex,
) -> Result<TaskGraphCatalog, TaskGraphError> {
    let mut graphs = list_system_graphs(workspace_root)?;
    let mut project_graphs = list_project_graphs(workspace_root, project)?;
    graphs.append(&mut project_graphs);
    let index = normalize_catalog_index(index, &graphs);
    write_graph_catalog_index(workspace_root, project, &index)?;
    apply_catalog_metadata(&mut graphs, &index);
    graphs.sort_by(|a, b| {
        scope_sort_key(a.scope)
            .cmp(&scope_sort_key(b.scope))
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(TaskGraphCatalog {
        graphs,
        groups: index.groups,
    })
}

/// Read a single project graph by id.
pub fn read_project_graph(
    workspace_root: &Path,
    project: &str,
    graph_id: &str,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    let path = project_graphs_dir(workspace_root, project).join(format!("{graph_id}.json"));
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
        return Err(TaskGraphError::ReadonlyGraph(def.id));
    }

    let dir = project_graphs_dir(workspace_root, project);
    let path = dir.join(format!("{}.json", &def.id));

    if let Some(expected) = expected_version {
        // Update path: file must exist and version must match
        if !path.exists() {
            return Err(TaskGraphError::NotFound {
                scope: "project".to_string(),
                id: def.id,
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
            return Err(TaskGraphError::DuplicateGraphId(def.id));
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
                id: def.id,
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
            return Err(TaskGraphError::DuplicateGraphId(def.id));
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
    let path = project_graphs_dir(workspace_root, project).join(format!("{graph_id}.json"));
    if !path.exists() {
        return Err(TaskGraphError::NotFound {
            scope: "project".to_string(),
            id: graph_id.to_string(),
        });
    }
    fs::remove_file(&path).map_err(|source| TaskGraphError::Io { path, source })?;
    Ok(())
}
