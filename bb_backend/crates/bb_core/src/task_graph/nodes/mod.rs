//! Concrete node execution used by the task executor.
//!
//! Each node type's execution function returns a `NodeOutcome` instead of
//! applying final state directly. The Coordinator's Reducer is responsible
//! for committing the final outcome to the `RunState`.
//!
//! Long-running runtime nodes may publish live logs and running-state projections
//! so the UI can stream progress before the reducer sees the final outcome.

mod control;
pub mod eval;
mod intent_extract;
mod kb_plan;
mod kb_staging;
pub mod llm;
mod llm_coordinator;
mod manifest_merge;
mod navigation;
mod plan;
pub mod registry;
mod runtime;
mod schema_validate;
mod subgraph;
mod system_write_output;
mod topology_mutation;

use std::collections::HashMap;

use crate::task_graph::definition::types::{
    NodeType, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::TaskGraphRun;

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
        NodeType::DataValue => control::execute_data_value_node(opts, node, run),
        NodeType::Plan => plan::execute_plan_node(opts, node, run, edge_map),
        NodeType::IntentExtract => intent_extract::execute_intent_extract_node(opts, node, run),
        NodeType::KbPlan => kb_plan::execute_kb_plan_node(opts, node, run),
        NodeType::ManifestMerge => manifest_merge::execute_manifest_merge_node(opts, node, run),
        NodeType::SchemaValidate => schema_validate::execute_schema_validate_node(opts, node, run),
        NodeType::SystemWriteOutput => {
            system_write_output::execute_system_write_output_node(opts, node, run)
        }
        NodeType::HumanGate => control::execute_human_gate_node(node),
        NodeType::Llm => runtime::execute_llm_node(opts, node, run, edge_map),
        NodeType::LlmCoordinator => {
            llm_coordinator::execute_llm_coordinator_node(opts, node, run, edge_map)
        }
        NodeType::LlmMutation => {
            topology_mutation::execute_llm_mutation_node(opts, node, run, edge_map)
        }
        NodeType::Shell => runtime::execute_shell_node(opts, node, run, edge_map),
        NodeType::SubGraph => subgraph::execute_subgraph(opts, node, run, edge_map),
    }
}
