//! Node output lowering into Pregel writes, including StateGraph and Command MVPs.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::task_graph::compile::compiler::{
    CompiledChannelKind, CompiledGraph, CompiledWriteValue, END_CHANNEL,
};
use crate::task_graph::definition::types::{NodeType, TaskGraphError};

use super::model::{PregelSend, PregelTask, PregelWrite};
use super::runtime_channels::TASKS_CHANNEL;

pub fn send_packet(node: impl Into<String>, args: Value) -> Value {
    json!({
        "node": node.into(),
        "args": args,
    })
}
pub fn writes_from_node_outcome(
    compiled: &CompiledGraph,
    task: &PregelTask,
    output: Option<&Value>,
    next_nodes: &[String],
    end_result: Option<&str>,
) -> Result<Vec<PregelWrite>, TaskGraphError> {
    let mut writes = Vec::new();
    let source = task.node_id.clone();

    if let Some(output) = output {
        writes.push(PregelWrite {
            task_id: task.id.clone(),
            source_node_id: source.clone(),
            channel: format!("node_outputs.{source}"),
            value: output.clone(),
        });

        for edge in compiled
            .data_edges
            .iter()
            .filter(|edge| edge.from == source)
        {
            writes.push(PregelWrite {
                task_id: task.id.clone(),
                source_node_id: source.clone(),
                channel: format!("data:{}:{}", edge.from, edge.to),
                value: output.clone(),
            });
        }

        append_state_update_writes(compiled, task, &source, output, &mut writes);
        append_command_control_writes(compiled, task, &source, output, &mut writes);
    }

    for next_node in next_nodes {
        if let Some(channel) = next_node_channel(compiled, &source, next_node) {
            writes.push(PregelWrite {
                task_id: task.id.clone(),
                source_node_id: source.clone(),
                channel,
                value: Value::String(source.clone()),
            });
        }
    }

    if compiled
        .nodes
        .get(&source)
        .map(|node| node.node_type == NodeType::End)
        .unwrap_or(false)
    {
        writes.push(PregelWrite {
            task_id: task.id.clone(),
            source_node_id: source,
            channel: END_CHANNEL.to_string(),
            value: Value::String(end_result.unwrap_or("succeeded").to_string()),
        });
    }

    Ok(writes)
}
fn append_state_update_writes(
    compiled: &CompiledGraph,
    task: &PregelTask,
    source: &str,
    output: &Value,
    writes: &mut Vec<PregelWrite>,
) {
    let state_writers = state_writers_for_process(compiled, source);
    if state_writers.is_empty() {
        return;
    }

    let allowed_keys = state_writers.keys().cloned().collect::<BTreeSet<_>>();
    for (key, value) in state_update_entries(output, &allowed_keys) {
        if let Some(channel) = state_writers.get(&key) {
            writes.push(PregelWrite {
                task_id: task.id.clone(),
                source_node_id: source.to_string(),
                channel: channel.clone(),
                value,
            });
        }
    }
}
fn append_command_control_writes(
    compiled: &CompiledGraph,
    task: &PregelTask,
    source: &str,
    output: &Value,
    writes: &mut Vec<PregelWrite>,
) {
    for destination in control_destinations(output) {
        match destination {
            ControlDestination::Node(node_id) => {
                if let Some(channel) = next_node_channel(compiled, source, &node_id) {
                    writes.push(PregelWrite {
                        task_id: task.id.clone(),
                        source_node_id: source.to_string(),
                        channel,
                        value: Value::String(source.to_string()),
                    });
                }
            }
            ControlDestination::Send(send) => {
                writes.push(PregelWrite {
                    task_id: task.id.clone(),
                    source_node_id: source.to_string(),
                    channel: TASKS_CHANNEL.to_string(),
                    value: send_packet(send.node, send.args),
                });
            }
        }
    }
}
fn state_writers_for_process(compiled: &CompiledGraph, source: &str) -> BTreeMap<String, String> {
    let Some(process) = compiled.processes.get(source) else {
        return BTreeMap::new();
    };

    process
        .writers
        .iter()
        .filter_map(|writer| match &writer.value {
            CompiledWriteValue::StateKey { key }
                if compiled
                    .channels
                    .get(&writer.channel)
                    .map(|channel| channel.kind == CompiledChannelKind::State)
                    .unwrap_or(false) =>
            {
                Some((key.clone(), writer.channel.clone()))
            }
            _ => None,
        })
        .collect()
}
fn state_update_entries(output: &Value, allowed_keys: &BTreeSet<String>) -> Vec<(String, Value)> {
    if allowed_keys.is_empty() || is_send(output) {
        return Vec::new();
    }

    if is_command(output) {
        return command_update_entries(output, allowed_keys);
    }

    if let Some(items) = output.as_array() {
        if items.iter().any(|item| is_command(item)) {
            let mut entries = Vec::new();
            for item in items {
                if is_command(item) {
                    entries.extend(command_update_entries(item, allowed_keys));
                } else {
                    entries.extend(state_update_entries(item, allowed_keys));
                }
            }
            return entries;
        }
        return Vec::new();
    }

    let Some(object) = output.as_object() else {
        return Vec::new();
    };

    object
        .iter()
        .filter(|(key, _)| allowed_keys.contains(*key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
fn command_update_entries(
    command: &Value,
    allowed_keys: &BTreeSet<String>,
) -> Vec<(String, Value)> {
    if command_graph(command) == Some("__parent__") {
        return Vec::new();
    }

    let Some(update) = command.get("update") else {
        return Vec::new();
    };

    if let Some(object) = update.as_object() {
        return object
            .iter()
            .filter(|(key, _)| allowed_keys.contains(*key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
    }

    if let Some(tuples) = update.as_array() {
        if tuples.iter().all(|tuple| {
            tuple
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .is_some()
        }) {
            return tuples
                .iter()
                .filter_map(|tuple| {
                    let items = tuple.as_array()?;
                    if items.len() != 2 {
                        return None;
                    }
                    let key = items[0].as_str()?;
                    allowed_keys
                        .contains(key)
                        .then(|| (key.to_string(), items[1].clone()))
                })
                .collect();
        }
    }

    if allowed_keys.contains("__root__") {
        return vec![("__root__".to_string(), update.clone())];
    }

    Vec::new()
}
#[derive(Debug, Clone)]
enum ControlDestination {
    Node(String),
    Send(PregelSend),
}
fn control_destinations(output: &Value) -> Vec<ControlDestination> {
    if let Some(send) = parse_send(output) {
        return vec![ControlDestination::Send(send)];
    }

    if is_command(output) {
        return command_goto_destinations(output);
    }

    let Some(items) = output.as_array() else {
        return Vec::new();
    };

    let mut destinations = Vec::new();
    for item in items {
        if let Some(send) = parse_send(item) {
            destinations.push(ControlDestination::Send(send));
        } else if is_command(item) {
            destinations.extend(command_goto_destinations(item));
        }
    }
    destinations
}
fn command_goto_destinations(command: &Value) -> Vec<ControlDestination> {
    if command_graph(command) == Some("__parent__") {
        return Vec::new();
    }

    let Some(goto) = command.get("goto") else {
        return Vec::new();
    };

    goto_destinations(goto)
}
fn goto_destinations(value: &Value) -> Vec<ControlDestination> {
    if let Some(node_id) = value.as_str() {
        return vec![ControlDestination::Node(node_id.to_string())];
    }

    if let Some(send) = parse_send(value) {
        return vec![ControlDestination::Send(send)];
    }

    let Some(items) = value.as_array() else {
        return Vec::new();
    };

    let mut destinations = Vec::new();
    for item in items {
        destinations.extend(goto_destinations(item));
    }
    destinations
}
fn next_node_channel(compiled: &CompiledGraph, source: &str, next_node: &str) -> Option<String> {
    compiled
        .processes
        .get(source)
        .and_then(|process| {
            process
                .writers
                .iter()
                .find(|writer| writer_targets_node(writer.channel.as_str(), next_node))
        })
        .map(|writer| writer.channel.clone())
        .or_else(|| {
            let channel = format!("branch:to:{next_node}");
            compiled.channels.contains_key(&channel).then_some(channel)
        })
}
fn is_command(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("lg_name"))
        .and_then(Value::as_str)
        == Some("Command")
}
fn is_send(value: &Value) -> bool {
    parse_send(value).is_some()
}
fn command_graph(command: &Value) -> Option<&str> {
    command
        .as_object()
        .and_then(|object| object.get("graph"))
        .and_then(Value::as_str)
}
fn writer_targets_node(channel: &str, node_id: &str) -> bool {
    channel == format!("branch:to:{node_id}")
        || (channel.starts_with("join:") && channel.ends_with(&format!(":{node_id}")))
}
pub(super) fn parse_send(value: &Value) -> Option<PregelSend> {
    let object = value.as_object()?;
    if let Some(lg_name) = object.get("lg_name").and_then(Value::as_str) {
        if lg_name != "Send" {
            return None;
        }
    }
    let node = object.get("node")?.as_str()?.to_string();
    let args = object.get("args").cloned().unwrap_or(Value::Null);
    Some(PregelSend { node, args })
}
