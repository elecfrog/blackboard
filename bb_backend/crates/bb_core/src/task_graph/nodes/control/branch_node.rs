use std::collections::HashMap;

use chrono::Utc;

use crate::task_graph::definition::types::{
    BranchConfig, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::eval::evaluate_branch;
use crate::task_graph::nodes::navigation::outgoing_edges;
use crate::task_graph::pregel::outcome::{ControlDirective, NodeOutcome, SideEffect};
use crate::task_graph::run_state::{
    BranchDecision, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};

// ─── Branch Node ─────────────────────────────────────────────────────────────

pub(crate) fn execute_branch_node(
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: BranchConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let start_time = Utc::now().to_rfc3339();

    // Evaluate branch
    let selected_rule_id = evaluate_branch(&config, &run.context);

    // Find the matching edge
    let handle = format!("rule:{}", selected_rule_id);
    let matching_edges = outgoing_edges(edge_map, &node.id, Some(&handle));

    if matching_edges.is_empty() {
        // No matching edge — try default
        let default_handle = format!("rule:{}", config.default_rule_id);
        let default_edges = outgoing_edges(edge_map, &node.id, Some(&default_handle));
        if default_edges.is_empty() {
            let now = Utc::now().to_rfc3339();
            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                output: None,
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    started_at: Some(start_time),
                    completed_at: Some(now),
                    duration_ms: None,
                    iteration: None,
                    exit_code: None,
                    error: Some(NodeError {
                        code: "no_matching_edge".to_string(),
                        message: format!(
                            "Branch rule '{}' has no matching outgoing edge",
                            selected_rule_id
                        ),
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
                side_effects: vec![],
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            });
        }

        // Use default edge
        let control = default_edges
            .iter()
            .map(|edge| ControlDirective::goto(edge.to.clone()))
            .collect();
        let now = Utc::now().to_rfc3339();
        let decision = BranchDecision {
            node_id: node.id.clone(),
            selected_rule_id: config.default_rule_id.clone(),
            selected_edge_id: default_edges[0].id.clone(),
            evaluated_at: now.clone(),
        };

        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            output: None,
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                started_at: Some(start_time),
                completed_at: Some(now),
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
            },
            side_effects: vec![SideEffect::BranchDecision(decision)],
            child_run_id: None,
            end_result: None,
            control,
            graph_mutations: vec![],
        });
    }

    // Use matched edge
    let control = matching_edges
        .iter()
        .map(|edge| ControlDirective::goto(edge.to.clone()))
        .collect();
    let now = Utc::now().to_rfc3339();
    let decision = BranchDecision {
        node_id: node.id.clone(),
        selected_rule_id: selected_rule_id.clone(),
        selected_edge_id: matching_edges[0].id.clone(),
        evaluated_at: now.clone(),
    };

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(start_time),
            completed_at: Some(now),
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
        },
        side_effects: vec![SideEffect::BranchDecision(decision)],
        child_run_id: None,
        end_result: None,
        control,
        graph_mutations: vec![],
    })
}
