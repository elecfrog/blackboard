//! Task Graph structural validation — 13 control-flow rules + config checks.
//!
//! 包含两层校验：
//! - `validate_graph`: 结构校验（保存时调用）
//! - `validate_pre_run`: 运行前完整性校验（创建 run 时调用）

mod config;
mod cycles;
mod decode;
mod pins;
mod pre_run;

use std::collections::{HashMap, HashSet};

use crate::task_graph::definition::types::*;

pub use decode::{
    decode_graph_value_at, parse_json_source, prefix_validation_errors, validate_graph_source,
    validate_graph_value, validate_graph_value_at,
};
pub use pre_run::validate_pre_run;

/// Validate a task graph definition against the MVP contract rules.
///
/// Returns an empty vec if valid; otherwise a list of structured errors.
pub fn validate_graph(def: &TaskGraphDefinition) -> Vec<TaskGraphValidationError> {
    let mut errors = Vec::new();

    // Build indexes
    let node_map: HashMap<&str, &TaskGraphNode> =
        def.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let node_ids: HashSet<&str> = def.nodes.iter().map(|n| n.id.as_str()).collect();

    // ── Graph ID format ──────────────────────────────────────────────────────
    if !is_valid_graph_id(&def.id) {
        errors.push(TaskGraphValidationError {
            path: "id".to_string(),
            code: "invalid_format".to_string(),
            message: format!("Graph id '{}' must match ^[a-z][a-z0-9-]{{1,63}}$", def.id),
        });
    }

    config::validate_graph_inputs(def, &mut errors);

    // ── Node ID format ───────────────────────────────────────────────────────
    for (i, node) in def.nodes.iter().enumerate() {
        if !config::is_valid_node_id(&node.id) {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{}].id", i),
                code: "invalid_format".to_string(),
                message: format!("Node id '{}' contains invalid characters", node.id),
            });
        }
    }

    // ── Rule 1: exactly one start node ───────────────────────────────────────
    let start_nodes: Vec<_> = def
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Start)
        .collect();
    if start_nodes.len() != 1 {
        errors.push(TaskGraphValidationError {
            path: "nodes".to_string(),
            code: "start_node_count".to_string(),
            message: format!(
                "Graph must have exactly one start node, found {}",
                start_nodes.len()
            ),
        });
    }

    // ── Rule 2: at least one end node ────────────────────────────────────────
    let end_nodes: Vec<_> = def
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::End)
        .collect();
    if end_nodes.is_empty() {
        errors.push(TaskGraphValidationError {
            path: "nodes".to_string(),
            code: "end_node_missing".to_string(),
            message: "Graph must have at least one end node".to_string(),
        });
    }

    // ── Rule 3: all edge endpoints exist ─────────────────────────────────────
    for (i, edge) in def.edges.iter().enumerate() {
        if !node_ids.contains(edge.from.as_str()) {
            errors.push(TaskGraphValidationError {
                path: format!("edges[{}].from", i),
                code: "endpoint_not_found".to_string(),
                message: format!(
                    "Edge '{}' references non-existent from node '{}'",
                    edge.id, edge.from
                ),
            });
        }
        if !node_ids.contains(edge.to.as_str()) {
            errors.push(TaskGraphValidationError {
                path: format!("edges[{}].to", i),
                code: "endpoint_not_found".to_string(),
                message: format!(
                    "Edge '{}' references non-existent to node '{}'",
                    edge.id, edge.to
                ),
            });
        }
    }

    // Build incoming/outgoing maps for structural rules
    let mut incoming: HashMap<&str, Vec<&TaskGraphEdge>> = HashMap::new();
    let mut outgoing: HashMap<&str, Vec<&TaskGraphEdge>> = HashMap::new();
    for edge in &def.edges {
        if node_ids.contains(edge.from.as_str()) && node_ids.contains(edge.to.as_str()) {
            incoming.entry(edge.to.as_str()).or_default().push(edge);
            outgoing.entry(edge.from.as_str()).or_default().push(edge);
        }
    }

    // ── Rule 4: start no incoming edge ───────────────────────────────────────
    for node in &start_nodes {
        if incoming.contains_key(node.id.as_str()) && !incoming[node.id.as_str()].is_empty() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[start:{}]", node.id),
                code: "start_has_incoming".to_string(),
                message: "Start node must not have incoming edges".to_string(),
            });
        }
    }

    // ── Rule 5: end no outgoing edge ─────────────────────────────────────────
    for node in &end_nodes {
        if outgoing.contains_key(node.id.as_str()) && !outgoing[node.id.as_str()].is_empty() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[end:{}]", node.id),
                code: "end_has_outgoing".to_string(),
                message: "End node must not have outgoing edges".to_string(),
            });
        }
    }

    // ── Rule 6 & 7: Branch rules have matching outgoing edges ────────────────
    for (i, node) in def.nodes.iter().enumerate() {
        if node.node_type == NodeType::Branch {
            config::validate_branch_config(node, i, &outgoing, &mut errors);
        }
    }

    // ── Rule 8 & 9: Loop has max_iterations and body/exit edges ──────────────
    for (i, node) in def.nodes.iter().enumerate() {
        if node.node_type == NodeType::Loop {
            config::validate_loop_config(node, i, &outgoing, &incoming, &mut errors);
        }
    }

    // ── Rule 10: cycle detection (cycles must pass through loop controllers) ─
    cycles::validate_no_illegal_cycles(def, &node_map, &mut errors);

    // ── Rule 14: sub_graph.graph_id is valid ─────────────────────────────────────
    for (i, node) in def.nodes.iter().enumerate() {
        if node.node_type == NodeType::SubGraph {
            config::validate_sub_graph_config(node, i, &mut errors);
        }
    }
    // ── Rule 12: llm.runtime is known ────────────────────────────────────────
    for (i, node) in def.nodes.iter().enumerate() {
        if node.node_type == NodeType::Llm {
            config::validate_llm_config(node, i, &mut errors);
        }
    }

    // ── Rule 13: scope/readonly consistency ──────────────────────────────────
    // (Handled at API layer: system graph writes are rejected by store.)

    // ── Node config type-matching ────────────────────────────────────────────
    for (i, node) in def.nodes.iter().enumerate() {
        config::validate_node_config_shape(node, i, &mut errors);
    }

    // ── Pin connection validation ────────────────────────────────────────────
    pins::validate_pin_connections(def, &node_map, &incoming, &mut errors);

    errors
}

// ─── Graph ID validation ─────────────────────────────────────────────────────

/// Graph id must match `^[a-z][a-z0-9-]{1,63}$`.
pub fn is_valid_graph_id(id: &str) -> bool {
    if id.len() < 2 || id.len() > 64 {
        return false;
    }
    let bytes = id.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
