//! Checkpoint construction, tuple metadata, and version diff helpers.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::task_graph::compile::compiler::{CompiledGraph, START_CHANNEL};

use super::model::{
    PregelCheckpoint, PregelCheckpointConfig, PregelCheckpointMetadata, PregelCheckpointTuple,
    PregelWrite,
};
use super::runtime_channels::reserved_runtime_channels;

pub fn checkpoint_config(
    thread_id: impl Into<String>,
    checkpoint_ns: impl Into<String>,
    checkpoint_id: impl Into<String>,
) -> PregelCheckpointConfig {
    PregelCheckpointConfig {
        thread_id: thread_id.into(),
        checkpoint_ns: checkpoint_ns.into(),
        checkpoint_id: checkpoint_id.into(),
    }
}
pub fn checkpoint_metadata(
    source: impl Into<String>,
    step: i64,
    parent_config: Option<&PregelCheckpointConfig>,
) -> PregelCheckpointMetadata {
    let mut parents = BTreeMap::new();
    if let Some(parent_config) = parent_config {
        parents.insert(
            parent_config.checkpoint_ns.clone(),
            parent_config.checkpoint_id.clone(),
        );
    }
    PregelCheckpointMetadata {
        source: source.into(),
        step,
        parents,
    }
}
pub fn checkpoint_tuple(
    thread_id: impl Into<String>,
    checkpoint_ns: impl Into<String>,
    checkpoint: PregelCheckpoint,
    metadata: PregelCheckpointMetadata,
    parent_config: Option<PregelCheckpointConfig>,
    pending_writes: Vec<PregelWrite>,
) -> PregelCheckpointTuple {
    let thread_id = thread_id.into();
    let checkpoint_ns = checkpoint_ns.into();
    let checkpoint_id = checkpoint.id.clone();
    PregelCheckpointTuple {
        config: checkpoint_config(thread_id, checkpoint_ns, checkpoint_id),
        checkpoint,
        metadata,
        parent_config,
        pending_writes,
    }
}
pub fn initial_checkpoint(compiled: &CompiledGraph, input: Value) -> PregelCheckpoint {
    let mut channel_versions = BTreeMap::new();
    for channel in compiled.channels.keys() {
        channel_versions.insert(channel.clone(), 0);
    }
    for channel in reserved_runtime_channels() {
        channel_versions.entry(channel.to_string()).or_insert(0);
    }

    let mut channel_values = BTreeMap::new();
    channel_values.insert(START_CHANNEL.to_string(), input);
    channel_versions.insert(START_CHANNEL.to_string(), 1);
    if let Some(input_object) = channel_values
        .get(START_CHANNEL)
        .and_then(serde_json::Value::as_object)
        .cloned()
    {
        for (key, value) in input_object {
            let channel = format!("state:{key}");
            if compiled.channels.contains_key(&channel) {
                channel_values.insert(channel.clone(), value);
                channel_versions.insert(channel, 1);
            }
        }
    }

    PregelCheckpoint {
        id: "pregel-checkpoint-000000".to_string(),
        superstep: 0,
        graph_revision: 0,
        channel_values,
        channel_versions,
        versions_seen: BTreeMap::new(),
        updated_channels: vec![START_CHANNEL.to_string()],
    }
}
#[must_use]
pub fn new_channel_versions(
    previous: &BTreeMap<String, u64>,
    current: &BTreeMap<String, u64>,
) -> BTreeMap<String, u64> {
    current
        .iter()
        .filter(|(channel, version)| previous.get(*channel).copied().unwrap_or(0) != **version)
        .map(|(channel, version)| (channel.clone(), *version))
        .collect()
}
