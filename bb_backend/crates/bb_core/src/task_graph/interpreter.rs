//! Task Graph workflow interpreter — facade module.
//!
//! This module provides the public API for executing task graph runs.
//! The actual implementation is delegated to:
//! - `coordinator.rs` — Graph Coordinator (control plane, single-threaded)
//! - `executor.rs` — Task Executor (execution plane, parallel)
//! - `node_exec.rs` — Per-node execution logic (returns NodeOutcome)
//!
//! This file retains the original public interface for backward compatibility.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;

use crate::agents_registry::McpServerConfig;

use super::coordinator::GraphCoordinator;
use super::node_exec::{build_edge_map, resolve_next_nodes};
use super::run_state::{self, NodeRunStatus, RunStatus, TaskGraphRunNode};
use super::types::TaskGraphError;

// ─── Public interpreter options ──────────────────────────────────────────────

/// Options for the interpreter execution.
#[derive(Debug, Clone)]
pub struct InterpreterOptions {
    /// Workspace root path.
    pub workspace_root: std::path::PathBuf,
    /// Project name.
    pub project: String,
    /// Run ID being executed.
    pub run_id: String,
    /// Path to codex binary.
    pub codex_path: String,
    /// Path to CodeBuddy binary.
    pub codebuddy_path: String,
    /// Path to opencode binary.
    pub opencode_path: String,
    /// Optional OpenCode config content injected for child runs.
    pub opencode_config_content: Option<String>,
    /// Optional model override.
    pub model: Option<String>,
    /// Timeout for a single node execution.
    pub node_timeout: Duration,
    /// Timeout for the entire run. If exceeded, run is marked as failed.
    pub run_timeout: Duration,
    /// Dry-run mode: don't actually execute runtimes.
    pub dry_run: bool,
    /// Extra environment variables applied to runtime processes.
    pub custom_env: BTreeMap<String, String>,
    /// Extra CLI arguments appended before the prompt.
    pub custom_args: Vec<String>,
    /// Extra MCP servers available to LLM-mode nodes.
    pub mcp_servers: Vec<McpServerConfig>,
    /// Skills available to LLM-mode nodes.
    pub skills: Vec<String>,
}

// ─── Interpreter result ──────────────────────────────────────────────────────

/// Result of a single interpreter step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    /// Advanced to next node(s). Carries the resolved next node IDs.
    Advanced(Vec<String>),
    /// Run completed (reached end node).
    Completed(String),
    /// Run paused at a human gate.
    Paused(String),
    /// A node failed.
    Failed(String, String),
}

/// Final outcome of a full run execution.
#[derive(Debug, Clone)]
pub enum RunOutcome {
    Succeeded,
    Failed { node_id: String, message: String },
    Paused { node_id: String },
    Cancelled,
}

// ─── Main entry point ────────────────────────────────────────────────────────

/// Execute a task graph run to completion or pause.
///
/// This is the main entry point. It creates a `GraphCoordinator` and delegates
/// the entire execution to the coordinator's main loop.
///
/// **Fast-fail**: 如果 Coordinator 内部返回 Err，本函数会先将 run 标记为 failed
/// 再传播错误，避免僵尸 run。
pub fn execute_run(opts: &InterpreterOptions) -> Result<RunOutcome, TaskGraphError> {
    let result = GraphCoordinator::load(opts).and_then(|mut c| c.run());
    if let Err(ref e) = result {
        // Fast-fail: 确保 run 被标记为 failed
        eprintln!(
            "[fast-fail] execute_run error for run {}: {}",
            opts.run_id, e
        );
        let _ = run_state::update_run_status(
            &opts.workspace_root,
            &opts.project,
            &opts.run_id,
            RunStatus::Failed,
        );
    }
    result
}

/// Resume a paused run (after human gate approval).
pub fn resume_run(
    opts: &InterpreterOptions,
    action_id: &str,
) -> Result<RunOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;

    let run = run_state::read_run(ws, project, run_id)?;

    if run.status != RunStatus::Paused {
        return Err(TaskGraphError::InvalidRunTransition {
            from: format!("{:?}", run.status).to_lowercase(),
            to: "running".to_string(),
        });
    }

    let Some(ref paused) = run.paused else {
        return Err(TaskGraphError::InvalidRunTransition {
            from: "paused".to_string(),
            to: "running (no paused metadata)".to_string(),
        });
    };

    // Validate the action
    let action = paused
        .actions
        .iter()
        .find(|a| a.id == action_id)
        .ok_or_else(|| TaskGraphError::InvalidRunTransition {
            from: "paused".to_string(),
            to: format!("running (action '{}' not found)", action_id),
        })?;

    let gate_node_id = paused.node_id.clone();
    let action_result = action.result.clone();

    // If action result is "cancel", cancel the run
    if action_result == "cancel" {
        run_state::update_run_status(ws, project, run_id, RunStatus::Cancelled)?;

        // Mark the gate node as skipped
        let now = Utc::now().to_rfc3339();
        let gate_state = TaskGraphRunNode {
            node_id: gate_node_id.clone(),
            status: NodeRunStatus::Skipped,
            started_at: None,
            completed_at: Some(now),
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Human gate rejected: action={}", action_id)),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        };
        run_state::update_node_state(ws, project, run_id, &gate_state)?;

        return Ok(RunOutcome::Cancelled);
    }

    // Resume: transition back to running
    run_state::update_run_status(ws, project, run_id, RunStatus::Running)?;

    // Mark gate node as succeeded
    let now = Utc::now().to_rfc3339();
    let gate_state = TaskGraphRunNode {
        node_id: gate_node_id.clone(),
        status: NodeRunStatus::Succeeded,
        started_at: None,
        completed_at: Some(now),
        duration_ms: None,
        iteration: None,
        exit_code: None,
        error: None,
        output_artifact: None,
        log_tail: Some(format!("Human gate approved: action={}", action_id)),
        child_run_id: None,
        runtime: None,
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    };
    run_state::update_node_state(ws, project, run_id, &gate_state)?;

    // Set node output for the gate
    run_state::set_node_output(
        ws,
        project,
        run_id,
        &gate_node_id,
        serde_json::json!({ "action": action_id, "result": action_result }),
    )?;

    // Resolve next edges from the gate node and advance cursor
    let detail = run_state::read_run_detail(ws, project, run_id)?;
    let graph = &detail.graph_snapshot;
    let edge_map = build_edge_map(&graph.edges);

    let Some(gate_node) = graph.nodes.iter().find(|node| node.id == gate_node_id) else {
        run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
        return Ok(RunOutcome::Failed {
            node_id: gate_node_id,
            message: "Human gate node not found in graph snapshot".to_string(),
        });
    };
    let next_ids = resolve_next_nodes(
        &edge_map,
        &gate_node_id,
        gate_node,
        &detail.run.context,
        None,
    )?;

    if next_ids.is_empty() {
        run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
        return Ok(RunOutcome::Failed {
            node_id: gate_node_id,
            message: "Human gate has no outgoing edges".to_string(),
        });
    }

    // 合并 HumanGate 的 next nodes 与 cursor 中其他并行分支的节点
    let current_run = run_state::read_run(ws, project, run_id)?;
    let mut new_cursor = Vec::new();
    for node_id in &current_run.cursor {
        if node_id != &gate_node_id && !new_cursor.contains(node_id) {
            new_cursor.push(node_id.clone());
        }
    }
    for n in &next_ids {
        if !new_cursor.contains(n) {
            new_cursor.push(n.clone());
        }
    }
    run_state::update_cursor(ws, project, run_id, new_cursor)?;

    // Continue execution via Coordinator
    let result = execute_run(opts);
    if let Err(ref e) = result {
        // Fast-fail: resume 失败时也确保 run 被标记为 failed
        eprintln!(
            "[fast-fail] resume_run error for run {}: {}",
            opts.run_id, e
        );
        let _ = run_state::update_run_status(ws, project, run_id, RunStatus::Failed);
    }
    result
}
