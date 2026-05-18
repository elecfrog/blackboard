use std::path::Path;

use serde::{Deserialize, Serialize};

use super::model::RunEvent;
use super::superstep::append_run_event;
use crate::task_graph::definition::types::TaskGraphError;

pub const TOOL_LIFECYCLE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLifecycleEventKind {
    Start,
    Update,
    End,
}

impl ToolLifecycleEventKind {
    #[must_use]
    pub const fn as_run_event_kind(self) -> &'static str {
        match self {
            Self::Start => "tool_start",
            Self::Update => "tool_update",
            Self::End => "tool_end",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLifecycleStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Timeout,
    Paused,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLifecycleError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLifecycleArtifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLifecycleEventInput {
    pub superstep: u64,
    pub kind: ToolLifecycleEventKind,
    pub node_id: String,
    pub node_run_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_kind: String,
    pub attempt: u32,
    pub status: ToolLifecycleStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub input_summary: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub output_summary: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolLifecycleError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ToolLifecycleArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolLifecyclePayload {
    schema_version: u32,
    run_id: String,
    superstep: u64,
    node_id: String,
    node_run_id: String,
    tool_call_id: String,
    tool_name: String,
    tool_kind: String,
    attempt: u32,
    status: ToolLifecycleStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    input_summary: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    output_summary: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ToolLifecycleError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    artifacts: Vec<ToolLifecycleArtifact>,
}

pub fn append_tool_lifecycle_event(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    input: ToolLifecycleEventInput,
) -> Result<RunEvent, TaskGraphError> {
    let event_kind = input.kind.as_run_event_kind();
    let message = format!(
        "{} {} {}",
        input.tool_kind,
        input.tool_name,
        match input.kind {
            ToolLifecycleEventKind::Start => "started",
            ToolLifecycleEventKind::Update => "updated",
            ToolLifecycleEventKind::End => "ended",
        }
    );
    let payload = serde_json::to_value(ToolLifecyclePayload {
        schema_version: TOOL_LIFECYCLE_SCHEMA_VERSION,
        run_id: run_id.to_string(),
        superstep: input.superstep,
        node_id: input.node_id.clone(),
        node_run_id: input.node_run_id,
        tool_call_id: input.tool_call_id,
        tool_name: input.tool_name,
        tool_kind: input.tool_kind,
        attempt: input.attempt,
        status: input.status,
        started_at: input.started_at,
        ended_at: input.ended_at,
        duration_ms: input.duration_ms,
        input_summary: input.input_summary,
        output_summary: input.output_summary,
        error: input.error,
        artifacts: input.artifacts,
    })
    .unwrap();

    append_run_event(
        workspace_root,
        project,
        run_id,
        input.superstep,
        event_kind,
        Some(input.node_id),
        message,
        payload,
    )
}
