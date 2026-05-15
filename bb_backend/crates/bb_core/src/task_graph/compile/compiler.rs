//! Graph compile layer for the TaskGraph execution kernel.
//!
//! This module is intentionally about executable graph structure only. It
//! mirrors LangGraph's compile direction: raw graph definitions are lowered
//! into a Pregel-style IR of processes, channels, triggers, and writers.
//! It does not define business agent roles, prompt sources, or Agent Profile
//! behavior.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::task_graph::definition::types::{
    EdgeKind, NodeType, TaskGraphDefinition, TaskGraphEdge, TaskGraphError, TaskGraphInputParam,
    TaskGraphNode, TaskGraphValidationError,
};
use crate::task_graph::definition::upgrade::upgrade_graph;
use crate::task_graph::pregel::reserved_runtime_channels;
use crate::task_graph::validation::validate_graph;

pub const START_CHANNEL: &str = "__start__";
pub const END_CHANNEL: &str = "__end__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledGraph {
    pub graph: TaskGraphDefinition,
    pub entrypoint: String,
    pub input_channels: Vec<String>,
    pub output_channels: Vec<String>,
    pub stream_channels: Vec<String>,
    pub reserved_channels: Vec<String>,
    pub nodes: BTreeMap<String, CompiledNode>,
    pub processes: BTreeMap<String, CompiledProcess>,
    pub channels: BTreeMap<String, CompiledChannel>,
    pub trigger_to_nodes: BTreeMap<String, Vec<String>>,
    pub exec_outgoing: BTreeMap<String, Vec<CompiledEdge>>,
    pub exec_incoming: BTreeMap<String, Vec<CompiledEdge>>,
    pub data_edges: Vec<CompiledEdge>,
    pub join_nodes: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledNode {
    pub id: String,
    pub node_type: NodeType,
    pub label: String,
    pub node: TaskGraphNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_pin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_pin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledProcess {
    pub id: String,
    pub node_id: String,
    pub node_type: NodeType,
    pub mode: CompiledProcessMode,
    pub triggers: Vec<String>,
    pub read_channels: Vec<String>,
    pub writers: Vec<CompiledWriter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledProcessMode {
    Inline,
    RuntimeAdapter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledWriter {
    pub channel: String,
    pub value: CompiledWriteValue,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompiledWriteValue {
    Passthrough,
    Null,
    SourceNode { node_id: String },
    StateKey { key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledChannel {
    pub name: String,
    pub kind: CompiledChannelKind,
    #[serde(default = "default_channel_class")]
    pub class: CompiledChannelClass,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_senders: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledChannelKind {
    Ephemeral,
    Barrier,
    NodeOutput,
    Data,
    State,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum CompiledChannelClass {
    EphemeralValue {
        /// LangGraph's EphemeralValue guard. `branch:to:*` uses false in StateGraph.
        guard: bool,
    },
    LastValue,
    AnyValue,
    Topic {
        unique: bool,
        accumulate: bool,
    },
    BinaryOperatorAggregate {
        reducer: CompiledReducer,
    },
    NamedBarrierValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledReducer {
    Append,
    MergeObject,
    Sum,
}

impl CompiledGraph {
    pub fn node(&self, id: &str) -> Option<&TaskGraphNode> {
        self.nodes.get(id).map(|compiled| &compiled.node)
    }

    pub fn exec_in_degree(&self, id: &str) -> usize {
        self.exec_incoming.get(id).map(Vec::len).unwrap_or(0)
    }
}

pub fn compile_graph(graph: &TaskGraphDefinition) -> Result<CompiledGraph, TaskGraphError> {
    let mut errors = validate_graph(graph);
    errors.extend(validate_compile_contract(graph));
    if !errors.is_empty() {
        return Err(TaskGraphError::ValidationFailed {
            count: errors.len(),
            errors,
        });
    }

    let entrypoint = graph
        .nodes
        .iter()
        .find(|node| node.node_type == NodeType::Start)
        .map(|node| node.id.clone())
        .ok_or_else(|| TaskGraphError::ValidationFailed {
            count: 1,
            errors: vec![TaskGraphValidationError {
                path: "nodes".to_string(),
                code: "missing_start".to_string(),
                message: "Graph must have exactly one start node before it can be compiled"
                    .to_string(),
            }],
        })?;

    let mut nodes = BTreeMap::new();
    for node in &graph.nodes {
        nodes.insert(
            node.id.clone(),
            CompiledNode {
                id: node.id.clone(),
                node_type: node.node_type,
                label: node.label.clone(),
                node: node.clone(),
            },
        );
    }

    let mut exec_outgoing: BTreeMap<String, Vec<CompiledEdge>> = BTreeMap::new();
    let mut exec_incoming: BTreeMap<String, Vec<CompiledEdge>> = BTreeMap::new();
    let mut data_edges = Vec::new();

    for edge in &graph.edges {
        let compiled = compile_edge(edge);
        match edge.kind {
            EdgeKind::Exec => {
                exec_outgoing
                    .entry(edge.from.clone())
                    .or_default()
                    .push(compiled.clone());
                exec_incoming
                    .entry(edge.to.clone())
                    .or_default()
                    .push(compiled);
            }
            EdgeKind::Data => data_edges.push(compiled),
        }
    }

    for edges in exec_outgoing.values_mut() {
        edges.sort_by(|a, b| a.id.cmp(&b.id));
    }
    for edges in exec_incoming.values_mut() {
        edges.sort_by(|a, b| a.id.cmp(&b.id));
    }
    data_edges.sort_by(|a, b| a.id.cmp(&b.id));

    let join_nodes = exec_incoming
        .iter()
        .filter(|(_, edges)| edges.len() > 1)
        .map(|(node_id, _)| node_id.clone())
        .collect();

    let mut channels = BTreeMap::new();
    channels.insert(
        START_CHANNEL.to_string(),
        CompiledChannel {
            name: START_CHANNEL.to_string(),
            kind: CompiledChannelKind::Ephemeral,
            class: CompiledChannelClass::EphemeralValue { guard: true },
            required_senders: Vec::new(),
        },
    );
    channels.insert(
        END_CHANNEL.to_string(),
        CompiledChannel {
            name: END_CHANNEL.to_string(),
            kind: CompiledChannelKind::Ephemeral,
            class: CompiledChannelClass::EphemeralValue { guard: true },
            required_senders: Vec::new(),
        },
    );

    let mut processes = compile_processes(graph, &exec_outgoing, &exec_incoming, &data_edges);
    attach_state_channels(&mut processes, &mut channels, graph);
    attach_exec_writers_and_triggers(
        &mut processes,
        &mut channels,
        &exec_outgoing,
        &exec_incoming,
    );
    attach_data_channels(&mut processes, &mut channels, &data_edges);
    attach_output_channels(&mut processes, &mut channels);
    let trigger_to_nodes = compile_trigger_to_nodes(&processes);
    let stream_channels = processes
        .keys()
        .map(|node_id| node_output_channel(node_id))
        .collect();

    Ok(CompiledGraph {
        graph: graph.clone(),
        entrypoint,
        input_channels: vec![START_CHANNEL.to_string()],
        output_channels: vec![END_CHANNEL.to_string()],
        stream_channels,
        reserved_channels: {
            let mut channels = vec![START_CHANNEL.to_string(), END_CHANNEL.to_string()];
            channels.extend(
                reserved_runtime_channels()
                    .into_iter()
                    .map(ToOwned::to_owned),
            );
            channels.sort();
            channels.dedup();
            channels
        },
        nodes,
        processes,
        channels,
        trigger_to_nodes,
        exec_outgoing,
        exec_incoming,
        data_edges,
        join_nodes,
    })
}

fn compile_processes(
    graph: &TaskGraphDefinition,
    exec_outgoing: &BTreeMap<String, Vec<CompiledEdge>>,
    exec_incoming: &BTreeMap<String, Vec<CompiledEdge>>,
    data_edges: &[CompiledEdge],
) -> BTreeMap<String, CompiledProcess> {
    let mut processes = BTreeMap::new();
    for node in &graph.nodes {
        let mut triggers = Vec::new();
        let mut read_channels = Vec::new();
        if node.node_type == NodeType::Start {
            triggers.push(START_CHANNEL.to_string());
            read_channels.push(START_CHANNEL.to_string());
        }

        for edge in data_edges.iter().filter(|edge| edge.to == node.id) {
            read_channels.push(data_channel(&edge.from, &edge.to));
        }
        if node.node_type == NodeType::InputVar {
            if let Some(input_id) = node
                .config
                .get("input_id")
                .and_then(serde_json::Value::as_str)
            {
                read_channels.push(state_channel(input_id));
            }
        }

        dedupe_sorted(&mut triggers);
        dedupe_sorted(&mut read_channels);

        let has_runtime_io = matches!(
            node.node_type,
            NodeType::Llm | NodeType::Shell | NodeType::SubGraph
        );
        processes.insert(
            node.id.clone(),
            CompiledProcess {
                id: node.id.clone(),
                node_id: node.id.clone(),
                node_type: node.node_type,
                mode: if has_runtime_io {
                    CompiledProcessMode::RuntimeAdapter
                } else {
                    CompiledProcessMode::Inline
                },
                triggers,
                read_channels,
                writers: Vec::with_capacity(
                    exec_outgoing.get(&node.id).map(Vec::len).unwrap_or(0)
                        + exec_incoming.get(&node.id).map(Vec::len).unwrap_or(0),
                ),
            },
        );
    }
    processes
}

fn attach_state_channels(
    processes: &mut BTreeMap<String, CompiledProcess>,
    channels: &mut BTreeMap<String, CompiledChannel>,
    graph: &TaskGraphDefinition,
) {
    let input_classes = graph
        .inputs
        .as_ref()
        .map(|inputs| {
            inputs
                .iter()
                .map(|input| (input.id.clone(), channel_class_for_input(input)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    let mut input_ids = input_classes.keys().cloned().collect::<BTreeSet<_>>();
    for node in &graph.nodes {
        if node.node_type == NodeType::InputVar {
            if let Some(input_id) = node
                .config
                .get("input_id")
                .and_then(serde_json::Value::as_str)
            {
                input_ids.insert(input_id.to_string());
            }
        }
    }

    for input_id in input_ids {
        let channel = state_channel(&input_id);
        channels.insert(
            channel.clone(),
            CompiledChannel {
                name: channel.clone(),
                kind: CompiledChannelKind::State,
                class: input_classes
                    .get(&input_id)
                    .cloned()
                    .unwrap_or(CompiledChannelClass::LastValue),
                required_senders: Vec::new(),
            },
        );
        for process in processes.values_mut() {
            if process.node_type != NodeType::End {
                process.writers.push(CompiledWriter {
                    channel: channel.clone(),
                    value: CompiledWriteValue::StateKey {
                        key: input_id.clone(),
                    },
                    hidden: true,
                });
                process.writers.sort_by(|a, b| a.channel.cmp(&b.channel));
            }
        }
    }
}

fn attach_exec_writers_and_triggers(
    processes: &mut BTreeMap<String, CompiledProcess>,
    channels: &mut BTreeMap<String, CompiledChannel>,
    exec_outgoing: &BTreeMap<String, Vec<CompiledEdge>>,
    exec_incoming: &BTreeMap<String, Vec<CompiledEdge>>,
) {
    for (target, incoming) in exec_incoming {
        if incoming.len() > 1 {
            let mut starts = incoming
                .iter()
                .map(|edge| edge.from.clone())
                .collect::<Vec<_>>();
            starts.sort();
            let channel = join_channel(&starts, target);
            channels.insert(
                channel.clone(),
                CompiledChannel {
                    name: channel.clone(),
                    kind: CompiledChannelKind::Barrier,
                    class: CompiledChannelClass::NamedBarrierValue,
                    required_senders: starts.clone(),
                },
            );
            if let Some(process) = processes.get_mut(target) {
                process.triggers.push(channel.clone());
                process.read_channels.push(channel.clone());
                dedupe_sorted(&mut process.triggers);
                dedupe_sorted(&mut process.read_channels);
            }
            for source in starts {
                if let Some(process) = processes.get_mut(&source) {
                    process.writers.push(CompiledWriter {
                        channel: channel.clone(),
                        value: CompiledWriteValue::SourceNode {
                            node_id: source.clone(),
                        },
                        hidden: true,
                    });
                }
            }
        } else if let Some(edge) = incoming.first() {
            let channel = branch_channel(target);
            channels.entry(channel.clone()).or_insert(CompiledChannel {
                name: channel.clone(),
                kind: CompiledChannelKind::Ephemeral,
                class: CompiledChannelClass::EphemeralValue { guard: false },
                required_senders: Vec::new(),
            });
            if let Some(process) = processes.get_mut(target) {
                process.triggers.push(channel.clone());
                process.read_channels.push(channel.clone());
                dedupe_sorted(&mut process.triggers);
                dedupe_sorted(&mut process.read_channels);
            }
            if let Some(process) = processes.get_mut(&edge.from) {
                process.writers.push(CompiledWriter {
                    channel,
                    value: CompiledWriteValue::SourceNode {
                        node_id: edge.from.clone(),
                    },
                    hidden: true,
                });
            }
        }
    }

    for source in exec_outgoing.keys() {
        if let Some(process) = processes.get_mut(source) {
            process.writers.sort_by(|a, b| a.channel.cmp(&b.channel));
        }
    }
}

fn attach_data_channels(
    processes: &mut BTreeMap<String, CompiledProcess>,
    channels: &mut BTreeMap<String, CompiledChannel>,
    data_edges: &[CompiledEdge],
) {
    for edge in data_edges {
        let channel = data_channel(&edge.from, &edge.to);
        channels.insert(
            channel.clone(),
            CompiledChannel {
                name: channel.clone(),
                kind: CompiledChannelKind::Data,
                class: CompiledChannelClass::LastValue,
                required_senders: Vec::new(),
            },
        );
        if let Some(source) = processes.get_mut(&edge.from) {
            source.writers.push(CompiledWriter {
                channel: channel.clone(),
                value: CompiledWriteValue::Passthrough,
                hidden: true,
            });
            source.writers.sort_by(|a, b| a.channel.cmp(&b.channel));
        }
        if let Some(target) = processes.get_mut(&edge.to) {
            target.read_channels.push(channel);
            dedupe_sorted(&mut target.read_channels);
        }
    }
}

fn attach_output_channels(
    processes: &mut BTreeMap<String, CompiledProcess>,
    channels: &mut BTreeMap<String, CompiledChannel>,
) {
    for (node_id, process) in processes {
        let channel = node_output_channel(node_id);
        channels.insert(
            channel.clone(),
            CompiledChannel {
                name: channel.clone(),
                kind: CompiledChannelKind::NodeOutput,
                class: CompiledChannelClass::LastValue,
                required_senders: Vec::new(),
            },
        );
        process.writers.push(CompiledWriter {
            channel: channel.clone(),
            value: CompiledWriteValue::Passthrough,
            hidden: false,
        });
        if process.node_type == NodeType::End {
            process.writers.push(CompiledWriter {
                channel: END_CHANNEL.to_string(),
                value: CompiledWriteValue::Passthrough,
                hidden: true,
            });
        }
        process.writers.sort_by(|a, b| a.channel.cmp(&b.channel));
    }
}

fn compile_trigger_to_nodes(
    processes: &BTreeMap<String, CompiledProcess>,
) -> BTreeMap<String, Vec<String>> {
    let mut trigger_to_nodes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (node_id, process) in processes {
        for trigger in &process.triggers {
            trigger_to_nodes
                .entry(trigger.clone())
                .or_default()
                .push(node_id.clone());
        }
    }
    for node_ids in trigger_to_nodes.values_mut() {
        node_ids.sort();
    }
    trigger_to_nodes
}

fn branch_channel(target: &str) -> String {
    format!("branch:to:{target}")
}

fn join_channel(starts: &[String], target: &str) -> String {
    format!("join:{}:{target}", starts.join("+"))
}

fn data_channel(from: &str, to: &str) -> String {
    format!("data:{from}:{to}")
}

fn node_output_channel(node_id: &str) -> String {
    format!("node_outputs.{node_id}")
}

fn state_channel(input_id: &str) -> String {
    format!("state:{input_id}")
}

fn channel_class_for_input(input: &TaskGraphInputParam) -> CompiledChannelClass {
    if let Some(class) = input
        .channel_class
        .as_deref()
        .and_then(explicit_channel_class)
    {
        return class;
    }
    if let Some(reducer) = input.reducer.as_deref().and_then(explicit_reducer) {
        return CompiledChannelClass::BinaryOperatorAggregate { reducer };
    }

    match input.value_type.as_str() {
        value_type if value_type.starts_with("array<") => {
            CompiledChannelClass::BinaryOperatorAggregate {
                reducer: CompiledReducer::Append,
            }
        }
        "array" => CompiledChannelClass::BinaryOperatorAggregate {
            reducer: CompiledReducer::Append,
        },
        "object" | "json" => CompiledChannelClass::AnyValue,
        _ => CompiledChannelClass::LastValue,
    }
}

fn explicit_reducer(value: &str) -> Option<CompiledReducer> {
    match value {
        "append" => Some(CompiledReducer::Append),
        "merge_object" | "merge" => Some(CompiledReducer::MergeObject),
        "sum" => Some(CompiledReducer::Sum),
        _ => None,
    }
}

fn explicit_channel_class(value: &str) -> Option<CompiledChannelClass> {
    match value {
        "last_value" | "last" => Some(CompiledChannelClass::LastValue),
        "any_value" | "any" => Some(CompiledChannelClass::AnyValue),
        "topic" => Some(CompiledChannelClass::Topic {
            unique: false,
            accumulate: false,
        }),
        "topic_unique" => Some(CompiledChannelClass::Topic {
            unique: true,
            accumulate: false,
        }),
        "topic_accumulate" => Some(CompiledChannelClass::Topic {
            unique: false,
            accumulate: true,
        }),
        "topic_unique_accumulate" => Some(CompiledChannelClass::Topic {
            unique: true,
            accumulate: true,
        }),
        "ephemeral" => Some(CompiledChannelClass::EphemeralValue { guard: true }),
        "ephemeral_many" => Some(CompiledChannelClass::EphemeralValue { guard: false }),
        _ => None,
    }
}

fn dedupe_sorted(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_channel_class() -> CompiledChannelClass {
    CompiledChannelClass::EphemeralValue { guard: true }
}

/// Compile a graph for execution, applying Blackboard's backward-compatible
/// graph upgrade first. This keeps older saved graphs and direct test fixtures
/// runnable while still producing a strict compiled execution snapshot.
pub fn compile_graph_for_execution(
    graph: &TaskGraphDefinition,
) -> Result<CompiledGraph, TaskGraphError> {
    let mut upgraded = graph.clone();
    upgrade_graph(&mut upgraded);
    compile_graph(&upgraded)
}

fn compile_edge(edge: &TaskGraphEdge) -> CompiledEdge {
    CompiledEdge {
        id: edge.id.clone(),
        from: edge.from.clone(),
        to: edge.to.clone(),
        kind: edge.kind,
        from_pin: edge.from_pin.clone(),
        to_pin: edge.to_pin.clone(),
        source_handle: edge.source_handle.clone(),
        target_handle: edge.target_handle.clone(),
    }
}

fn validate_compile_contract(graph: &TaskGraphDefinition) -> Vec<TaskGraphValidationError> {
    let mut errors = Vec::new();
    let start_count = graph
        .nodes
        .iter()
        .filter(|node| node.node_type == NodeType::Start)
        .count();
    if start_count == 1 {
        let start_id = graph
            .nodes
            .iter()
            .find(|node| node.node_type == NodeType::Start)
            .map(|node| node.id.as_str())
            .unwrap_or_default();
        let has_exec_out = graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Exec && edge.from == start_id);
        if !has_exec_out {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[start:{start_id}]"),
                code: "start_without_exec_out".to_string(),
                message: "Compiled graph entrypoint must have at least one exec outgoing edge"
                    .to_string(),
            });
        }
    }
    if let Some(inputs) = &graph.inputs {
        for (idx, input) in inputs.iter().enumerate() {
            if let Some(reducer) = input.reducer.as_deref() {
                if explicit_reducer(reducer).is_none() {
                    errors.push(TaskGraphValidationError {
                        path: format!("inputs[{idx}].reducer"),
                        code: "invalid_reducer".to_string(),
                        message: format!(
                            "Input '{}' reducer '{}' is not supported",
                            input.id, reducer
                        ),
                    });
                }
            }
            if let Some(channel_class) = input.channel_class.as_deref() {
                if explicit_channel_class(channel_class).is_none() {
                    errors.push(TaskGraphValidationError {
                        path: format!("inputs[{idx}].channel_class"),
                        code: "invalid_channel_class".to_string(),
                        message: format!(
                            "Input '{}' channel_class '{}' is not supported",
                            input.id, channel_class
                        ),
                    });
                }
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::task_graph::definition::types::{TaskGraphScope, *};

    #[test]
    fn compile_graph_indexes_entrypoint_edges_and_joins() {
        let graph = TaskGraphDefinition {
            schema_version: 1,
            id: "compile-test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Compile Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: Some(vec![
                input("ticket", "string", json!("000060")),
                input("items", "array<string>", json!([])),
                input("payload", "json", json!({})),
                input_with_reducer("score", "number", json!(0), "sum"),
                input_with_channel_class(
                    "events",
                    "string",
                    json!(null),
                    "topic_unique_accumulate",
                ),
            ]),
            nodes: vec![
                node("start", NodeType::Start),
                node("left", NodeType::InputVar),
                node("right", NodeType::InputVar),
                node("join", NodeType::End),
            ],
            edges: vec![
                edge("start-left", "start", "left", EdgeKind::Exec),
                edge("start-right", "start", "right", EdgeKind::Exec),
                edge("left-join", "left", "join", EdgeKind::Exec),
                edge("right-join", "right", "join", EdgeKind::Exec),
                edge("left-data", "left", "join", EdgeKind::Data),
            ],
            layout: None,
        };

        let compiled = compile_graph(&graph).unwrap();
        assert_eq!(compiled.entrypoint, "start");
        assert_eq!(compiled.nodes.len(), 4);
        assert_eq!(compiled.exec_outgoing["start"].len(), 2);
        assert_eq!(compiled.exec_in_degree("join"), 2);
        assert!(compiled.join_nodes.contains("join"));
        assert_eq!(compiled.data_edges.len(), 1);

        assert_eq!(compiled.input_channels, vec![START_CHANNEL.to_string()]);
        assert_eq!(compiled.output_channels, vec![END_CHANNEL.to_string()]);
        assert!(compiled.channels.contains_key(START_CHANNEL));
        assert!(compiled.channels.contains_key(END_CHANNEL));
        assert_eq!(
            compiled.processes["start"].triggers,
            vec![START_CHANNEL.to_string()]
        );
        assert!(compiled.processes["start"]
            .writers
            .iter()
            .any(|writer| writer.channel == "branch:to:left"));
        assert!(compiled.processes["start"]
            .writers
            .iter()
            .any(|writer| writer.channel == "branch:to:right"));
        assert_eq!(
            compiled.processes["join"].triggers,
            vec!["join:left+right:join".to_string()]
        );
        assert_eq!(
            compiled.channels["join:left+right:join"].kind,
            CompiledChannelKind::Barrier
        );
        assert_eq!(
            compiled.channels["join:left+right:join"].class,
            CompiledChannelClass::NamedBarrierValue
        );
        assert_eq!(
            compiled.channels["branch:to:left"].class,
            CompiledChannelClass::EphemeralValue { guard: false }
        );
        assert_eq!(
            compiled.channels["node_outputs.left"].class,
            CompiledChannelClass::LastValue
        );
        assert_eq!(
            compiled.trigger_to_nodes["join:left+right:join"],
            vec!["join".to_string()]
        );
        assert!(compiled.processes["join"]
            .read_channels
            .contains(&"data:left:join".to_string()));
        assert!(compiled.processes["left"]
            .read_channels
            .contains(&"state:left".to_string()));
        assert_eq!(
            compiled.channels["state:ticket"].class,
            CompiledChannelClass::LastValue
        );
        assert_eq!(
            compiled.channels["state:items"].class,
            CompiledChannelClass::BinaryOperatorAggregate {
                reducer: CompiledReducer::Append
            }
        );
        assert_eq!(
            compiled.channels["state:payload"].class,
            CompiledChannelClass::AnyValue
        );
        assert_eq!(
            compiled.channels["state:score"].class,
            CompiledChannelClass::BinaryOperatorAggregate {
                reducer: CompiledReducer::Sum
            }
        );
        assert_eq!(
            compiled.channels["state:events"].class,
            CompiledChannelClass::Topic {
                unique: true,
                accumulate: true
            }
        );
        assert!(compiled.processes["left"].writers.iter().any(|writer| {
            writer.channel == "state:items"
                && writer.value
                    == CompiledWriteValue::StateKey {
                        key: "items".to_string(),
                    }
                && writer.hidden
        }));
        assert!(!compiled.processes["join"]
            .writers
            .iter()
            .any(|writer| matches!(writer.value, CompiledWriteValue::StateKey { .. })));
        assert!(compiled
            .stream_channels
            .contains(&"node_outputs.left".to_string()));
    }

    fn node(id: &str, node_type: NodeType) -> TaskGraphNode {
        let config = match node_type {
            NodeType::InputVar => json!({ "input_id": id }),
            NodeType::End => json!({ "result": "succeeded" }),
            _ => json!({}),
        };
        TaskGraphNode {
            id: id.to_string(),
            node_type,
            label: id.to_string(),
            description: None,
            position: None,
            config,
            pins: vec![],
        }
    }

    fn edge(id: &str, from: &str, to: &str, kind: EdgeKind) -> TaskGraphEdge {
        TaskGraphEdge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            kind,
            label: None,
            from_pin: None,
            to_pin: None,
            source_handle: None,
            target_handle: None,
        }
    }

    fn input(id: &str, value_type: &str, default_value: serde_json::Value) -> TaskGraphInputParam {
        TaskGraphInputParam {
            id: id.to_string(),
            label: None,
            value_type: value_type.to_string(),
            reducer: None,
            channel_class: None,
            default_value,
            description: None,
            min: None,
            max: None,
        }
    }

    fn input_with_reducer(
        id: &str,
        value_type: &str,
        default_value: serde_json::Value,
        reducer: &str,
    ) -> TaskGraphInputParam {
        TaskGraphInputParam {
            reducer: Some(reducer.to_string()),
            ..input(id, value_type, default_value)
        }
    }

    fn input_with_channel_class(
        id: &str,
        value_type: &str,
        default_value: serde_json::Value,
        channel_class: &str,
    ) -> TaskGraphInputParam {
        TaskGraphInputParam {
            channel_class: Some(channel_class.to_string()),
            ..input(id, value_type, default_value)
        }
    }
}
