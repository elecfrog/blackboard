use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use super::model::{
    AgentEvent, AgentSession, AgentSessionParent, AgentSessionStatus, AgentSessionSummary,
    TokenUsage,
};
use super::AgentSessionError;
use crate::fs_util::{read_json_file, write_json_pretty};

#[derive(Debug, Clone)]
pub struct FinalizeStats {
    pub event_count: u64,
    pub tool_count: u64,
    pub usage: BTreeMap<String, TokenUsage>,
}

#[derive(Debug, Clone)]
pub struct CreateAgentSession {
    pub project: String,
    pub title: Option<String>,
    pub runtime: String,
    pub agent: String,
    pub model: Option<String>,
    pub variant: Option<String>,
    pub parent: Option<AgentSessionParent>,
}

pub fn create_session(
    workspace_root: &Path,
    input: CreateAgentSession,
) -> Result<AgentSession, AgentSessionError> {
    let id = generate_session_id();
    let now = Utc::now().to_rfc3339();
    let session = AgentSession {
        id: id.clone(),
        project: input.project,
        title: input.title,
        status: AgentSessionStatus::Pending,
        runtime: input.runtime,
        agent: input.agent,
        model: input.model,
        variant: input.variant,
        parent: input.parent,
        provider_session_id: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        event_count: 0,
        tool_count: 0,
        usage: BTreeMap::new(),
    };
    fs::create_dir_all(session_dir(workspace_root, &session.project, &id)).map_err(|source| {
        AgentSessionError::Io {
            path: session_dir(workspace_root, &session.project, &id),
            source,
        }
    })?;
    write_session(workspace_root, &session)?;
    write_usage(workspace_root, &session.project, &id, &session.usage)?;
    Ok(session)
}

pub fn read_session(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
) -> Result<AgentSession, AgentSessionError> {
    let path = session_path(workspace_root, project, session_id);
    if !path.exists() {
        return Err(AgentSessionError::NotFound {
            project: project.to_string(),
            session_id: session_id.to_string(),
        });
    }
    read_json_file(&path).map_err(|err| AgentSessionError::Parse {
        path,
        message: err.to_string(),
    })
}

pub fn write_session(
    workspace_root: &Path,
    session: &AgentSession,
) -> Result<(), AgentSessionError> {
    let path = session_path(workspace_root, &session.project, &session.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AgentSessionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_pretty(&path, session).map_err(|err| AgentSessionError::IoMessage {
        path,
        message: err.to_string(),
    })
}

pub fn update_session<F>(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    update: F,
) -> Result<AgentSession, AgentSessionError>
where
    F: FnOnce(&mut AgentSession),
{
    let mut session = read_session(workspace_root, project, session_id)?;
    update(&mut session);
    session.updated_at = Utc::now().to_rfc3339();
    write_session(workspace_root, &session)?;
    Ok(session)
}

pub fn update_provider_session_id(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    provider_session_id: &str,
) -> Result<(), AgentSessionError> {
    update_session(workspace_root, project, session_id, |session| {
        session.provider_session_id = Some(provider_session_id.to_string());
        session.status = AgentSessionStatus::Running;
    })?;
    Ok(())
}

pub fn finalize_session(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    status: AgentSessionStatus,
    provider_session_id: Option<String>,
    stats: FinalizeStats,
) -> Result<AgentSession, AgentSessionError> {
    let completed_at = Utc::now().to_rfc3339();
    let session = update_session(workspace_root, project, session_id, |session| {
        session.status = status;
        if provider_session_id.is_some() {
            session.provider_session_id = provider_session_id;
        }
        session.completed_at = Some(completed_at.clone());
        session.event_count = stats.event_count;
        session.tool_count = stats.tool_count;
        session.usage = stats.usage.clone();
    })?;
    write_usage(workspace_root, project, session_id, &stats.usage)?;
    Ok(session)
}

pub fn append_event(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    event: &AgentEvent,
) -> Result<(), AgentSessionError> {
    let path = events_path(workspace_root, project, session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AgentSessionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| AgentSessionError::Io {
            path: path.clone(),
            source,
        })?;
    let line = serde_json::to_string(event).map_err(|err| AgentSessionError::Parse {
        path: path.clone(),
        message: err.to_string(),
    })?;
    writeln!(file, "{line}").map_err(|source| AgentSessionError::Io { path, source })?;
    Ok(())
}

pub fn read_events(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    since: Option<u64>,
) -> Result<Vec<AgentEvent>, AgentSessionError> {
    let path = events_path(workspace_root, project, session_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&path).map_err(|source| AgentSessionError::Io {
        path: path.clone(),
        source,
    })?;
    let mut events = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| AgentSessionError::Io {
            path: path.clone(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let event: AgentEvent =
            serde_json::from_str(&line).map_err(|err| AgentSessionError::Parse {
                path: path.clone(),
                message: err.to_string(),
            })?;
        if since.map(|seq| event.seq > seq).unwrap_or(true) {
            events.push(event);
        }
    }
    Ok(events)
}

pub fn write_usage(
    workspace_root: &Path,
    project: &str,
    session_id: &str,
    usage: &BTreeMap<String, TokenUsage>,
) -> Result<(), AgentSessionError> {
    let path = usage_path(workspace_root, project, session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AgentSessionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_pretty(&path, usage).map_err(|err| AgentSessionError::IoMessage {
        path,
        message: err.to_string(),
    })
}

pub fn session_summary(session: &AgentSession) -> AgentSessionSummary {
    AgentSessionSummary::from(session)
}

pub fn session_dir(workspace_root: &Path, project: &str, session_id: &str) -> PathBuf {
    sessions_root(workspace_root, project).join(session_id)
}

pub fn events_path(workspace_root: &Path, project: &str, session_id: &str) -> PathBuf {
    session_dir(workspace_root, project, session_id).join("events.jsonl")
}

fn sessions_root(workspace_root: &Path, project: &str) -> PathBuf {
    workspace_root
        .join("runtime")
        .join("agent_sessions")
        .join(project)
}

fn session_path(workspace_root: &Path, project: &str, session_id: &str) -> PathBuf {
    session_dir(workspace_root, project, session_id).join("session.json")
}

fn usage_path(workspace_root: &Path, project: &str, session_id: &str) -> PathBuf {
    session_dir(workspace_root, project, session_id).join("usage.json")
}

fn generate_session_id() -> String {
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let suffix = &Uuid::new_v4().to_string()[..8];
    format!("as-{ts}-{suffix}")
}
