//! Task Executor — 执行平面并行派发器。
//!
//! 接收 Planner 计算出的 `ReadyNode` 列表，并行执行所有节点，
//! 返回 `Vec<NodeOutcome>` 给 Coordinator 的 Reducer 归并。
//!
//! **核心原则**：Executor 只做"输入 → 输出"。
//! 最终状态由 Coordinator Reducer 归并；长运行节点可以写 streaming log / running projection。

use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::{Map, Value};

use super::outcome::{ExecutionMode, NodeOutcome, ReadyNode};
use super::runner::RunnerOptions;
use crate::task_graph::definition::types::{
    TaskGraphEdge, TaskGraphError, TaskGraphNode, TaskGraphValidationError,
};
use crate::task_graph::nodes;
use crate::task_graph::pregel::PregelTaskKind;
use crate::task_graph::run_state::TaskGraphRun;

/// 执行一批 ready nodes，返回所有 NodeOutcome。
///
/// - Inline 节点：在当前线程直接执行
/// - Dispatch 节点：使用 `std::thread::scope` 并行执行
/// - 单节点退化优化：只有 1 个节点时不开线程
pub(super) fn execute_ready_nodes(
    opts: &RunnerOptions,
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
                errors: vec![TaskGraphValidationError {
                    path: format!("nodes.{}", rn.node_id),
                    code: "node_not_found".to_string(),
                    message: format!("Node '{}' not found in graph snapshot", rn.node_id),
                }],
            });
        };
        let run_for_node = run_for_ready_node(run, rn);
        let outcome = nodes::execute_node(opts, node, run_for_node.as_ref(), edge_map, node_map)?;
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
                errors: vec![TaskGraphValidationError {
                    path: format!("nodes.{}", rn.node_id),
                    code: "node_not_found".to_string(),
                    message: format!("Node '{}' not found in graph snapshot", rn.node_id),
                }],
            });
        };
        let run_for_node = run_for_ready_node(run, rn);
        let outcome = nodes::execute_node(opts, node, run_for_node.as_ref(), edge_map, node_map)?;
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
                                    errors: vec![TaskGraphValidationError {
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
                        let run_for_node = run_for_ready_node(run, rn);
                        let result = nodes::execute_node(
                            opts,
                            node,
                            run_for_node.as_ref(),
                            edge_map,
                            node_map,
                        );
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

fn run_for_ready_node<'a>(run: &'a TaskGraphRun, ready_node: &ReadyNode) -> Cow<'a, TaskGraphRun> {
    if ready_node.task_kind == PregelTaskKind::Push {
        let mut task_run = run.clone();
        task_run.context.input = ready_node.task_input.clone();
        return Cow::Owned(task_run);
    }

    let data_inputs = project_data_inputs(&ready_node.task_input);
    if data_inputs.is_null() {
        return Cow::Borrowed(run);
    }

    let mut task_run = run.clone();
    let mut input = task_run
        .context
        .input
        .as_object()
        .cloned()
        .unwrap_or_default();
    input.insert("__task_input".to_string(), ready_node.task_input.clone());
    input.insert("__data".to_string(), data_inputs);
    task_run.context.input = Value::Object(input);
    Cow::Owned(task_run)
}

fn project_data_inputs(task_input: &Value) -> Value {
    let Some(map) = task_input.as_object() else {
        return Value::Null;
    };

    let mut data = Map::new();
    for (channel, value) in map {
        let mut parts = channel.splitn(3, ':');
        if parts.next() != Some("data") {
            continue;
        }
        let Some(source_node_id) = parts.next() else {
            continue;
        };
        if parts.next().is_none() {
            continue;
        }

        let mut projected = Map::new();
        projected.insert("output".to_string(), value.clone());
        if let Some(artifact_path) = value.get("artifact_path") {
            projected.insert("artifact_path".to_string(), artifact_path.clone());
        }
        if let Some(data_value) = value.get("data") {
            projected.insert("data".to_string(), data_value.clone());
        }
        data.insert(source_node_id.to_string(), Value::Object(projected));
    }

    if data.is_empty() {
        Value::Null
    } else {
        Value::Object(data)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::time::Duration;

    use serde_json::{json, Map, Value};

    use super::*;
    use crate::task_graph::definition::types::{NodeType, TaskGraphScope};
    use crate::task_graph::run_state::{GraphRef, RunContext, RunStatus};

    #[test]
    fn push_task_args_override_run_input_for_this_invocation() {
        let opts = options();
        let node = input_node("read", "value");
        let run = run(json!({ "value": "original" }));
        let ready_nodes = vec![ReadyNode {
            node_id: "read".to_string(),
            execution_mode: ExecutionMode::Inline,
            task_kind: PregelTaskKind::Push,
            task_input: json!({ "value": "sent" }),
        }];
        let edge_map = HashMap::new();
        let mut node_map = HashMap::new();
        node_map.insert("read".to_string(), &node);

        let outcomes =
            execute_ready_nodes(&opts, &ready_nodes, &run, &edge_map, &node_map).unwrap();

        assert_eq!(outcomes[0].output, Some(json!("sent")));
    }

    #[test]
    fn pull_task_keeps_run_input_projection() {
        let opts = options();
        let node = input_node("read", "value");
        let run = run(json!({ "value": "original" }));
        let ready_nodes = vec![ReadyNode {
            node_id: "read".to_string(),
            execution_mode: ExecutionMode::Inline,
            task_kind: PregelTaskKind::Pull,
            task_input: json!({ "value": "sent" }),
        }];
        let edge_map = HashMap::new();
        let mut node_map = HashMap::new();
        node_map.insert("read".to_string(), &node);

        let outcomes =
            execute_ready_nodes(&opts, &ready_nodes, &run, &edge_map, &node_map).unwrap();

        assert_eq!(outcomes[0].output, Some(json!("original")));
    }

    #[test]
    fn pull_task_projects_data_channels_into_template_inputs() {
        let opts = options();
        let node = llm_node("read-data", "{{data.source.artifact_path}}");
        let run = run(json!({ "value": "original" }));
        let ready_nodes = vec![ReadyNode {
            node_id: "read-data".to_string(),
            execution_mode: ExecutionMode::Inline,
            task_kind: PregelTaskKind::Pull,
            task_input: json!({
                "data:source:read-data": {
                    "data": { "kind": "plan" },
                    "artifact_path": "D:/tmp/plan.json",
                    "artifact_type": "json"
                }
            }),
        }];
        let edge_map = HashMap::new();
        let mut node_map = HashMap::new();
        node_map.insert("read-data".to_string(), &node);

        let outcomes =
            execute_ready_nodes(&opts, &ready_nodes, &run, &edge_map, &node_map).unwrap();

        assert_eq!(
            outcomes[0].output.as_ref().unwrap()["prompt"],
            "D:/tmp/plan.json"
        );
    }

    fn options() -> RunnerOptions {
        RunnerOptions {
            workspace_root: std::env::temp_dir(),
            scripts_dir: std::env::temp_dir().join("scripts"),
            project: "blackboard".to_string(),
            run_id: "run-test".to_string(),
            codex_path: "codex".to_string(),
            codebuddy_path: "codebuddy".to_string(),
            opencode_path: "opencode".to_string(),
            opencode_config_content: None,
            model: None,
            node_timeout: Duration::from_secs(1),
            run_timeout: Duration::from_secs(1),
            dry_run: true,
            custom_env: BTreeMap::new(),
            custom_args: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }
    }

    fn run(input: Value) -> TaskGraphRun {
        TaskGraphRun {
            id: "run-test".to_string(),
            project: "blackboard".to_string(),
            graph_ref: GraphRef {
                scope: TaskGraphScope::Project,
                id: "graph".to_string(),
                version: 1,
            },
            status: RunStatus::Running,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            queued_at: None,
            queue_deadline_at: None,
            started_at: None,
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            completed_at: None,
            current_superstep: 0,
            last_checkpoint_id: None,
            pregel_checkpoint: None,
            current_graph_revision: 0,
            active_nodes: vec!["read".to_string()],
            paused: None,
            context: RunContext {
                input,
                node_outputs: Map::new(),
                branch_decisions: vec![],
                loop_iterations: vec![],
                loop_stack: vec![],
                completed_branches: HashMap::new(),
            },
            parent_run_id: None,
            checkpoint_ns: None,
        }
    }

    fn input_node(id: &str, input_id: &str) -> TaskGraphNode {
        TaskGraphNode {
            id: id.to_string(),
            node_type: NodeType::InputVar,
            label: id.to_string(),
            description: None,
            position: None,
            config: json!({ "input_id": input_id }),
            pins: vec![],
        }
    }

    fn llm_node(id: &str, prompt: &str) -> TaskGraphNode {
        TaskGraphNode {
            id: id.to_string(),
            node_type: NodeType::Llm,
            label: id.to_string(),
            description: None,
            position: None,
            config: json!({
                "runtime": "opencode",
                "agent": "native",
                "prompt": {
                    "mode": "inline",
                    "template": prompt
                }
            }),
            pins: vec![],
        }
    }
}
