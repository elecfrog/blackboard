//! Superstep planning: derive runnable PULL/PUSH tasks from checkpoint state.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::task_graph::compile::compiler::CompiledGraph;

use super::command::parse_send;
use super::model::{PregelCheckpoint, PregelPreparedStep, PregelTask, PregelTaskKind, PregelWrite};
use super::runtime_channels::{ERROR_CHANNEL, PULL_TRIGGER, PUSH_TRIGGER, TASKS_CHANNEL};
use super::writes::{channel_is_available, channel_version};

#[must_use]
pub fn prepare_next_tasks(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    next_superstep: u64,
) -> PregelPreparedStep {
    prepare_next_tasks_with_pending_writes(compiled, checkpoint, &[], next_superstep)
}
#[must_use]
pub fn prepare_next_tasks_with_pending_writes(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    pending_writes: &[PregelWrite],
    next_superstep: u64,
) -> PregelPreparedStep {
    let mut tasks = Vec::new();
    let mut replayed_tasks = Vec::new();
    let mut replayed_writes = Vec::new();

    for task in prepare_push_tasks(compiled, checkpoint, next_superstep) {
        if task_has_successful_pending_writes(pending_writes, &task.id) {
            replayed_writes.extend(pending_writes_for_task(pending_writes, &task.id));
            replayed_tasks.push(task);
        } else {
            tasks.push(task);
        }
    }

    let candidates = candidate_nodes(compiled, checkpoint);

    for node_id in candidates {
        let Some(process) = compiled.processes.get(&node_id) else {
            continue;
        };

        let Some(trigger) = process.triggers.iter().find(|channel| {
            channel_is_available(compiled, checkpoint, channel)
                && channel_version(checkpoint, channel)
                    > seen_version(checkpoint, &process.node_id, channel)
        }) else {
            continue;
        };

        let input = process_input(checkpoint, &process.read_channels);
        let task = PregelTask {
            id: format!(
                "task-{:06}-{}-pull-{}",
                next_superstep,
                process.node_id,
                stable_channel_fragment(trigger)
            ),
            node_id: process.node_id.clone(),
            kind: PregelTaskKind::Pull,
            triggers: vec![trigger.clone()],
            path: vec![
                PULL_TRIGGER.to_string(),
                process.node_id.clone(),
                trigger.clone(),
            ],
            input,
        };
        if task_has_successful_pending_writes(pending_writes, &task.id) {
            replayed_writes.extend(pending_writes_for_task(pending_writes, &task.id));
            replayed_tasks.push(task);
        } else {
            tasks.push(task);
        }
    }

    tasks.sort_by(|a, b| a.path.cmp(&b.path).then(a.node_id.cmp(&b.node_id)));
    replayed_tasks.sort_by(|a, b| a.path.cmp(&b.path).then(a.node_id.cmp(&b.node_id)));
    replayed_writes.sort_by(|a, b| {
        a.task_id
            .cmp(&b.task_id)
            .then(a.channel.cmp(&b.channel))
            .then(a.source_node_id.cmp(&b.source_node_id))
    });
    PregelPreparedStep {
        superstep: next_superstep,
        tasks,
        replayed_tasks,
        replayed_writes,
    }
}
fn candidate_nodes(compiled: &CompiledGraph, checkpoint: &PregelCheckpoint) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    if checkpoint.updated_channels.is_empty() {
        for node_id in compiled.processes.keys() {
            candidates.insert(node_id.clone());
        }
    } else {
        for channel in &checkpoint.updated_channels {
            if let Some(node_ids) = compiled.trigger_to_nodes.get(channel) {
                for node_id in node_ids {
                    candidates.insert(node_id.clone());
                }
            }
        }
    }
    candidates.into_iter().collect()
}
fn process_input(checkpoint: &PregelCheckpoint, read_channels: &[String]) -> Value {
    if read_channels.len() == 1 {
        return checkpoint
            .channel_values
            .get(&read_channels[0])
            .cloned()
            .unwrap_or(Value::Null);
    }

    let mut object = serde_json::Map::new();
    for channel in read_channels {
        if let Some(value) = checkpoint.channel_values.get(channel) {
            object.insert(channel.clone(), value.clone());
        }
    }
    Value::Object(object)
}
fn seen_version(checkpoint: &PregelCheckpoint, node_id: &str, channel: &str) -> u64 {
    checkpoint
        .versions_seen
        .get(node_id)
        .and_then(|entry| entry.get(channel))
        .copied()
        .unwrap_or(0)
}
fn task_has_successful_pending_writes(pending_writes: &[PregelWrite], task_id: &str) -> bool {
    pending_writes
        .iter()
        .any(|write| write.task_id == task_id && write.channel != ERROR_CHANNEL)
}
fn pending_writes_for_task(pending_writes: &[PregelWrite], task_id: &str) -> Vec<PregelWrite> {
    pending_writes
        .iter()
        .filter(|write| write.task_id == task_id)
        .cloned()
        .collect()
}
fn prepare_push_tasks(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    next_superstep: u64,
) -> Vec<PregelTask> {
    let Some(Value::Array(packets)) = checkpoint.channel_values.get(TASKS_CHANNEL) else {
        return Vec::new();
    };

    let mut tasks = Vec::new();
    for (index, packet) in packets.iter().enumerate() {
        let Some(send) = parse_send(packet) else {
            continue;
        };
        if !compiled.processes.contains_key(&send.node) {
            continue;
        }
        tasks.push(PregelTask {
            id: format!("task-{next_superstep:06}-{}-push-{index}", send.node),
            node_id: send.node.clone(),
            kind: PregelTaskKind::Push,
            triggers: vec![TASKS_CHANNEL.to_string()],
            path: vec![
                PUSH_TRIGGER.to_string(),
                index.to_string(),
                send.node.clone(),
            ],
            input: send.args,
        });
    }
    tasks
}
fn stable_channel_fragment(channel: &str) -> String {
    channel
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
