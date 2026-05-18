//! `TaskGraph` Pregel run entrypoint.
//!
//! This module provides the public run/resume API around `PregelLoop`.
//! The implementation is delegated to:
//! - `coordinator.rs` — Graph Coordinator (control plane, single-threaded)
//! - `executor.rs` — Task Executor (execution plane, parallel)
//! - `nodes/` — Per-node execution logic (returns `NodeOutcome`)
//!
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;

use crate::agents_registry::McpServerConfig;

use super::coordinator::GraphCoordinator;
use crate::task_graph::compile::compiler::compile_graph_for_execution;
use crate::task_graph::definition::types::TaskGraphError;
use crate::task_graph::pregel::{
    apply_writes as apply_pregel_writes, initial_checkpoint as initial_pregel_checkpoint,
    resume_write, writes_from_node_outcome, PregelTask, PregelTaskKind,
};
use crate::task_graph::run_state::{self, NodeRunStatus, RunStatus, TaskGraphRunNode};

// ─── Public runner options ───────────────────────────────────────────────────

/// Options for `TaskGraph` runner execution.
#[derive(Debug, Clone)]
pub struct RunnerOptions {
    /// Workspace root path.
    pub workspace_root: PathBuf,
    /// Directory containing Blackboard runtime scripts.
    pub scripts_dir: PathBuf,
    /// Project name.
    pub project: String,
    /// Run ID being executed.
    pub run_id: String,
    /// Path to codex binary.
    pub codex_path: String,
    /// Path to `CodeBuddy` binary.
    pub codebuddy_path: String,
    /// Path to opencode binary.
    pub opencode_path: String,
    /// Optional `OpenCode` config content injected for child runs.
    pub opencode_config_content: Option<String>,
    /// Path to Pi binary.
    pub pi_path: String,
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

/// Resolve the scripts directory for `TaskGraph` agent sessions.
///
/// Source checkouts commonly execute with `.bb_template` as the workspace root
/// while scripts live at the repository root. Packaged/user workspaces keep
/// copied scripts under the writable workspace root.
#[must_use]
pub fn resolve_scripts_dir(workspace_root: &Path) -> PathBuf {
    let workspace_scripts = workspace_root.join("scripts");
    if workspace_scripts.join("check_ticket_ids.py").is_file() {
        return workspace_scripts;
    }

    if workspace_root.file_name().and_then(|name| name.to_str()) == Some(".bb_template") {
        if let Some(repo_root) = workspace_root.parent() {
            let repo_scripts = repo_root.join("scripts");
            if repo_scripts.join("check_ticket_ids.py").is_file() {
                return repo_scripts;
            }
        }
    }

    workspace_scripts
}

#[cfg(test)]
mod tests {
    use super::resolve_scripts_dir;

    #[test]
    fn resolve_scripts_dir_prefers_workspace_scripts() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().join(".bb");
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/check_ticket_ids.py"), "").unwrap();

        assert_eq!(resolve_scripts_dir(&root), root.join("scripts"));
    }

    #[test]
    fn resolve_scripts_dir_uses_repo_scripts_for_template_workspace() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().join(".bb_template");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(temp.path().join("scripts")).unwrap();
        std::fs::write(temp.path().join("scripts/check_ticket_ids.py"), "").unwrap();

        assert_eq!(resolve_scripts_dir(&root), temp.path().join("scripts"));
    }
}

// ─── Runner result ───────────────────────────────────────────────────────────

/// Result of a single runner step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerStepResult {
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
pub fn execute_run(opts: &RunnerOptions) -> Result<RunOutcome, TaskGraphError> {
    let result = GraphCoordinator::load(opts).and_then(|mut c| c.run());
    if let Err(ref e) = result {
        // Fast-fail: ensure terminal run/node projections stay consistent.
        eprintln!(
            "[fast-fail] execute_run error for run {}: {}",
            opts.run_id, e
        );
        fail_run_after_error(opts, "runner_error", e);
    }
    result
}

/// Resume a paused run (after human gate approval).
pub fn resume_run(opts: &RunnerOptions, action_id: &str) -> Result<RunOutcome, TaskGraphError> {
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
            to: format!("running (action '{action_id}' not found)"),
        })?;

    let gate_node_id = paused.node_id.clone();
    let action_result = action.result.clone();

    // If action result is "cancel", cancel the run
    if action_result == "cancel" {
        run_state::update_run_status(ws, project, run_id, RunStatus::Cancelled)?;

        // Mark the gate node as skipped
        let now = Utc::now().to_rfc3339();
        let gate_state = TaskGraphRunNode {
            node_id: gate_node_id,
            status: NodeRunStatus::Skipped,
            started_at: None,
            completed_at: Some(now),
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Human gate rejected: action={action_id}")),
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

    if paused.reason == "interrupt_before" {
        let mut current_run =
            run_state::update_run_status(ws, project, run_id, RunStatus::Running)?;
        current_run.paused = None;
        run_state::write_run_json(ws, project, run_id, &current_run)?;
        return execute_run(opts);
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
        log_tail: Some(format!("Human gate approved: action={action_id}")),
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

    let detail = run_state::read_run_detail(ws, project, run_id)?;
    let graph = &detail.graph_snapshot;

    let Some(gate_node) = graph.nodes.iter().find(|node| node.id == gate_node_id) else {
        run_state::fail_run_active_nodes(
            ws,
            project,
            run_id,
            "human_gate_not_found",
            "Human gate node not found in graph snapshot",
        )?;
        return Ok(RunOutcome::Failed {
            node_id: gate_node_id,
            message: "Human gate node not found in graph snapshot".to_string(),
        });
    };
    let _ = gate_node;

    // Pregel truth update: resume is an external completion of the paused
    // HumanGate task. Write its output and branch trigger into the checkpoint
    // so the scheduler can continue from channel versions.
    let mut current_run = run_state::read_run(ws, project, run_id)?;
    let compiled = compile_graph_for_execution(graph)?;
    let current_checkpoint = current_run
        .pregel_checkpoint
        .clone()
        .unwrap_or_else(|| initial_pregel_checkpoint(&compiled, current_run.context.input.clone()));
    let resume_task = PregelTask {
        id: format!("task-resume-{gate_node_id}"),
        node_id: gate_node_id.clone(),
        kind: PregelTaskKind::Pull,
        triggers: Vec::new(),
        path: vec!["__resume__".to_string(), gate_node_id.clone()],
        input: serde_json::json!({ "action": action_id, "result": action_result }),
    };
    let resume_output = serde_json::json!({ "action": action_id, "result": action_result });
    let mut resume_writes = writes_from_node_outcome(
        &compiled,
        &resume_task,
        Some(&resume_output),
        &[],
        None,
        true,
    )?;
    resume_writes.push(resume_write(&resume_task, resume_output));
    let next_checkpoint = apply_pregel_writes(
        &compiled,
        &current_checkpoint,
        &[resume_task],
        &resume_writes,
        current_run.current_superstep,
    )?;
    current_run.pregel_checkpoint = Some(next_checkpoint);
    run_state::write_run_json(ws, project, run_id, &current_run)?;

    // Continue execution via Coordinator
    execute_run(opts)
}

fn fail_run_after_error(opts: &RunnerOptions, code: &str, error: &TaskGraphError) {
    let message = error.to_string();
    match run_state::fail_run_active_nodes(
        &opts.workspace_root,
        &opts.project,
        &opts.run_id,
        code,
        message.clone(),
    ) {
        Ok(run) => {
            let _ = run_state::append_run_event(
                &opts.workspace_root,
                &opts.project,
                &opts.run_id,
                run.current_superstep,
                "run_failed",
                None,
                "run failed after runner error",
                serde_json::json!({
                    "code": code,
                    "error": message,
                    "active_nodes_finalized": true,
                }),
            );
        }
        Err(finalize_error) => {
            eprintln!(
                "[fast-fail] failed to finalize run {} after error: {}",
                opts.run_id, finalize_error
            );
        }
    }
}
