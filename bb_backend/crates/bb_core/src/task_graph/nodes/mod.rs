//! Concrete node execution used by the task executor.
//!
//! Each node type's execution function returns a `NodeOutcome` instead of
//! applying final state directly. The Coordinator's Reducer is responsible
//! for committing the final outcome to the RunState.
//!
//! Long-running runtime nodes may publish live logs and running-state projections
//! so the UI can stream progress before the reducer sees the final outcome.

mod control;
pub mod eval;
pub mod llm;
mod navigation;
pub mod registry;
mod runtime;
mod subgraph;

use std::collections::HashMap;

use crate::task_graph::definition::types::{
    NodeType, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::TaskGraphRun;

pub(super) use navigation::{build_edge_map, resolve_next_nodes};

// ─── Main dispatch ───────────────────────────────────────────────────────────

/// Execute a single node and return a `NodeOutcome`.
///
/// Final state is returned as `NodeOutcome`; live progress writes are limited
/// to runtime-oriented nodes.
pub(super) fn execute_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    _node_map: &HashMap<String, &TaskGraphNode>,
) -> Result<NodeOutcome, TaskGraphError> {
    match node.node_type {
        NodeType::Start => control::execute_start_node(node, run, edge_map),
        NodeType::End => control::execute_end_node(node),
        NodeType::Branch => control::execute_branch_node(node, run, edge_map),
        NodeType::Loop => control::execute_loop_node(node, run, edge_map),
        NodeType::InputVar => control::execute_input_var_node(node, run, edge_map),
        NodeType::HumanGate => control::execute_human_gate_node(node),
        NodeType::Llm => runtime::execute_llm_node(opts, node, run, edge_map),
        NodeType::Shell => runtime::execute_shell_node(opts, node, run, edge_map),
        NodeType::SubGraph => subgraph::execute_subgraph(opts, node, run, edge_map),
    }
}
