//! `AgentSession` runtime — shared LLM execution/session layer.
//!
//! This module is intentionally parallel to Task Graph. Task Graph LLM nodes
//! call into `AgentSession`; direct chat can do the same later without creating
//! a graph run.

use std::path::PathBuf;

pub mod model;
mod providers;
mod runtime;
mod store;

pub use model::{
    AgentEvent, AgentEventType, AgentResult, AgentResultStatus, AgentSession, AgentSessionParent,
    AgentSessionStatus, AgentSessionSummary, TokenUsage,
};
pub use runtime::{effective_agent_startup_timeout, run_turn, AgentTurnOutcome, AgentTurnRequest};
pub use store::{
    append_event, create_session, read_events, read_session, session_dir, session_summary,
    update_session, CreateAgentSession,
};

#[derive(Debug, thiserror::Error)]
pub enum AgentSessionError {
    #[error("agent session not found: {project}/{session_id}")]
    NotFound { project: String, session_id: String },
    #[error("invalid agent session input: {0}")]
    InvalidInput(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("I/O error at {path}: {message}")]
    IoMessage { path: PathBuf, message: String },
    #[error("parse error at {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
}
