//! Dataflow channel primitives for superstep execution.
//!
//! The coordinator can keep using `RunContext::node_outputs` while the MVP
//! grows. These types define the durable contract that #000061 promotes into
//! first-class channel state: typed values, deterministic merge semantics, and
//! per-node version tracking.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::task_graph::definition::types::{TaskGraphError, TaskGraphValidationError};

/// The semantic shape carried by a dataflow channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelValueType {
    String,
    Text,
    Markdown,
    Int,
    Float,
    Bool,
    Json,
    Array,
    FileRef,
    WikiRef,
    TicketRef,
    Diff,
    TestResult,
    ReviewComment,
    HandoffSummary,
    RuntimeLog,
    ArtifactRef,
    Any,
}

/// How multiple writes to the same channel are reduced at a superstep barrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    /// Keep the most recent value.
    LastValue,
    /// Append every write as a source-tagged entry.
    Topic,
    /// Append every write as reducer input; future reducers can compact it.
    Aggregate,
    /// Track which upstream nodes have arrived and expose readiness.
    Barrier,
    /// Append artifact references as source-tagged entries.
    ArtifactRef,
}

/// Static channel declaration compiled from graph edges / node contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSpec {
    pub name: String,
    pub kind: ChannelKind,
    pub value_type: ChannelValueType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reducer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub barrier_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Durable channel state after writes are applied at a barrier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub name: String,
    pub kind: ChannelKind,
    pub value_type: ChannelValueType,
    pub version: u64,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A node-produced write waiting to be reduced into channel state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelWrite {
    pub channel: String,
    pub source_node_id: String,
    pub value: Value,
}

/// Common artifact kinds produced by agent workflow nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Findings,
    Plan,
    Diff,
    TestResult,
    ReviewComment,
    HandoffSummary,
    RuntimeLog,
    WikiRef,
    TicketRef,
    FileRef,
    Other,
}

/// Stable pointer to a graph-produced artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub kind: ArtifactKind,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_node_id: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
}

/// Version snapshot seen by each node: `node_id` -> `channel_name` -> version.
pub type VersionsSeen = BTreeMap<String, BTreeMap<String, u64>>;

/// Apply pending channel writes and return the next immutable channel map.
pub fn apply_channel_writes(
    specs: &[ChannelSpec],
    states: &BTreeMap<String, ChannelState>,
    writes: &[ChannelWrite],
    now: &str,
) -> Result<BTreeMap<String, ChannelState>, TaskGraphError> {
    let spec_map: BTreeMap<&str, &ChannelSpec> = specs
        .iter()
        .map(|spec| (spec.name.as_str(), spec))
        .collect();
    let mut next = states.clone();

    for write in writes {
        let spec = spec_map
            .get(write.channel.as_str())
            .copied()
            .ok_or_else(|| {
                channel_validation_error(
                    format!("channels.{}", write.channel),
                    "unknown_channel",
                    format!("unknown channel `{}`", write.channel),
                )
            })?;

        validate_channel_value(spec.value_type, &write.value, &write.channel)?;

        let state = next
            .entry(write.channel.clone())
            .or_insert_with(|| initial_state(spec));

        match spec.kind {
            ChannelKind::LastValue => {
                state.value = write.value.clone();
            }
            ChannelKind::Topic | ChannelKind::Aggregate | ChannelKind::ArtifactRef => {
                append_entry(state, write);
            }
            ChannelKind::Barrier => {
                update_barrier_state(state, spec, write);
            }
        }

        state.kind = spec.kind;
        state.value_type = spec.value_type;
        state.version += 1;
        state.updated_by_node_id = Some(write.source_node_id.clone());
        state.updated_at = Some(now.to_string());
    }

    Ok(next)
}

/// Mark the current versions as observed by a node after it consumes channels.
pub fn mark_versions_seen(
    node_id: &str,
    states: &BTreeMap<String, ChannelState>,
    versions_seen: &mut VersionsSeen,
) {
    let entry = versions_seen.entry(node_id.to_string()).or_default();
    for (channel, state) in states {
        entry.insert(channel.clone(), state.version);
    }
}

/// Return channel names whose versions changed since `node_id` last observed them.
#[must_use]
pub fn changed_channels_for(
    node_id: &str,
    states: &BTreeMap<String, ChannelState>,
    versions_seen: &VersionsSeen,
) -> Vec<String> {
    let seen = versions_seen.get(node_id);
    states
        .iter()
        .filter_map(|(channel, state)| {
            let seen_version = seen.and_then(|entry| entry.get(channel));
            if seen_version.copied() == Some(state.version) {
                None
            } else {
                Some(channel.clone())
            }
        })
        .collect()
}

fn initial_state(spec: &ChannelSpec) -> ChannelState {
    ChannelState {
        name: spec.name.clone(),
        kind: spec.kind,
        value_type: spec.value_type,
        version: 0,
        value: initial_value(spec.kind),
        updated_by_node_id: None,
        updated_at: None,
    }
}

fn initial_value(kind: ChannelKind) -> Value {
    match kind {
        ChannelKind::LastValue => Value::Null,
        ChannelKind::Topic | ChannelKind::Aggregate | ChannelKind::ArtifactRef => json!([]),
        ChannelKind::Barrier => json!({ "ready": false, "arrived": [] }),
    }
}

fn append_entry(state: &mut ChannelState, write: &ChannelWrite) {
    let entry = json!({
        "source_node_id": write.source_node_id,
        "value": write.value,
    });

    match state.value {
        Value::Array(ref mut items) => items.push(entry),
        _ => state.value = json!([entry]),
    }
}

fn update_barrier_state(state: &mut ChannelState, spec: &ChannelSpec, write: &ChannelWrite) {
    let mut arrived = state
        .value
        .get("arrived")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    arrived.insert(write.source_node_id.clone());
    let ready = !spec.barrier_nodes.is_empty()
        && spec
            .barrier_nodes
            .iter()
            .all(|node_id| arrived.contains(node_id));
    let arrived = arrived.into_iter().collect::<Vec<_>>();
    state.value = json!({
        "ready": ready,
        "arrived": arrived,
    });
}

fn validate_channel_value(
    value_type: ChannelValueType,
    value: &Value,
    channel: &str,
) -> Result<(), TaskGraphError> {
    let valid = match value_type {
        ChannelValueType::Any | ChannelValueType::Json => true,
        ChannelValueType::String
        | ChannelValueType::Text
        | ChannelValueType::Markdown
        | ChannelValueType::Diff
        | ChannelValueType::ReviewComment
        | ChannelValueType::HandoffSummary => value.is_string(),
        ChannelValueType::RuntimeLog => value.is_string() || value.is_object() || value.is_array(),
        ChannelValueType::Int => value.as_i64().is_some() || value.as_u64().is_some(),
        ChannelValueType::Float => value.is_number(),
        ChannelValueType::Bool => value.is_boolean(),
        ChannelValueType::Array => value.is_array(),
        ChannelValueType::TestResult => value.is_object(),
        ChannelValueType::FileRef
        | ChannelValueType::WikiRef
        | ChannelValueType::TicketRef
        | ChannelValueType::ArtifactRef => value.is_string() || value.is_object(),
    };

    if valid {
        Ok(())
    } else {
        Err(channel_validation_error(
            format!("channels.{channel}.value"),
            "invalid_channel_value_type",
            format!(
                "channel `{channel}` expected value type `{}`",
                value_type_name(value_type)
            ),
        ))
    }
}

const fn value_type_name(value_type: ChannelValueType) -> &'static str {
    match value_type {
        ChannelValueType::String => "string",
        ChannelValueType::Text => "text",
        ChannelValueType::Markdown => "markdown",
        ChannelValueType::Int => "int",
        ChannelValueType::Float => "float",
        ChannelValueType::Bool => "bool",
        ChannelValueType::Json => "json",
        ChannelValueType::Array => "array",
        ChannelValueType::FileRef => "file_ref",
        ChannelValueType::WikiRef => "wiki_ref",
        ChannelValueType::TicketRef => "ticket_ref",
        ChannelValueType::Diff => "diff",
        ChannelValueType::TestResult => "test_result",
        ChannelValueType::ReviewComment => "review_comment",
        ChannelValueType::HandoffSummary => "handoff_summary",
        ChannelValueType::RuntimeLog => "runtime_log",
        ChannelValueType::ArtifactRef => "artifact_ref",
        ChannelValueType::Any => "any",
    }
}

fn channel_validation_error(path: String, code: &str, message: String) -> TaskGraphError {
    TaskGraphError::ValidationFailed {
        count: 1,
        errors: vec![TaskGraphValidationError {
            path,
            code: code.to_string(),
            message,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str, kind: ChannelKind, value_type: ChannelValueType) -> ChannelSpec {
        ChannelSpec {
            name: name.to_string(),
            kind,
            value_type,
            reducer: None,
            barrier_nodes: Vec::new(),
            description: None,
        }
    }

    #[test]
    fn last_value_write_replaces_value_and_increments_version() {
        let specs = vec![spec(
            "proposal",
            ChannelKind::LastValue,
            ChannelValueType::Markdown,
        )];
        let writes = vec![ChannelWrite {
            channel: "proposal".to_string(),
            source_node_id: "designer".to_string(),
            value: json!("contract draft"),
        }];

        let states =
            apply_channel_writes(&specs, &BTreeMap::new(), &writes, "2026-05-15T00:00:00Z")
                .unwrap();
        let state = states.get("proposal").unwrap();

        assert_eq!(state.version, 1);
        assert_eq!(state.value, json!("contract draft"));
        assert_eq!(state.updated_by_node_id.as_deref(), Some("designer"));
    }

    #[test]
    fn topic_channel_appends_source_tagged_entries() {
        let specs = vec![spec("findings", ChannelKind::Topic, ChannelValueType::Json)];
        let writes = vec![
            ChannelWrite {
                channel: "findings".to_string(),
                source_node_id: "frontend".to_string(),
                value: json!({"surface": "ui"}),
            },
            ChannelWrite {
                channel: "findings".to_string(),
                source_node_id: "backend".to_string(),
                value: json!({"surface": "api"}),
            },
        ];

        let states =
            apply_channel_writes(&specs, &BTreeMap::new(), &writes, "2026-05-15T00:00:00Z")
                .unwrap();
        let entries = states.get("findings").unwrap().value.as_array().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["source_node_id"], json!("frontend"));
        assert_eq!(entries[1]["value"]["surface"], json!("api"));
    }

    #[test]
    fn barrier_channel_reports_ready_after_all_nodes_arrive() {
        let mut barrier = spec("design_join", ChannelKind::Barrier, ChannelValueType::Any);
        barrier.barrier_nodes = vec!["frontend".to_string(), "backend".to_string()];
        let specs = vec![barrier];

        let first = apply_channel_writes(
            &specs,
            &BTreeMap::new(),
            &[ChannelWrite {
                channel: "design_join".to_string(),
                source_node_id: "frontend".to_string(),
                value: Value::Null,
            }],
            "2026-05-15T00:00:00Z",
        )
        .unwrap();
        assert_eq!(first["design_join"].value["ready"], json!(false));

        let second = apply_channel_writes(
            &specs,
            &first,
            &[ChannelWrite {
                channel: "design_join".to_string(),
                source_node_id: "backend".to_string(),
                value: Value::Null,
            }],
            "2026-05-15T00:00:01Z",
        )
        .unwrap();
        assert_eq!(second["design_join"].value["ready"], json!(true));
        assert_eq!(second["design_join"].version, 2);
    }

    #[test]
    fn invalid_type_returns_structured_validation_error() {
        let specs = vec![spec(
            "summary",
            ChannelKind::LastValue,
            ChannelValueType::Markdown,
        )];
        let err = apply_channel_writes(
            &specs,
            &BTreeMap::new(),
            &[ChannelWrite {
                channel: "summary".to_string(),
                source_node_id: "writer".to_string(),
                value: json!({"not": "markdown"}),
            }],
            "2026-05-15T00:00:00Z",
        )
        .unwrap_err();

        match err {
            TaskGraphError::ValidationFailed { errors, .. } => {
                assert_eq!(errors[0].code, "invalid_channel_value_type");
                assert_eq!(errors[0].path, "channels.summary.value");
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn versions_seen_tracks_changed_channels() {
        let specs = vec![spec("diff", ChannelKind::LastValue, ChannelValueType::Diff)];
        let states = apply_channel_writes(
            &specs,
            &BTreeMap::new(),
            &[ChannelWrite {
                channel: "diff".to_string(),
                source_node_id: "implementer".to_string(),
                value: json!("patch"),
            }],
            "2026-05-15T00:00:00Z",
        )
        .unwrap();

        let mut versions_seen = VersionsSeen::new();
        assert_eq!(
            changed_channels_for("reviewer", &states, &versions_seen),
            vec!["diff".to_string()]
        );
        mark_versions_seen("reviewer", &states, &mut versions_seen);
        assert!(changed_channels_for("reviewer", &states, &versions_seen).is_empty());
    }
}
