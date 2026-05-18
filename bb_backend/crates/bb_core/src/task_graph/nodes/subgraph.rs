use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;

use crate::task_graph::definition::types::{
    SubGraphConfig, TaskGraphEdge, TaskGraphError, TaskGraphNode, TaskGraphScope,
};
use crate::task_graph::nodes::eval::render_prompt_template;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::{execute_run, RunOutcome, RunnerOptions};
use crate::task_graph::pregel::{child_checkpoint_namespace, DEFAULT_CHECKPOINT_NAMESPACE};
use crate::task_graph::run_state::{
    self, GraphRef, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::store;

// ─── Sub-Graph Node ─────────────────────────────────────────────────────────────

pub(super) fn execute_subgraph(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;

    let config: SubGraphConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let start_time = Utc::now().to_rfc3339();

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] SubGraph node starting: graph_id={}, scope={:?}",
            start_time, config.graph_id, config.graph_scope
        ),
    )?;

    let start_instant = Instant::now();

    // 1. Load the child graph definition (with scope fallback)
    let child_graph = match config.graph_scope {
        TaskGraphScope::System => store::read_system_graph(ws, &config.graph_id)
            .or_else(|_| store::read_project_graph(ws, project, &config.graph_id)),
        TaskGraphScope::Project => store::read_project_graph(ws, project, &config.graph_id)
            .or_else(|_| store::read_system_graph(ws, &config.graph_id)),
    };

    let child_graph = match child_graph {
        Ok(g) => g,
        Err(e) => {
            let end_time = Utc::now().to_rfc3339();
            let duration_ms = start_instant.elapsed().as_millis() as u64;
            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                output: None,
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    started_at: Some(start_time),
                    completed_at: Some(end_time),
                    duration_ms: Some(duration_ms),
                    iteration: None,
                    exit_code: None,
                    error: Some(NodeError {
                        code: "child_graph_load_failed".to_string(),
                        message: format!("Failed to load child graph '{}': {}", config.graph_id, e),
                    }),
                    output_artifact: None,
                    log_tail: Some(format!("Load error: {e}")),
                    child_run_id: None,
                    runtime: None,
                    agent: None,
                    model: None,
                    agent_session_id: None,
                    agent_session: None,
                },
                side_effects: vec![],
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            });
        }
    };

    // 2. Resolve input bindings
    let child_input =
        resolve_sub_graph_input(&config, &run.context, project, ws, &opts.scripts_dir);

    // 3. Create child run
    let graph_ref = GraphRef {
        scope: config.graph_scope,
        id: config.graph_id.clone(),
        version: child_graph.version,
    };

    let mut child_run = run_state::create_run(ws, project, graph_ref, &child_graph, child_input)?;
    child_run.parent_run_id = Some(run_id.clone());
    child_run.checkpoint_ns = Some(child_checkpoint_namespace(
        run.checkpoint_ns
            .as_deref()
            .unwrap_or(DEFAULT_CHECKPOINT_NAMESPACE),
        &node.id,
    ));
    run_state::write_run_json(ws, project, &child_run.id, &child_run)?;

    let child_run_id = child_run.id.clone();

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] Created child run: {} checkpoint_ns={}",
            Utc::now().to_rfc3339(),
            child_run_id,
            child_run.checkpoint_ns.as_deref().unwrap_or_default()
        ),
    )?;

    // 3.5 立即写入 running 状态的 node_state（包含 child_run_id），
    //     让前端 SSE 能实时看到节点正在执行并可以点击进入子 run
    let running_state = TaskGraphRunNode {
        node_id: node.id.clone(),
        status: NodeRunStatus::Running,
        started_at: Some(start_time.clone()),
        completed_at: None,
        duration_ms: None,
        iteration: None,
        exit_code: None,
        error: None,
        output_artifact: None,
        log_tail: Some(format!(
            "Executing child graph '{}' (run: {})",
            config.graph_id, child_run_id
        )),
        child_run_id: Some(child_run_id.clone()),
        runtime: None,
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    };
    run_state::update_node_state(ws, project, run_id, &running_state)?;

    // 4. Execute child run recursively
    // 注意：这里使用 runner::execute_run 进行递归调用
    // 子 run 是一个独立的 Coordinator 实例
    let child_opts = RunnerOptions {
        workspace_root: opts.workspace_root.clone(),
        scripts_dir: opts.scripts_dir.clone(),
        project: project.clone(),
        run_id: child_run_id.clone(),
        codex_path: opts.codex_path.clone(),
        codebuddy_path: opts.codebuddy_path.clone(),
        opencode_path: opts.opencode_path.clone(),
        opencode_config_content: opts.opencode_config_content.clone(),
        pi_path: opts.pi_path.clone(),
        model: opts.model.clone(),
        node_timeout: opts.node_timeout,
        run_timeout: opts.run_timeout,
        dry_run: opts.dry_run,
        custom_env: opts.custom_env.clone(),
        custom_args: opts.custom_args.clone(),
        mcp_servers: opts.mcp_servers.clone(),
        skills: opts.skills.clone(),
    };

    let child_outcome = execute_run(&child_opts)?;

    let end_time = Utc::now().to_rfc3339();
    let duration_ms = start_instant.elapsed().as_millis() as u64;

    // 5. Handle child outcome
    match child_outcome {
        RunOutcome::Succeeded => {
            let child_detail = run_state::read_run_detail(ws, project, &child_run_id)?;
            let child_output = collect_child_output(&child_detail);

            Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                output: if child_output.is_null() {
                    None
                } else {
                    Some(child_output)
                },
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Succeeded,
                    started_at: Some(start_time),
                    completed_at: Some(end_time),
                    duration_ms: Some(duration_ms),
                    iteration: None,
                    exit_code: Some(0),
                    error: None,
                    output_artifact: None,
                    log_tail: Some(format!("Child run {child_run_id} succeeded")),
                    child_run_id: Some(child_run_id.clone()),
                    runtime: None,
                    agent: None,
                    model: None,
                    agent_session_id: None,
                    agent_session: None,
                },
                side_effects: vec![],
                child_run_id: Some(child_run_id),
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            })
        }
        RunOutcome::Failed {
            node_id: failed_node,
            message,
        } => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(duration_ms),
                iteration: None,
                exit_code: None,
                error: Some(NodeError {
                    code: "child_graph_failed".to_string(),
                    message: format!(
                        "Child graph '{}' failed at node '{}': {}",
                        config.graph_id, failed_node, message
                    ),
                }),
                output_artifact: None,
                log_tail: Some(format!("Child run {child_run_id} failed: {message}")),
                child_run_id: Some(child_run_id.clone()),
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
        RunOutcome::Paused {
            node_id: paused_node,
        } => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Paused,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Paused,
                started_at: Some(start_time),
                completed_at: None,
                duration_ms: None,
                iteration: None,
                exit_code: None,
                error: None,
                output_artifact: None,
                log_tail: Some(format!(
                    "Child run {child_run_id} paused at node '{paused_node}'"
                )),
                child_run_id: Some(child_run_id.clone()),
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
        RunOutcome::Cancelled => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(duration_ms),
                iteration: None,
                exit_code: None,
                error: Some(NodeError {
                    code: "child_graph_cancelled".to_string(),
                    message: format!("Child graph '{}' was cancelled", config.graph_id),
                }),
                output_artifact: None,
                log_tail: Some(format!("Child run {child_run_id} cancelled")),
                child_run_id: Some(child_run_id.clone()),
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
    }
}

// ─── Sub-Graph Helpers ──────────────────────────────────────────────────────────

/// Resolve input bindings for a sub-graph node.
fn resolve_sub_graph_input(
    config: &SubGraphConfig,
    context: &run_state::RunContext,
    project: &str,
    ws: &std::path::Path,
    scripts_dir: &std::path::Path,
) -> serde_json::Value {
    let Some(ref bindings) = config.input_bindings else {
        return serde_json::Value::Object(serde_json::Map::new());
    };

    match bindings {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, value) in map {
                let resolved = match value {
                    serde_json::Value::String(s) => {
                        let rendered =
                            render_prompt_template(s, project, ws, scripts_dir, context, None);
                        serde_json::from_str(&rendered)
                            .unwrap_or(serde_json::Value::String(rendered))
                    }
                    other => other.clone(),
                };
                result.insert(key.clone(), resolved);
            }
            serde_json::Value::Object(result)
        }
        other => other.clone(),
    }
}

/// Collect the output from a completed child run.
fn collect_child_output(detail: &run_state::TaskGraphRunDetail) -> serde_json::Value {
    if !detail.run.context.node_outputs.is_empty() {
        let graph_nodes = &detail.graph_snapshot.nodes;
        for node in graph_nodes.iter().rev() {
            if let Some(output) = detail.run.context.node_outputs.get(&node.id) {
                return output.clone();
            }
        }
    }
    serde_json::Value::Null
}
