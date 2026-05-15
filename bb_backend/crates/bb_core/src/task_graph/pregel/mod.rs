//! Pregel-style graph execution engine for TaskGraph.
//!
//! This module mirrors LangGraph's execution facts in Blackboard terms:
//! compiled processes subscribe to channels, tasks are prepared from channel
//! versions, writes are applied at a superstep barrier, and checkpoints carry
//! `channel_values`, `channel_versions`, and `versions_seen`.

mod checkpoint;
mod command;
pub mod coordinator;
pub mod executor;
mod interrupt;
mod loop_state;
mod model;
mod namespace;
pub mod outcome;
mod prepare;
pub mod runner;
mod runtime_channels;
mod writes;

#[cfg(test)]
mod tests;

pub use checkpoint::{
    checkpoint_config, checkpoint_metadata, checkpoint_tuple, initial_checkpoint,
    new_channel_versions,
};
pub use command::{send_packet, writes_from_node_outcome};
pub use interrupt::{interrupt_write, mark_interrupt_seen, resume_write, should_interrupt};
pub use loop_state::{PregelLoop, PregelLoopCommit, PregelLoopConfig, PregelLoopStatus};
pub use model::{
    PregelCheckpoint, PregelCheckpointConfig, PregelCheckpointMetadata, PregelCheckpointTuple,
    PregelPreparedStep, PregelSend, PregelTask, PregelTaskKind, PregelWrite,
};
pub use namespace::{
    child_checkpoint_namespace, task_checkpoint_namespace, CHECKPOINT_NAMESPACE_END,
    CHECKPOINT_NAMESPACE_SEPARATOR, DEFAULT_CHECKPOINT_NAMESPACE,
};
pub use prepare::{prepare_next_tasks, prepare_next_tasks_with_pending_writes};
pub use runner::{execute_run, resume_run, RunOutcome, RunnerOptions, RunnerStepResult};
pub use runtime_channels::{
    reserved_runtime_channels, ERROR_CHANNEL, INTERRUPT_CHANNEL, PREVIOUS_CHANNEL, PULL_TRIGGER,
    PUSH_TRIGGER, RESUME_CHANNEL, RETURN_CHANNEL, TASKS_CHANNEL,
};
pub use writes::apply_writes;
