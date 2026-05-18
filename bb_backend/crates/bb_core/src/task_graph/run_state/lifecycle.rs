use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};

use super::super::compiler::compile_graph_for_execution;
use super::super::pregel::initial_checkpoint;
use super::super::types::{TaskGraphDefinition, TaskGraphError};
use super::model::{
    GraphRef, NodeError, NodeRunStatus, RunContext, RunPaused, RunStatus, TaskGraphRun,
    TaskGraphRunDetail, TaskGraphRunNode, TaskGraphRunSummary,
};
use super::node_io::load_all_node_outputs;
use super::{
    compiled_snapshot_path, generate_run_id, node_state_path, read_json, run_dir, run_json_path,
    runs_root, snapshot_path, write_json,
};
use crate::task_graph::topology::GraphRevision;

// ─── CRUD Operations ─────────────────────────────────────────────────────────

#[must_use]
pub fn resolve_graph_input(
    graph_snapshot: &TaskGraphDefinition,
    input: serde_json::Value,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();

    if let Some(params) = &graph_snapshot.inputs {
        for param in params {
            merged.insert(param.id.clone(), param.default_value.clone());
        }
    }

    match input {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if !value.is_null() {
                    merged.insert(key, value);
                }
            }
        }
        serde_json::Value::Null => {}
        value => {
            merged.insert("value".to_string(), value);
        }
    }

    serde_json::Value::Object(merged)
}

/// Create a new run, freezing the graph snapshot and initializing empty context.
pub fn create_run(
    workspace_root: &Path,
    project: &str,
    graph_ref: GraphRef,
    graph_snapshot: &TaskGraphDefinition,
    input: serde_json::Value,
) -> Result<TaskGraphRun, TaskGraphError> {
    let compiled = compile_graph_for_execution(graph_snapshot)?;
    let id = generate_run_id();
    let dir = run_dir(workspace_root, project, &id);
    let input = resolve_graph_input(graph_snapshot, input);

    // Create directory structure
    fs::create_dir_all(dir.join("nodes")).map_err(|source| TaskGraphError::Io {
        path: dir.join("nodes"),
        source,
    })?;
    fs::create_dir_all(dir.join("logs")).map_err(|source| TaskGraphError::Io {
        path: dir.join("logs"),
        source,
    })?;
    fs::create_dir_all(dir.join("artifacts")).map_err(|source| TaskGraphError::Io {
        path: dir.join("artifacts"),
        source,
    })?;
    fs::create_dir_all(dir.join("checkpoints")).map_err(|source| TaskGraphError::Io {
        path: dir.join("checkpoints"),
        source,
    })?;
    fs::create_dir_all(dir.join("graph_revisions")).map_err(|source| TaskGraphError::Io {
        path: dir.join("graph_revisions"),
        source,
    })?;
    fs::create_dir_all(dir.join("mutation_batches")).map_err(|source| TaskGraphError::Io {
        path: dir.join("mutation_batches"),
        source,
    })?;

    let now = Utc::now().to_rfc3339();

    let pregel_checkpoint = initial_checkpoint(&compiled, input.clone());

    let run = TaskGraphRun {
        id: id.clone(),
        project: project.to_string(),
        graph_ref,
        status: RunStatus::Pending,
        created_at: now.clone(),
        queued_at: None,
        queue_deadline_at: None,
        started_at: None,
        updated_at: now,
        completed_at: None,
        current_superstep: 0,
        last_checkpoint_id: None,
        pregel_checkpoint: Some(pregel_checkpoint),
        current_graph_revision: 0,
        active_nodes: Vec::new(),
        paused: None,
        context: RunContext {
            input,
            node_outputs: serde_json::Map::new(),
            branch_decisions: Vec::new(),
            loop_iterations: Vec::new(),
            loop_stack: Vec::new(),
            completed_branches: std::collections::HashMap::new(),
        },
        parent_run_id: None,
        checkpoint_ns: None,
    };

    // Write run.json
    write_json(&run_json_path(&dir), &run)?;

    // Write frozen graph snapshot
    write_json(&snapshot_path(&dir), graph_snapshot)?;

    super::superstep::write_graph_revision(
        workspace_root,
        project,
        &id,
        &GraphRevision {
            revision: 0,
            graph: graph_snapshot.clone(),
            created_at: Utc::now().to_rfc3339(),
            parent_revision: None,
            mutation_batch_id: None,
        },
    )?;

    // Write compiled graph snapshot so execution has an auditable plan input.
    write_json(&compiled_snapshot_path(&dir), &compiled)?;

    // Initialize node states as idle
    for node in &graph_snapshot.nodes {
        let node_state = TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Idle,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: None,
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        };
        write_json(&node_state_path(&dir, &node.id), &node_state)?;
    }

    Ok(run)
}

/// Create a run in the durable admission queue.
pub fn create_queued_run(
    workspace_root: &Path,
    project: &str,
    graph_ref: GraphRef,
    graph_snapshot: &TaskGraphDefinition,
    input: serde_json::Value,
    queue_deadline_at: String,
) -> Result<TaskGraphRun, TaskGraphError> {
    let mut run = create_run(workspace_root, project, graph_ref, graph_snapshot, input)?;
    let now = Utc::now().to_rfc3339();
    run.status = RunStatus::Queued;
    run.queued_at = Some(now.clone());
    run.queue_deadline_at = Some(queue_deadline_at);
    run.updated_at = now;

    let dir = run_dir(workspace_root, project, &run.id);
    write_json(&run_json_path(&dir), &run)?;
    Ok(run)
}

/// List all runs for a project (summary only).
pub fn list_runs(
    workspace_root: &Path,
    project: &str,
) -> Result<Vec<TaskGraphRunSummary>, TaskGraphError> {
    let root = runs_root(workspace_root, project);
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let entries = fs::read_dir(&root).map_err(|source| TaskGraphError::Io {
        path: root.clone(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| TaskGraphError::Io {
            path: root.clone(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            let run_file = run_json_path(&path);
            if run_file.exists() {
                let run: TaskGraphRun = read_json(&run_file)?;
                results.push(TaskGraphRunSummary {
                    id: run.id,
                    project: run.project,
                    graph_ref: run.graph_ref,
                    status: run.status,
                    created_at: run.created_at,
                    queued_at: run.queued_at,
                    queue_deadline_at: run.queue_deadline_at,
                    started_at: run.started_at,
                    updated_at: run.updated_at,
                    completed_at: run.completed_at,
                    current_superstep: run.current_superstep,
                    last_checkpoint_id: run.last_checkpoint_id,
                    current_graph_revision: run.current_graph_revision,
                    active_nodes: run.active_nodes,
                    checkpoint_ns: run.checkpoint_ns,
                });
            }
        }
    }

    results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(results)
}

/// Read the full run detail (run metadata + frozen snapshot + all node states).
pub fn read_run_detail(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<TaskGraphRunDetail, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    // 从独立文件聚合 node_outputs（并行安全）
    let outputs = load_all_node_outputs(workspace_root, project, run_id)?;
    for (k, v) in outputs {
        run.context.node_outputs.entry(k).or_insert(v);
    }
    let graph_snapshot = read_current_graph_snapshot(workspace_root, project, run_id, &run)?;

    // Read all node states
    let nodes_dir = dir.join("nodes");
    let mut nodes = Vec::new();
    if nodes_dir.exists() {
        let entries = fs::read_dir(&nodes_dir).map_err(|source| TaskGraphError::Io {
            path: nodes_dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| TaskGraphError::Io {
                path: nodes_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let node: TaskGraphRunNode = read_json(&path)?;
                nodes.push(node);
            }
        }
    }
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    Ok(TaskGraphRunDetail {
        run,
        graph_snapshot,
        nodes,
    })
}

/// Read just the run metadata (without snapshot or nodes).
pub fn read_run(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    // 从独立文件聚合 node_outputs（并行安全）
    let outputs = load_all_node_outputs(workspace_root, project, run_id)?;
    for (k, v) in outputs {
        run.context.node_outputs.entry(k).or_insert(v);
    }
    Ok(run)
}

fn read_current_graph_snapshot(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    run: &TaskGraphRun,
) -> Result<TaskGraphDefinition, TaskGraphError> {
    if let Some(revision) = super::superstep::read_graph_revision(
        workspace_root,
        project,
        run_id,
        run.current_graph_revision,
    )? {
        return Ok(revision.graph);
    }
    let dir = run_dir(workspace_root, project, run_id);
    read_json(&snapshot_path(&dir))
}

/// Update run-level status with basic transition validation.
pub fn update_run_status(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    new_status: RunStatus,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;

    validate_status_transition(run.status, new_status)?;

    let now = Utc::now().to_rfc3339();
    run.status = new_status;
    run.updated_at = now.clone();

    match new_status {
        RunStatus::Running if run.started_at.is_none() => {
            run.started_at = Some(now);
        }
        RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled => {
            run.completed_at = Some(now);
            run.paused = None;
            run.active_nodes.clear();
        }
        RunStatus::Paused => {}
        _ => {}
    }

    write_json(&run_file, &run)?;
    Ok(run)
}

/// Mark a run as failed and reconcile any still-active node projections.
///
/// This is the terminal failure gate used by runner/coordinator fast-fail paths.
/// A terminal run must not keep active nodes, and active node projections must not
/// stay queued/running after the run itself is failed.
pub fn fail_run_active_nodes(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    if run.status != RunStatus::Failed {
        validate_status_transition(run.status, RunStatus::Failed)?;
    }

    let code = code.into();
    let message = message.into();
    let active_nodes = run.active_nodes.clone();
    let now = Utc::now().to_rfc3339();

    run.status = RunStatus::Failed;
    run.updated_at = now.clone();
    run.completed_at = Some(now.clone());
    run.paused = None;
    run.active_nodes.clear();
    write_json(&run_file, &run)?;

    fail_active_node_states(&dir, active_nodes, &now, &code, &message)?;
    Ok(run)
}

/// Extend a queued run's deadline without changing its original queue order.
pub fn update_queued_run_deadline(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    queue_deadline_at: String,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    if run.status != RunStatus::Queued {
        return Err(TaskGraphError::InvalidRunTransition {
            from: format!("{:?}", run.status).to_lowercase(),
            to: "queued".to_string(),
        });
    }

    run.queue_deadline_at = Some(queue_deadline_at);
    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(run)
}

/// 级联取消一个 run 及其所有活跃的子 run。
///
/// 遍历该 run 的所有 node state，找到 `child_run_id` 不为空且仍处于活跃状态的子 run，
/// 递归取消它们，最后取消自身。
pub fn cancel_run_cascade(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<TaskGraphRun, TaskGraphError> {
    // 1. 读取当前 run
    let run = read_run(workspace_root, project, run_id)?;

    // 如果已经是终态，直接返回
    if matches!(
        run.status,
        RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled
    ) {
        return Ok(run);
    }

    // 2. 扫描 nodes 目录，找到所有 child_run_id
    let nodes_dir = run_dir(workspace_root, project, run_id).join("nodes");
    if nodes_dir.exists() {
        if let Ok(entries) = fs::read_dir(&nodes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(node) = read_json::<TaskGraphRunNode>(&path) {
                        if let Some(ref child_id) = node.child_run_id {
                            // 递归取消子 run（忽略错误，尽力而为）
                            let _ = cancel_run_cascade(workspace_root, project, child_id);
                        }
                    }
                }
            }
        }
    }

    // 3. 取消自身
    update_run_status(workspace_root, project, run_id, RunStatus::Cancelled)
}

/// Write a `TaskGraphRun` back to its run.json file (for updating fields like `parent_run_id`).
pub fn write_run_json(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    run: &TaskGraphRun,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);
    write_json(&run_file, run)
}

pub(super) fn update_run_checkpoint_pointer(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    superstep: u64,
    checkpoint_id: String,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    run.current_superstep = superstep;
    run.last_checkpoint_id = Some(checkpoint_id);
    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(run)
}

/// Set the run's paused state (for human gate).
pub fn set_run_paused(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    paused: RunPaused,
) -> Result<TaskGraphRun, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    if !run_file.exists() {
        return Err(TaskGraphError::RunNotFound {
            project: project.to_string(),
            run_id: run_id.to_string(),
        });
    }

    let mut run: TaskGraphRun = read_json(&run_file)?;
    run.paused = Some(paused);
    run.status = RunStatus::Paused;
    run.updated_at = Utc::now().to_rfc3339();

    write_json(&run_file, &run)?;
    Ok(run)
}

// ─── Status transition validation ────────────────────────────────────────────

fn validate_status_transition(from: RunStatus, to: RunStatus) -> Result<(), TaskGraphError> {
    let valid = matches!(
        (from, to),
        (
            RunStatus::Queued,
            RunStatus::Pending | RunStatus::Failed | RunStatus::Cancelled
        ) | (RunStatus::Pending | RunStatus::Paused, RunStatus::Running)
            | (RunStatus::Pending | RunStatus::Running, RunStatus::Failed)
            | (
                RunStatus::Pending | RunStatus::Running | RunStatus::Paused,
                RunStatus::Cancelled
            )
            | (RunStatus::Running, RunStatus::Paused | RunStatus::Succeeded)
    );

    if !valid {
        return Err(TaskGraphError::InvalidRunTransition {
            from: format!("{from:?}").to_lowercase(),
            to: format!("{to:?}").to_lowercase(),
        });
    }
    Ok(())
}

fn fail_active_node_states(
    run_dir: &Path,
    active_nodes: Vec<String>,
    completed_at: &str,
    code: &str,
    message: &str,
) -> Result<(), TaskGraphError> {
    for node_id in active_nodes {
        let path = node_state_path(run_dir, &node_id);
        let mut node = if path.exists() {
            read_json::<TaskGraphRunNode>(&path)?
        } else {
            TaskGraphRunNode {
                node_id: node_id.clone(),
                status: NodeRunStatus::Idle,
                started_at: None,
                completed_at: None,
                duration_ms: None,
                iteration: None,
                exit_code: None,
                error: None,
                output_artifact: None,
                log_tail: None,
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            }
        };

        if !matches!(
            node.status,
            NodeRunStatus::Idle
                | NodeRunStatus::Queued
                | NodeRunStatus::Running
                | NodeRunStatus::Paused
        ) {
            continue;
        }

        if node.started_at.is_none() {
            node.started_at = Some(completed_at.to_string());
        }
        if node.completed_at.is_none() {
            node.completed_at = Some(completed_at.to_string());
        }
        if node.duration_ms.is_none() {
            node.duration_ms = node
                .started_at
                .as_deref()
                .and_then(|started_at| duration_ms_between(started_at, completed_at));
        }
        node.status = NodeRunStatus::Failed;
        node.error = Some(NodeError {
            code: code.to_string(),
            message: message.to_string(),
        });
        node.log_tail = Some(message.to_string());

        write_json(&path, &node)?;
    }

    Ok(())
}

fn duration_ms_between(started_at: &str, completed_at: &str) -> Option<u64> {
    let started = DateTime::parse_from_rfc3339(started_at).ok()?;
    let completed = DateTime::parse_from_rfc3339(completed_at).ok()?;
    let millis = completed.signed_duration_since(started).num_milliseconds();
    Some(millis.max(0) as u64)
}
