//! Barrier write application and channel update semantics.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::task_graph::compile::compiler::{CompiledChannelClass, CompiledGraph, CompiledReducer};
use crate::task_graph::definition::types::{TaskGraphError, TaskGraphValidationError};

use super::interrupt::mark_interrupt_seen;
use super::model::{PregelCheckpoint, PregelTask, PregelWrite};
use super::runtime_channels::{
    ignored_write_channel, is_reserved_runtime_channel, INTERRUPT_CHANNEL, RESUME_CHANNEL,
    TASKS_CHANNEL,
};

pub fn apply_writes(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    completed_tasks: &[PregelTask],
    writes: &[PregelWrite],
    next_superstep: u64,
) -> Result<PregelCheckpoint, TaskGraphError> {
    let mut next = checkpoint.clone();
    next.superstep = next_superstep;
    next.id = format!("pregel-checkpoint-{next_superstep:06}");
    next.updated_channels.clear();
    let should_mark_interrupt_seen = writes
        .iter()
        .any(|write| write.channel == INTERRUPT_CHANNEL || write.channel == RESUME_CHANNEL);

    for task in completed_tasks {
        let entry = next.versions_seen.entry(task.node_id.clone()).or_default();
        for trigger in &task.triggers {
            if let Some(version) = checkpoint.channel_versions.get(trigger) {
                entry.insert(trigger.clone(), *version);
            }
        }
    }

    let mut updated = BTreeSet::new();
    for channel in completed_tasks
        .iter()
        .flat_map(|task| task.triggers.iter())
        .filter(|channel| !is_reserved_runtime_channel(channel))
    {
        if consume_channel(compiled, &mut next, channel) {
            bump_channel_version(&mut next, channel);
            if channel_is_available(compiled, &next, channel) {
                updated.insert(channel.clone());
            }
        }
    }

    let mut grouped: BTreeMap<String, Vec<&PregelWrite>> = BTreeMap::new();
    for write in writes {
        if ignored_write_channel(&write.channel) {
            continue;
        }
        if !compiled.channels.contains_key(&write.channel)
            && !is_reserved_runtime_channel(&write.channel)
        {
            return Err(TaskGraphError::ValidationFailed {
                count: 1,
                errors: vec![TaskGraphValidationError {
                    path: format!("channels.{}", write.channel),
                    code: "unknown_channel".to_string(),
                    message: format!("write target channel `{}` is not compiled", write.channel),
                }],
            });
        }
        grouped
            .entry(write.channel.clone())
            .or_default()
            .push(write);
    }

    for (channel, channel_writes) in grouped {
        apply_channel_update(compiled, &mut next, &channel, &channel_writes)?;
        bump_channel_version(&mut next, &channel);
        if channel_is_available(compiled, &next, &channel) {
            updated.insert(channel);
        }
    }

    next.updated_channels = updated.into_iter().collect();
    if should_mark_interrupt_seen {
        mark_interrupt_seen(&mut next);
    }
    Ok(next)
}
fn apply_channel_update(
    compiled: &CompiledGraph,
    checkpoint: &mut PregelCheckpoint,
    channel: &str,
    writes: &[&PregelWrite],
) -> Result<(), TaskGraphError> {
    if channel == TASKS_CHANNEL {
        let mut packets = checkpoint
            .channel_values
            .get(channel)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for write in writes {
            match &write.value {
                Value::Array(values) => packets.extend(values.clone()),
                value => packets.push(value.clone()),
            }
        }
        checkpoint
            .channel_values
            .insert(channel.to_string(), Value::Array(packets));
        return Ok(());
    }

    let class = compiled.channels.get(channel).map_or(
        CompiledChannelClass::EphemeralValue { guard: true },
        |channel| channel.class.clone(),
    );

    match class {
        CompiledChannelClass::NamedBarrierValue => {
            let spec =
                compiled
                    .channels
                    .get(channel)
                    .ok_or_else(|| TaskGraphError::ValidationFailed {
                        count: 1,
                        errors: vec![TaskGraphValidationError {
                            path: format!("channels.{channel}"),
                            code: "missing_barrier_channel".to_string(),
                            message: format!("barrier channel `{channel}` is missing"),
                        }],
                    })?;

            let mut arrived = checkpoint
                .channel_values
                .get(channel)
                .and_then(|value| value.get("arrived"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>();

            for write in writes {
                match &write.value {
                    Value::String(node_id) => {
                        arrived.insert(node_id.clone());
                    }
                    _ => {
                        arrived.insert(write.source_node_id.clone());
                    }
                }
            }

            let ready = !spec.required_senders.is_empty()
                && spec
                    .required_senders
                    .iter()
                    .all(|node_id| arrived.contains(node_id));
            checkpoint.channel_values.insert(
                channel.to_string(),
                json!({
                    "ready": ready,
                    "arrived": arrived.into_iter().collect::<Vec<_>>(),
                }),
            );
        }
        CompiledChannelClass::EphemeralValue { guard } => {
            if guard && writes.len() > 1 {
                return Err(channel_update_error(
                    channel,
                    "invalid_concurrent_ephemeral_update",
                    "EphemeralValue can only receive one value per step",
                ));
            }
            if let Some(last) = writes.last() {
                checkpoint
                    .channel_values
                    .insert(channel.to_string(), last.value.clone());
            }
        }
        CompiledChannelClass::LastValue => {
            if writes.len() > 1 {
                return Err(channel_update_error(
                    channel,
                    "invalid_concurrent_last_value_update",
                    "LastValue can only receive one value per step",
                ));
            }
            if let Some(last) = writes.last() {
                checkpoint
                    .channel_values
                    .insert(channel.to_string(), last.value.clone());
            }
        }
        CompiledChannelClass::AnyValue => {
            if let Some(last) = writes.last() {
                checkpoint
                    .channel_values
                    .insert(channel.to_string(), last.value.clone());
            }
        }
        CompiledChannelClass::Topic { unique, accumulate } => {
            let mut values = if accumulate {
                checkpoint
                    .channel_values
                    .get(channel)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            for write in writes {
                match &write.value {
                    Value::Array(items) => {
                        for item in items {
                            push_topic_value(&mut values, item.clone(), unique);
                        }
                    }
                    value => push_topic_value(&mut values, value.clone(), unique),
                }
            }
            checkpoint
                .channel_values
                .insert(channel.to_string(), Value::Array(values));
        }
        CompiledChannelClass::BinaryOperatorAggregate { reducer } => {
            apply_binary_operator_aggregate(checkpoint, channel, reducer, writes)?;
        }
    }

    Ok(())
}
fn consume_channel(
    compiled: &CompiledGraph,
    checkpoint: &mut PregelCheckpoint,
    channel: &str,
) -> bool {
    if channel == TASKS_CHANNEL {
        return checkpoint.channel_values.remove(channel).is_some();
    }

    let Some(spec) = compiled.channels.get(channel) else {
        return false;
    };

    match &spec.class {
        CompiledChannelClass::EphemeralValue { .. } => {
            checkpoint.channel_values.remove(channel).is_some()
        }
        CompiledChannelClass::NamedBarrierValue => {
            if channel_is_available(compiled, checkpoint, channel) {
                checkpoint.channel_values.insert(
                    channel.to_string(),
                    json!({
                        "ready": false,
                        "arrived": [],
                    }),
                );
                true
            } else {
                false
            }
        }
        CompiledChannelClass::LastValue => false,
        CompiledChannelClass::AnyValue => false,
        CompiledChannelClass::Topic {
            accumulate: false, ..
        } => checkpoint.channel_values.remove(channel).is_some(),
        CompiledChannelClass::Topic {
            accumulate: true, ..
        } => false,
        CompiledChannelClass::BinaryOperatorAggregate { .. } => false,
    }
}
pub(super) fn channel_is_available(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    channel: &str,
) -> bool {
    let Some(value) = checkpoint.channel_values.get(channel) else {
        return false;
    };
    if channel == TASKS_CHANNEL {
        return value.as_array().is_some_and(|items| !items.is_empty());
    }
    match compiled.channels.get(channel).map(|channel| &channel.class) {
        Some(CompiledChannelClass::NamedBarrierValue) => {
            value.get("ready").and_then(Value::as_bool).unwrap_or(false)
        }
        Some(CompiledChannelClass::Topic { .. }) => {
            value.as_array().is_some_and(|items| !items.is_empty())
        }
        Some(_) => true,
        None => is_reserved_runtime_channel(channel),
    }
}
fn apply_binary_operator_aggregate(
    checkpoint: &mut PregelCheckpoint,
    channel: &str,
    reducer: CompiledReducer,
    writes: &[&PregelWrite],
) -> Result<(), TaskGraphError> {
    match reducer {
        CompiledReducer::Append => {
            let mut values = checkpoint
                .channel_values
                .get(channel)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for write in writes {
                match &write.value {
                    Value::Array(items) => values.extend(items.clone()),
                    value => values.push(value.clone()),
                }
            }
            checkpoint
                .channel_values
                .insert(channel.to_string(), Value::Array(values));
        }
        CompiledReducer::MergeObject => {
            let mut object = checkpoint
                .channel_values
                .get(channel)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for write in writes {
                let Some(incoming) = write.value.as_object() else {
                    return Err(channel_update_error(
                        channel,
                        "invalid_merge_object_update",
                        "MergeObject reducer only accepts object values",
                    ));
                };
                for (key, value) in incoming {
                    object.insert(key.clone(), value.clone());
                }
            }
            checkpoint
                .channel_values
                .insert(channel.to_string(), Value::Object(object));
        }
        CompiledReducer::Sum => {
            let mut total = checkpoint
                .channel_values
                .get(channel)
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            for write in writes {
                let Some(value) = write.value.as_f64() else {
                    return Err(channel_update_error(
                        channel,
                        "invalid_sum_update",
                        "Sum reducer only accepts numeric values",
                    ));
                };
                total += value;
            }
            checkpoint
                .channel_values
                .insert(channel.to_string(), json!(total));
        }
    }
    Ok(())
}
fn push_topic_value(values: &mut Vec<Value>, value: Value, unique: bool) {
    if !unique || !values.contains(&value) {
        values.push(value);
    }
}
fn channel_update_error(channel: &str, code: &str, message: &str) -> TaskGraphError {
    TaskGraphError::ValidationFailed {
        count: 1,
        errors: vec![TaskGraphValidationError {
            path: format!("channels.{channel}"),
            code: code.to_string(),
            message: format!("channel `{channel}` update failed: {message}"),
        }],
    }
}
fn bump_channel_version(checkpoint: &mut PregelCheckpoint, channel: &str) {
    let next = checkpoint
        .channel_versions
        .values()
        .copied()
        .max()
        .unwrap_or(0)
        + 1;
    checkpoint
        .channel_versions
        .insert(channel.to_string(), next);
}
pub(super) fn channel_version(checkpoint: &PregelCheckpoint, channel: &str) -> u64 {
    checkpoint
        .channel_versions
        .get(channel)
        .copied()
        .unwrap_or(0)
}
