use std::collections::HashMap;

use crate::task_graph::definition::types::{TaskGraphEdge, TaskGraphError, TaskGraphNode};
use crate::task_graph::run_state::{self, LoopFrame};

// ─── Graph navigation helpers ────────────────────────────────────────────────

/// Build an adjacency map: node_id → outgoing edges.
pub(in crate::task_graph) fn build_edge_map(
    edges: &[TaskGraphEdge],
) -> HashMap<String, Vec<&TaskGraphEdge>> {
    let mut map: HashMap<String, Vec<&TaskGraphEdge>> = HashMap::new();
    for edge in edges {
        map.entry(edge.from.clone()).or_default().push(edge);
    }
    map
}

/// Build a node lookup map: node_id → node.
#[allow(dead_code)]
pub(super) fn build_node_map(nodes: &[TaskGraphNode]) -> HashMap<String, &TaskGraphNode> {
    nodes.iter().map(|n| (n.id.clone(), n)).collect()
}

/// Find outgoing edges for a node, optionally filtered by from_pin (or fallback to source_handle).
pub(super) fn outgoing_edges<'a>(
    edge_map: &'a HashMap<String, Vec<&'a TaskGraphEdge>>,
    node_id: &str,
    from_pin: Option<&str>,
) -> Vec<&'a TaskGraphEdge> {
    let edges = edge_map.get(node_id).cloned().unwrap_or_default();
    match from_pin {
        Some(pin_id) => edges
            .into_iter()
            .filter(|e| {
                e.from_pin.as_deref() == Some(pin_id) || e.source_handle.as_deref() == Some(pin_id)
            })
            .collect(),
        None => edges
            .into_iter()
            .filter(|e| {
                let has_from_pin = e.from_pin.is_some();
                let dominated_by_from_pin =
                    e.from_pin.as_deref() == Some("exec_out") || e.from_pin.is_none();
                let old_pin_prefix = e
                    .source_handle
                    .as_deref()
                    .map(|h| h.starts_with("pin:"))
                    .unwrap_or(false);
                dominated_by_from_pin && (has_from_pin || !old_pin_prefix)
            })
            .collect(),
    }
}

/// Resolve next node(s) from the current node by following outgoing edges.
pub(in crate::task_graph) fn resolve_next_nodes(
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    node_id: &str,
    _node: &TaskGraphNode,
    context: &run_state::RunContext,
    from_pin: Option<&str>,
) -> Result<Vec<String>, TaskGraphError> {
    let edges = match from_pin {
        Some(pin_id) => outgoing_edges(edge_map, node_id, Some(pin_id)),
        None => outgoing_edges(edge_map, node_id, None),
    };

    if from_pin.is_none() {
        if let Some(frame) = context.loop_stack.last() {
            if edges.is_empty()
                || edges
                    .iter()
                    .any(|edge| edge_returns_to_loop_frame(edge, frame))
            {
                return Ok(vec![frame.loop_node_id.clone()]);
            }
        }
    }

    let next_ids: Vec<String> = edges.iter().map(|e| e.to.clone()).collect();
    Ok(next_ids)
}

fn edge_returns_to_loop_frame(edge: &TaskGraphEdge, frame: &LoopFrame) -> bool {
    edge.to == frame.loop_node_id
        && (edge.to_pin.as_deref() == Some("return")
            || edge.target_handle.as_deref() == Some("return")
            || edge.to_pin.as_deref() == Some("exec_in")
            || edge.to_pin.is_none())
}
