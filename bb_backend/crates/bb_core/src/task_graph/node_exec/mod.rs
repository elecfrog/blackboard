//! Per-node execution logic — extracted from interpreter.rs.
//!
//! Each node type's execution function returns a `NodeOutcome` instead of
//! writing global state directly. The Coordinator's Reducer is responsible
//! for applying the outcome to the RunState.
//!
//! **Invariant**: No function in this module calls `run_state::update_node_state`,
//! `run_state::set_node_output`, `run_state::record_branch_decision`,
//! `run_state::record_loop_iteration`, `run_state::push_loop_frame`,
//! `run_state::pop_loop_frame`, or `run_state::update_cursor`.
//!
//! The only allowed I/O side-effect is `run_state::append_node_log` (streaming logs).

mod control_nodes;
mod navigation;
mod runtime_nodes;
mod sub_graph_node;

use std::collections::HashMap;

use super::interpreter::InterpreterOptions;
use super::outcome::NodeOutcome;
use super::run_state::TaskGraphRun;
use super::types::{NodeType, TaskGraphEdge, TaskGraphError, TaskGraphNode};

pub(super) use navigation::{build_edge_map, find_start_node, resolve_next_nodes};

// ─── Main dispatch ───────────────────────────────────────────────────────────

/// Execute a single node and return a `NodeOutcome`.
///
/// **This function does NOT write any global state** (no `run_state::update_*` calls).
/// The only allowed I/O is `run_state::append_node_log` for streaming log output.
pub(super) fn execute_node(
    opts: &InterpreterOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    _node_map: &HashMap<String, &TaskGraphNode>,
) -> Result<NodeOutcome, TaskGraphError> {
    match node.node_type {
        NodeType::Start => control_nodes::execute_start_node(node, run, edge_map),
        NodeType::End => control_nodes::execute_end_node(node),
        NodeType::Branch => control_nodes::execute_branch_node(node, run, edge_map),
        NodeType::Loop => control_nodes::execute_loop_node(node, run, edge_map),
        NodeType::InputVar => control_nodes::execute_input_var_node(node, run, edge_map),
        NodeType::HumanGate => control_nodes::execute_human_gate_node(node),
        NodeType::Llm => runtime_nodes::execute_llm_node(opts, node, run, edge_map),
        NodeType::SubGraph => sub_graph_node::execute_sub_graph_node(opts, node, run, edge_map),
    }
}
