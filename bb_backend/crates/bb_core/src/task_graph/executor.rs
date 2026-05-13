//! Task Executor — 执行平面并行派发器。
//!
//! 接收 Planner 计算出的 `ReadyNode` 列表，并行执行所有节点，
//! 返回 `Vec<NodeOutcome>` 给 Coordinator 的 Reducer 归并。
//!
//! **核心原则**：Executor 只做"输入 → 输出"，不写任何全局状态。

use std::collections::HashMap;

use super::interpreter::InterpreterOptions;
use super::node_exec;
use super::outcome::{ExecutionMode, NodeOutcome, ReadyNode};
use super::run_state::TaskGraphRun;
use super::types::{TaskGraphEdge, TaskGraphError, TaskGraphNode};

/// 执行一批 ready nodes，返回所有 NodeOutcome。
///
/// - Inline 节点：在当前线程直接执行
/// - Dispatch 节点：使用 `std::thread::scope` 并行执行
/// - 单节点退化优化：只有 1 个节点时不开线程
pub(super) fn execute_ready_nodes(
    opts: &InterpreterOptions,
    ready_nodes: &[ReadyNode],
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    node_map: &HashMap<String, &TaskGraphNode>,
) -> Result<Vec<NodeOutcome>, TaskGraphError> {
    if ready_nodes.is_empty() {
        return Ok(vec![]);
    }

    // 分离 inline 和 dispatch 节点
    let mut inline_nodes: Vec<&ReadyNode> = Vec::new();
    let mut dispatch_nodes: Vec<&ReadyNode> = Vec::new();

    for rn in ready_nodes {
        match rn.execution_mode {
            ExecutionMode::Inline => inline_nodes.push(rn),
            ExecutionMode::Dispatch => dispatch_nodes.push(rn),
        }
    }

    let mut outcomes: Vec<NodeOutcome> = Vec::new();

    // ── 先执行 Inline 节点（控制平面内联，串行） ──
    for rn in &inline_nodes {
        let Some(node) = node_map.get(rn.node_id.as_str()) else {
            return Err(TaskGraphError::ValidationFailed {
                count: 1,
                errors: vec![super::types::TaskGraphValidationError {
                    path: format!("nodes.{}", rn.node_id),
                    code: "node_not_found".to_string(),
                    message: format!("Node '{}' not found in graph snapshot", rn.node_id),
                }],
            });
        };
        let outcome = node_exec::execute_node(opts, node, run, edge_map, node_map)?;
        outcomes.push(outcome);
    }

    // ── 再执行 Dispatch 节点（并行） ──
    if dispatch_nodes.is_empty() {
        return Ok(outcomes);
    }

    if dispatch_nodes.len() == 1 {
        // 单节点退化：不开线程
        let rn = dispatch_nodes[0];
        let Some(node) = node_map.get(rn.node_id.as_str()) else {
            return Err(TaskGraphError::ValidationFailed {
                count: 1,
                errors: vec![super::types::TaskGraphValidationError {
                    path: format!("nodes.{}", rn.node_id),
                    code: "node_not_found".to_string(),
                    message: format!("Node '{}' not found in graph snapshot", rn.node_id),
                }],
            });
        };
        let outcome = node_exec::execute_node(opts, node, run, edge_map, node_map)?;
        outcomes.push(outcome);
        return Ok(outcomes);
    }

    // 多个 Dispatch 节点：并行执行
    let dispatch_results: Vec<(String, Result<NodeOutcome, TaskGraphError>)> =
        std::thread::scope(|s| {
            let handles: Vec<_> = dispatch_nodes
                .iter()
                .map(|rn| {
                    let node_id = rn.node_id.clone();
                    s.spawn(move || {
                        let Some(node) = node_map.get(node_id.as_str()) else {
                            return (
                                node_id.clone(),
                                Err(TaskGraphError::ValidationFailed {
                                    count: 1,
                                    errors: vec![super::types::TaskGraphValidationError {
                                        path: format!("nodes.{}", node_id),
                                        code: "node_not_found".to_string(),
                                        message: format!(
                                            "Node '{}' not found in graph snapshot",
                                            node_id
                                        ),
                                    }],
                                }),
                            );
                        };
                        let result = node_exec::execute_node(opts, node, run, edge_map, node_map);
                        (node_id, result)
                    })
                })
                .collect();

            handles
                .into_iter()
                .map(|h| h.join().expect("parallel node thread panicked"))
                .collect()
        });

    for (_node_id, result) in dispatch_results {
        outcomes.push(result?);
    }

    Ok(outcomes)
}
