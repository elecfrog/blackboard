use std::collections::HashMap;

use crate::task_graph::definition::types::{TaskGraphEdge, TaskGraphNode};

// ─── Graph navigation helpers ────────────────────────────────────────────────

/// Build a node lookup map: `node_id` → node.
#[allow(dead_code)]
pub(super) fn build_node_map(nodes: &[TaskGraphNode]) -> HashMap<String, &TaskGraphNode> {
    nodes.iter().map(|n| (n.id.clone(), n)).collect()
}

/// Find outgoing edges for a node, optionally filtered by `from_pin` (or fallback to `source_handle`).
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
                    .is_some_and(|h| h.starts_with("pin:"));
                dominated_by_from_pin && (has_from_pin || !old_pin_prefix)
            })
            .collect(),
    }
}
