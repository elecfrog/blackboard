//! bb_core — Blackboard multi-project workspace kernel.
//!
//! This crate owns the filesystem layout and CRUD surface for projects,
//! tickets, and inbox notes. The heavy lifting is split into sub-modules:
//!
//! - `error`     — core error type (`BlackboardError`; `InboxError` alias retained)
//! - `fs_util`   — filesystem and JSON utility functions
//! - `platform`  — OS/machine detection helpers
//! - `workspace` — multi-project workspace management
//! - `board`     — per-project data handle (`Blackboard`/`ProjectBoard`)
//! - `ticket`    — ticket CRUD, indexing, search, maintenance
//! - `inbox`     — inbox note CRUD, indexing, search
//! - `idea_canvas` — Idea Canvas CRUD, indexing, and sticky note persistence
//! - `project`   — project metadata, lane CRUD, project index

use std::path::{Component, Path};

// ─── Module declarations ─────────────────────────────────────────────────────

pub mod agent_session;
pub mod agent_tools;
pub mod agents_config;
pub mod agents_registry;
pub mod board;
pub(crate) mod common;
pub mod error;
pub mod fs_util;
pub mod idea_canvas;
pub mod inbox;
pub mod platform;
pub mod project;
#[cfg(feature = "schema")]
pub mod schema;
pub mod skills;
pub mod task_graph;
pub mod ticket;
pub mod types;
pub mod wiki;
pub mod workspace;

// ─── Re-exports ──────────────────────────────────────────────────────────────

pub use agent_tools::{
    install_agent_tool, list_agent_tools, AgentTool, AgentToolInstallResult, AgentToolList,
    AgentToolStatus,
};
pub use agents_config::{
    disconnect_agent_connector, list_agent_connectors, sync_agent_connector, AgentConnector,
    AgentConnectorList, AgentConnectorState, AgentConnectorSyncEvent, AgentConnectorTarget,
    AgentConnectorType, AgentMcpConfigFormat, AgentSourceState,
};
pub use agents_registry::{
    list_agents, list_project_agents, AgentProfile, AgentRegistryList, AgentRegistrySourceState,
    McpServerConfig, ProjectAgentList, ProjectAgentProfile, ProjectAgentRegistration,
    RemovedProjectAgentRegistration,
};
pub use board::{Blackboard, ProjectBoard};
pub use error::{BlackboardError, InboxError};
pub use fs_util::{
    canonicalize, canonicalize_existing_dir, clean_path_string, path_to_string, read_json_file,
    slug_segment, write_file_atomic, write_json_pretty,
};
pub use inbox::render_note;
pub use project::{validate_lane_def, validate_lane_id, validate_ticket_lane};
pub use ticket::{
    validate_ticket_family, validate_ticket_id, validate_ticket_name, validate_ticket_status,
    CORE_FRONTMATTER_FIELDS, REQUIRED_METADATA_FIELDS,
};
pub use types::*;
pub use wiki::{WikiAsset, WikiContentResponse, WikiNodeKind, WikiTreeNode, WikiTreeResponse};
pub use workspace::Workspace;

// ─── Shared utility functions ────────────────────────────────────────────────

pub fn is_valid_note_name(name: &str) -> bool {
    validate_note_name(name).is_ok()
}

pub fn validate_note_name(name: &str) -> Result<String, InboxError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name {
        return Err(InboxError::InvalidName(name.to_string()));
    }
    if !trimmed.ends_with(".md") {
        return Err(InboxError::InvalidName(name.to_string()));
    }

    let path = Path::new(trimmed);
    if path.is_absolute() || path.components().count() != 1 {
        return Err(InboxError::InvalidName(name.to_string()));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) | Component::CurDir
        )
    }) {
        return Err(InboxError::InvalidName(name.to_string()));
    }

    Ok(trimmed.to_string())
}

/// Project names accept letters/numbers plus `-`; punctuation is normalized by
/// callers before registration.
pub fn validate_project_name(name: &str) -> Result<&str, InboxError> {
    let trimmed = name.trim();
    if trimmed != name || trimmed.is_empty() || trimmed.len() > 64 {
        return Err(InboxError::InvalidProjectName(name.to_string()));
    }

    let mut chars = trimmed.chars();
    let first = chars
        .next()
        .ok_or_else(|| InboxError::InvalidProjectName(name.to_string()))?;
    if !(first.is_alphanumeric()) {
        return Err(InboxError::InvalidProjectName(name.to_string()));
    }
    for ch in chars {
        if !(ch.is_alphanumeric() || ch == '-') {
            return Err(InboxError::InvalidProjectName(name.to_string()));
        }
    }

    Ok(trimmed)
}

pub(crate) fn normalize_project_name_or_uuid(name: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_dash = false;
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            normalized.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            normalized.push('-');
            last_was_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests;
