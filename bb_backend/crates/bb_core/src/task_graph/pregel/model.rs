//! Shared Pregel checkpoint, task, and write data structures.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelCheckpoint {
    pub id: String,
    pub superstep: u64,
    #[serde(default)]
    pub channel_values: BTreeMap<String, Value>,
    #[serde(default)]
    pub channel_versions: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions_seen: BTreeMap<String, BTreeMap<String, u64>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updated_channels: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PregelCheckpointConfig {
    pub thread_id: String,
    #[serde(default)]
    pub checkpoint_ns: String,
    pub checkpoint_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PregelCheckpointMetadata {
    pub source: String,
    pub step: i64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parents: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelCheckpointTuple {
    pub config: PregelCheckpointConfig,
    pub checkpoint: PregelCheckpoint,
    pub metadata: PregelCheckpointMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_config: Option<PregelCheckpointConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_writes: Vec<PregelWrite>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PregelTaskKind {
    Pull,
    Push,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelTask {
    pub id: String,
    pub node_id: String,
    pub kind: PregelTaskKind,
    pub triggers: Vec<String>,
    pub path: Vec<String>,
    pub input: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelWrite {
    pub task_id: String,
    pub source_node_id: String,
    pub channel: String,
    pub value: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelPreparedStep {
    pub superstep: u64,
    pub tasks: Vec<PregelTask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replayed_tasks: Vec<PregelTask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replayed_writes: Vec<PregelWrite>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PregelSend {
    pub node: String,
    #[serde(default)]
    pub args: Value,
}
