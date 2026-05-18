//! `TaskGraph` public API surface.
//!
//! The graph execution engine lives in `pregel/`; this root module groups
//! definition, compile, runtime, schedule, and public re-exports.

pub mod compile;
pub mod definition;
pub mod nodes;
pub mod pregel;
pub mod run_state;
pub mod runtime;
pub(crate) mod runtime_concurrency;
pub mod schedules;
pub mod tool_layer;
pub mod topology;
pub mod validation;

#[cfg(test)]
mod tests;

pub use compile::channels;
pub use compile::channels::{
    apply_channel_writes, changed_channels_for, mark_versions_seen, ArtifactKind, ArtifactRef,
    ChannelKind, ChannelSpec, ChannelState, ChannelValueType, ChannelWrite, VersionsSeen,
};
pub use compile::compiler;
pub use compile::compiler::{
    compile_graph, compile_graph_for_execution, CompiledChannel, CompiledChannelClass,
    CompiledChannelKind, CompiledEdge, CompiledGraph, CompiledNode, CompiledProcess,
    CompiledProcessMode, CompiledReducer, CompiledWriteValue, CompiledWriter,
};
pub use definition::pins;
pub use definition::pins::default_pins_for;
pub use definition::store;
pub use definition::store::{
    delete_project_graph, list_graph_catalog, list_project_graphs, list_system_graphs,
    read_project_graph, read_system_graph, save_graph_catalog_index, save_project_graph,
    save_system_graph,
};
pub use definition::types;
pub use definition::types::*;
pub use definition::upgrade;
pub use definition::upgrade::upgrade_graph;
pub use nodes::eval;
pub use nodes::eval::{evaluate_branch, evaluate_loop_condition};
pub use nodes::llm;
pub use nodes::registry as node_registry;
pub use nodes::registry::{
    builtin_node_specs, node_category_for, node_role_from_config, node_spec_for,
    ArtifactOutputSpec, NodeCategory, NodeRole, NodeSpec, PermissionKind, PermissionSpec,
    RuntimeBinding, RuntimeBindingKind, SessionResumePolicy,
};
pub use pregel::coordinator;
pub use pregel::executor;
pub use pregel::outcome;
pub use pregel::outcome::{
    ControlDirective, ExecutionMode, NodeOutcome, ReadyNode, ReduceAction, SideEffect,
    SuperstepPlan,
};
pub use pregel::runner;
pub use pregel::runner::{
    execute_run, resolve_scripts_dir, resume_run, RunOutcome, RunnerOptions, RunnerStepResult,
};
pub use run_state::{
    append_node_log, append_run_event, cancel_run_cascade, clear_pending_pregel_writes,
    create_queued_run, create_run, fail_run_active_nodes, list_run_events, list_runs,
    list_superstep_checkpoints, read_latest_superstep_checkpoint, read_pending_pregel_writes,
    read_pregel_checkpoint_tuple, read_run, read_run_detail, record_branch_decision,
    record_loop_iteration, set_node_output, set_run_paused, update_node_state,
    update_queued_run_deadline, update_run_status, write_artifact, write_pending_pregel_writes,
    write_superstep_checkpoint, ArtifactContentType, BranchDecision, GraphRef, LoopFrame,
    LoopIterationEntry, LoopIterationResult, LoopIterationState, NodeError, NodeRunStatus,
    OutputArtifact, PausedAction, PendingWrite, RunContext, RunEvent, RunPaused, RunStatus,
    SuperstepCheckpoint, SuperstepStatus, TaskGraphRun, TaskGraphRunDetail, TaskGraphRunNode,
    TaskGraphRunSummary,
};
pub use schedules::{
    claim_schedule_fire, compute_next_run_after, create_schedule, delete_schedule,
    due_planned_fire_at, list_schedules, mark_schedule_failed, mark_schedule_skipped,
    mark_schedule_triggered, patch_schedule, read_schedule, refresh_schedule_last_status,
    TaskSchedule, TaskScheduleConcurrencyPolicy, TaskScheduleCreate, TaskScheduleGraphRef,
    TaskScheduleKind, TaskScheduleMisfirePolicy, TaskSchedulePatch, TaskScheduleSpec,
    TaskScheduleState,
};
pub use tool_layer::{
    build_codebuddy_mcp_config_content, build_codebuddy_settings_json, build_codex_mcp_config_args,
    build_opencode_task_graph_config, build_pi_mcp_adapter_config_content,
    build_pi_mcp_adapter_config_content_with_options, render_runtime_tool_config,
    resolve_tool_injection_plan, McpServerToolOptions, RuntimeToolRender, ToolEndpointSpec,
    ToolInjectionPlan, ToolTransportKind, DEFAULT_BLACKBOARD_MCP_DIRECT_TOOLS, KNOWN_LLM_TOOLKITS,
    TOOLKIT_BLACKBOARD_MCP, TOOLKIT_BROWSER_USE,
};
pub use topology::{
    GraphMutationBatch, GraphMutationBatchResult, GraphMutationConflict, GraphMutationOp,
    GraphMutationRequest, GraphMutationSummary, GraphRevision,
};
pub use validation::{
    decode_graph_value_at, parse_json_source, prefix_validation_errors, validate_graph,
    validate_graph_source, validate_graph_value, validate_graph_value_at, validate_pre_run,
};
