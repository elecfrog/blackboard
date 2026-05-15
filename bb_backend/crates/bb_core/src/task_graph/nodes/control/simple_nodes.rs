use std::collections::HashMap;

use chrono::Utc;

use crate::task_graph::definition::types::{
    EndConfig, HumanGateConfig, InputVarConfig, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::navigation::resolve_next_nodes;
use crate::task_graph::pregel::outcome::{NodeOutcome, SideEffect};
use crate::task_graph::run_state::{
    NodeRunStatus, PausedAction, RunPaused, TaskGraphRun, TaskGraphRunNode,
};

pub(crate) fn execute_start_node(
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let now = Utc::now().to_rfc3339();
    let next = resolve_next_nodes(edge_map, &node.id, node, &run.context, None)?;
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        next_nodes: next,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
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
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
    })
}

// ─── End Node ────────────────────────────────────────────────────────────────

pub(crate) fn execute_end_node(node: &TaskGraphNode) -> Result<NodeOutcome, TaskGraphError> {
    let config: EndConfig = serde_json::from_value(node.config.clone()).unwrap_or(EndConfig {
        result: "succeeded".to_string(),
    });

    let now = Utc::now().to_rfc3339();
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        next_nodes: vec![],
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
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
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: Some(config.result),
    })
}

// ─── InputVar Node ───────────────────────────────────────────────────────────

pub(crate) fn execute_input_var_node(
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: InputVarConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let value = run
        .context
        .input
        .get(&config.input_id)
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let now = Utc::now().to_rfc3339();
    let next = resolve_next_nodes(edge_map, &node.id, node, &run.context, None)?;

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        next_nodes: next,
        output: Some(value),
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Read input: {}", config.input_id)),
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
    })
}

// ─── HumanGate Node ──────────────────────────────────────────────────────────

pub(crate) fn execute_human_gate_node(node: &TaskGraphNode) -> Result<NodeOutcome, TaskGraphError> {
    let config: HumanGateConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let now = Utc::now().to_rfc3339();
    let paused = RunPaused {
        node_id: node.id.clone(),
        reason: "waiting_for_human_gate".to_string(),
        actions: config
            .actions
            .iter()
            .map(|a| PausedAction {
                id: a.id.clone(),
                label: a.label.clone(),
                result: a.result.clone(),
            })
            .collect(),
    };

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Paused,
        next_nodes: vec![],
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Paused,
            started_at: Some(now),
            completed_at: None,
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Waiting for human: {}", config.title)),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![SideEffect::RunPaused(paused)],
        child_run_id: None,
        end_result: None,
    })
}
