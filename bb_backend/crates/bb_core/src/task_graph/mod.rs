//! Task Graph storage and validation layer.
//!
//! Implements System Graph registry (read-only), Project Graph store (CRUD),
//! structural validation, run state management, and workflow interpreter
//! for the Task Graph MVP contract.

pub mod coordinator;
pub mod eval;
pub mod executor;
pub mod interpreter;
pub mod llm;
pub mod node_exec;
pub mod outcome;
pub mod pins;
pub mod run_state;
pub mod runtime;
pub mod store;
pub mod types;
pub mod upgrade;
pub mod validation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod interpreter_tests;

pub use eval::{evaluate_branch, evaluate_loop_condition};
pub use interpreter::{execute_run, resume_run, InterpreterOptions, RunOutcome, StepResult};
pub use llm::{
    build_codebuddy_mcp_config_content, build_codebuddy_settings_json, build_codex_mcp_config_args,
    build_opencode_task_graph_config,
};
pub use outcome::{ExecutionMode, NodeOutcome, ReadyNode, ReduceAction, SideEffect};
pub use pins::default_pins_for;
pub use run_state::{
    append_node_log, cancel_run_cascade, create_run, list_runs, read_run, read_run_detail,
    record_branch_decision, record_loop_iteration, set_node_output, set_run_paused, update_cursor,
    update_node_state, update_run_status, write_artifact, ArtifactContentType, BranchDecision,
    GraphRef, LoopFrame, LoopIterationEntry, LoopIterationResult, LoopIterationState, NodeError,
    NodeRunStatus, OutputArtifact, PausedAction, RunContext, RunPaused, RunStatus, TaskGraphRun,
    TaskGraphRunDetail, TaskGraphRunNode, TaskGraphRunSummary,
};
pub use store::{
    delete_project_graph, list_project_graphs, list_system_graphs, read_project_graph,
    read_system_graph, save_project_graph, save_system_graph,
};
pub use types::*;
pub use upgrade::upgrade_graph;
pub use validation::{validate_graph, validate_pre_run};
