//! 核心错误类型。

use std::path::PathBuf;

/// Blackboard 核心操作的统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum BlackboardError {
    #[error("could not discover Blackboard root from {0}")]
    RootNotFound(PathBuf),
    #[error("Blackboard inbox directory is missing under {0}")]
    MissingInbox(PathBuf),
    #[error("invalid inbox note name: {0}")]
    InvalidName(String),
    #[error("inbox note not found: {0}")]
    NotFound(String),
    #[error("invalid ticket status: {0}")]
    InvalidTicketStatus(String),
    #[error("ticket not found: {name}")]
    TicketNotFound { name: String },
    #[error("invalid ticket id: {0}")]
    InvalidTicketId(String),
    #[error("ticket id not found: {0}")]
    TicketIdNotFound(String),
    #[error("duplicate ticket id {id}: {matches:?}")]
    DuplicateTicketId { id: String, matches: Vec<String> },
    #[error("invalid ticket family: {0}")]
    InvalidTicketFamily(String),
    #[error("ticket write conflict: {0}")]
    TicketWriteConflict(String),
    #[error("ticket id allocation lock timed out: {0}")]
    TicketIdLockTimeout(PathBuf),
    #[error("invalid inbox note input: {0}")]
    InvalidInput(String),
    #[error("invalid project name: {0}")]
    InvalidProjectName(String),
    #[error("project not found: {0}")]
    ProjectNotFound(String),
    #[error("projects root directory is missing under {0}")]
    ProjectsRootMissing(PathBuf),
    #[error("invalid project metadata at {path}: {message}")]
    InvalidProjectMeta { path: PathBuf, message: String },
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Backward-compatible alias for callers that still import the historical name.
pub type InboxError = BlackboardError;
