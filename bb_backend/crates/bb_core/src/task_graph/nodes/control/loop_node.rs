use std::collections::HashMap;

use chrono::Utc;

use crate::task_graph::definition::types::{
    LoopConfig, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::eval::{evaluate_loop_condition, resolve_loop_max_iterations};
use crate::task_graph::nodes::navigation::outgoing_edges;
use crate::task_graph::pregel::outcome::{ControlDirective, NodeOutcome, SideEffect};
use crate::task_graph::run_state::{
    LoopFrame, LoopIterationEntry, LoopIterationResult, LoopIterationState, NodeError,
    NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};

// ─── Loop Node ───────────────────────────────────────────────────────────────

pub fn execute_loop_node(
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: LoopConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let mut side_effects: Vec<SideEffect> = Vec::new();

    // 如果当前 loop_stack 顶部是本节点的 frame，说明是从 body 返回，需要 pop
    if run
        .context
        .loop_stack
        .last()
        .filter(|frame| frame.loop_node_id == node.id)
        .is_some()
    {
        let frame = run.context.loop_stack.last().unwrap().clone();
        side_effects.push(SideEffect::LoopFramePop(node.id.clone()));

        // 标记当前迭代完成
        if let Some(existing) = run
            .context
            .loop_iterations
            .iter()
            .find(|state| state.loop_node_id == node.id)
        {
            let mut history = existing.history.clone();
            if let Some(entry) = history
                .iter_mut()
                .rev()
                .find(|entry| entry.iteration == frame.iteration && entry.completed_at.is_none())
            {
                entry.completed_at = Some(Utc::now().to_rfc3339());
            }
            side_effects.push(SideEffect::LoopIteration(LoopIterationState {
                loop_node_id: existing.loop_node_id.clone(),
                current_iteration: existing.current_iteration,
                max_iterations: existing.max_iterations,
                exit_reason: existing.exit_reason.clone(),
                history,
            }));
        }
    }

    // 使用传入的 context 获取 loop 状态（Reducer 会在每轮调度前更新 context）
    // 注意：side_effects 中可能已经有 LoopIteration 更新，但这里我们基于传入的 context 计算
    // Reducer 会按顺序应用 side_effects
    let existing_loop = run
        .context
        .loop_iterations
        .iter()
        .find(|l| l.loop_node_id == node.id);

    let current_iteration = existing_loop.map_or(0, |l| l.current_iteration);
    let max_iterations = resolve_loop_max_iterations(&config, &run.context);

    // Check loop condition before max_iterations. When the body just returned a
    // clean result on the final allowed iteration, the loop should exit cleanly.
    let condition_met = evaluate_loop_condition(&config, &run.context);

    if !condition_met && current_iteration > 0 {
        let exit_reason = "condition_false";
        let history = existing_loop.map(|l| l.history.clone()).unwrap_or_default();

        side_effects.push(SideEffect::LoopIteration(LoopIterationState {
            loop_node_id: node.id.clone(),
            current_iteration,
            max_iterations,
            exit_reason: Some(exit_reason.to_string()),
            history,
        }));

        let now = Utc::now().to_rfc3339();
        let exit_edges = outgoing_edges(edge_map, &node.id, Some("exit"));
        if let Some(exit_edge) = exit_edges.first() {
            let next = vec![exit_edge.to.clone()];
            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                output: None,
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Succeeded,
                    started_at: Some(now.clone()),
                    completed_at: Some(now),
                    duration_ms: None,
                    iteration: Some(current_iteration),
                    exit_code: None,
                    error: None,
                    output_artifact: None,
                    log_tail: Some(format!("Exit: {exit_reason}")),
                    child_run_id: None,
                    runtime: None,
                    agent: None,
                    model: None,
                    agent_session_id: None,
                    agent_session: None,
                },
                side_effects,
                child_run_id: None,
                end_result: None,
                control: next.into_iter().map(ControlDirective::goto).collect(),
                graph_mutations: vec![],
            });
        }
        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(now.clone()),
                completed_at: Some(now),
                duration_ms: None,
                iteration: Some(current_iteration),
                exit_code: None,
                error: Some(NodeError {
                    code: "no_exit_edge".to_string(),
                    message: "No exit edge for loop node".to_string(),
                }),
                output_artifact: None,
                log_tail: Some(format!("Exit: {exit_reason}")),
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects,
            child_run_id: None,
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        });
    }

    // Check max_iterations only after a false condition had a chance to exit.
    if current_iteration >= max_iterations {
        let exit_reason = "max_iterations_reached";
        side_effects.push(SideEffect::LoopIteration(LoopIterationState {
            loop_node_id: node.id.clone(),
            current_iteration,
            max_iterations,
            exit_reason: Some(exit_reason.to_string()),
            history: existing_loop.map(|l| l.history.clone()).unwrap_or_default(),
        }));

        let now = Utc::now().to_rfc3339();
        let on_max = config.on_max_iterations.as_deref().unwrap_or("fail");
        let loop_node_status = if on_max == "fail" {
            NodeRunStatus::Failed
        } else {
            NodeRunStatus::Succeeded
        };

        // Take exit edge
        let exit_edges = outgoing_edges(edge_map, &node.id, Some("exit"));
        let next = exit_edges
            .first()
            .map(|e| vec![e.to.clone()])
            .unwrap_or_default();

        // 如果没有 exit edge 且 on_max == "fail"，返回 Failed
        if next.is_empty() && on_max == "fail" {
            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                output: None,
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    started_at: Some(now.clone()),
                    completed_at: Some(now),
                    duration_ms: None,
                    iteration: Some(current_iteration),
                    exit_code: None,
                    error: Some(NodeError {
                        code: "max_iterations_reached".to_string(),
                        message: "Max iterations reached and no exit edge".to_string(),
                    }),
                    output_artifact: None,
                    log_tail: Some(format!("Exit: {exit_reason}")),
                    child_run_id: None,
                    runtime: None,
                    agent: None,
                    model: None,
                    agent_session_id: None,
                    agent_session: None,
                },
                side_effects,
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            });
        }

        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: loop_node_status,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: loop_node_status,
                started_at: Some(now.clone()),
                completed_at: Some(now),
                duration_ms: None,
                iteration: Some(current_iteration),
                exit_code: None,
                error: if on_max == "fail" {
                    Some(NodeError {
                        code: "max_iterations_reached".to_string(),
                        message: format!("Loop reached max iterations ({max_iterations})"),
                    })
                } else {
                    None
                },
                output_artifact: None,
                log_tail: Some(format!("Exit: {exit_reason}")),
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects,
            child_run_id: None,
            end_result: None,
            control: next.into_iter().map(ControlDirective::goto).collect(),
            graph_mutations: vec![],
        });
    }

    // Continue iterating: increment counter, record iteration, enter body
    let new_iteration = current_iteration + 1;
    let now = Utc::now().to_rfc3339();
    let mut history = existing_loop.map(|l| l.history.clone()).unwrap_or_default();
    history.push(LoopIterationEntry {
        iteration: new_iteration,
        started_at: now.clone(),
        completed_at: None,
        result: LoopIterationResult::Continued,
    });

    side_effects.push(SideEffect::LoopIteration(LoopIterationState {
        loop_node_id: node.id.clone(),
        current_iteration: new_iteration,
        max_iterations,
        exit_reason: None,
        history,
    }));

    // Enter the loop body
    let body_edges = outgoing_edges(edge_map, &node.id, Some("body"));
    if let Some(body_edge) = body_edges.first() {
        side_effects.push(SideEffect::LoopFramePush(LoopFrame {
            loop_node_id: node.id.clone(),
            iteration: new_iteration,
            body_entry_node_id: body_edge.to.clone(),
            started_at: now.clone(),
        }));
        let next = vec![body_edge.to.clone()];
        Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Running,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Running,
                started_at: Some(now),
                completed_at: None,
                duration_ms: None,
                iteration: Some(new_iteration),
                exit_code: None,
                error: None,
                output_artifact: None,
                log_tail: Some(format!("Iteration {new_iteration}/{max_iterations}")),
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects,
            child_run_id: None,
            end_result: None,
            control: next.into_iter().map(ControlDirective::goto).collect(),
            graph_mutations: vec![],
        })
    } else {
        Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(now.clone()),
                completed_at: Some(now),
                duration_ms: None,
                iteration: Some(new_iteration),
                exit_code: None,
                error: Some(NodeError {
                    code: "no_body_edge".to_string(),
                    message: "Loop node has no body edge".to_string(),
                }),
                output_artifact: None,
                log_tail: None,
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects,
            child_run_id: None,
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        })
    }
}
