//! Runtime graph topology mutation support.
//!
//! This module models Pregel-style topology mutations as barrier-applied
//! requests. It deliberately does not perform disk I/O; `run_state` owns
//! persistence, while coordinator owns the barrier lifecycle.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::task_graph::compile::compiler::{
    CompiledChannelClass, CompiledGraph, CompiledReducer, CompiledWriteValue,
};
use crate::task_graph::definition::types::{
    EdgeKind, NodePin, NodeType, PinCategory, PinDirection, PinValueType, Position,
    TaskGraphDefinition, TaskGraphEdge, TaskGraphNode, TaskGraphValidationError,
};
use crate::task_graph::pregel::PregelCheckpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRevision {
    pub revision: u64,
    pub graph: TaskGraphDefinition,
    pub created_at: String,
    pub parent_revision: Option<u64>,
    pub mutation_batch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMutationRequest {
    pub id: String,
    pub source_task_id: String,
    pub source_node_id: String,
    pub op: GraphMutationOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphMutationOp {
    AddNode { node: TaskGraphNode },
    RemoveNode { node_id: String },
    AddEdge { edge: TaskGraphEdge },
    RemoveEdge { edge_id: String },
    PatchNodeConfig { node_id: String, patch: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMutationBatch {
    pub id: String,
    pub superstep: u64,
    pub base_revision: u64,
    pub requests: Vec<GraphMutationRequest>,
    pub result: GraphMutationBatchResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GraphMutationBatchResult {
    Applied {
        new_revision: u64,
        summary: GraphMutationSummary,
    },
    Rejected {
        conflicts: Vec<GraphMutationConflict>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphMutationSummary {
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub added_edges: Vec<String>,
    pub removed_edges: Vec<String>,
    pub patched_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMutationConflict {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphMutationApply {
    pub batch: GraphMutationBatch,
    pub graph: Option<TaskGraphDefinition>,
}

#[must_use]
pub fn mutation_batch_id(superstep: u64) -> String {
    format!("mutation-batch-{superstep:06}")
}

pub fn graph_mutations_from_output(
    output: Option<&Value>,
    source_task_id: &str,
    source_node_id: &str,
) -> Vec<GraphMutationRequest> {
    let Some(output) = output else {
        return Vec::new();
    };
    let Some(items) = output.get("graph_mutations").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if let Ok(mut request) = serde_json::from_value::<GraphMutationRequest>(item.clone()) {
                if request.source_task_id.is_empty() {
                    request.source_task_id = source_task_id.to_string();
                }
                if request.source_node_id.is_empty() {
                    request.source_node_id = source_node_id.to_string();
                }
                return Some(request);
            }
            let op_value = item
                .get("op")
                .cloned()
                .or_else(|| item.get("mutation").cloned())
                .unwrap_or_else(|| item.clone());
            let op = parse_graph_mutation_op(&op_value, item)?;
            Some(GraphMutationRequest {
                id: item.get("id").and_then(Value::as_str).map_or_else(
                    || format!("{source_task_id}-mutation-{index}"),
                    ToOwned::to_owned,
                ),
                source_task_id: source_task_id.to_string(),
                source_node_id: source_node_id.to_string(),
                op,
                reason: item
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect()
}

fn parse_graph_mutation_op(op: &Value, item: &Value) -> Option<GraphMutationOp> {
    if let Ok(op) = serde_json::from_value::<GraphMutationOp>(op.clone()) {
        return Some(op);
    }

    match op.get("type").and_then(Value::as_str)? {
        "add_node" => Some(GraphMutationOp::AddNode {
            node: compact_add_node(op)?,
        }),
        "add_edge" => Some(GraphMutationOp::AddEdge {
            edge: compact_add_edge(op, item)?,
        }),
        _ => None,
    }
}

fn compact_add_edge(op: &Value, item: &Value) -> Option<TaskGraphEdge> {
    let id = string_field(op, "id")
        .or_else(|| string_field(op, "edge_id"))
        .or_else(|| string_field(item, "id"))?;
    let from = string_field(op, "from").or_else(|| string_field(op, "source"))?;
    let to = string_field(op, "to").or_else(|| string_field(op, "target"))?;
    let pins = op.get("pins");

    Some(TaskGraphEdge {
        id,
        from,
        to,
        kind: op
            .get("kind")
            .and_then(|value| serde_json::from_value::<EdgeKind>(value.clone()).ok())
            .unwrap_or(EdgeKind::Exec),
        label: string_field(op, "label"),
        from_pin: string_field(op, "from_pin")
            .or_else(|| string_field(op, "source_handle"))
            .or_else(|| pins.and_then(|value| string_field(value, "exec_out"))),
        to_pin: string_field(op, "to_pin")
            .or_else(|| string_field(op, "target_handle"))
            .or_else(|| pins.and_then(|value| string_field(value, "exec_in"))),
        source_handle: string_field(op, "source_handle"),
        target_handle: string_field(op, "target_handle"),
    })
}

fn compact_add_node(op: &Value) -> Option<TaskGraphNode> {
    let id = string_field(op, "id").or_else(|| string_field(op, "node_id"))?;
    let node_type = op
        .get("node_type")
        .or_else(|| op.get("nodeType"))
        .or_else(|| op.get("kind"))
        .and_then(|value| serde_json::from_value::<NodeType>(value.clone()).ok())
        .unwrap_or(NodeType::Llm);

    Some(TaskGraphNode {
        label: string_field(op, "label").unwrap_or_else(|| id.clone()),
        description: string_field(op, "description"),
        position: op
            .get("position")
            .and_then(|value| serde_json::from_value::<Position>(value.clone()).ok()),
        config: op
            .get("config")
            .cloned()
            .unwrap_or_else(|| compact_node_config(op, node_type)),
        pins: compact_node_pins(op, node_type),
        id,
        node_type,
    })
}

fn compact_node_config(op: &Value, node_type: NodeType) -> Value {
    if node_type != NodeType::Llm {
        return json!({});
    }

    let mut config = serde_json::Map::new();
    config.insert(
        "runtime".to_string(),
        op.get("runtime")
            .cloned()
            .unwrap_or_else(|| json!("opencode")),
    );
    config.insert(
        "agent".to_string(),
        op.get("agent").cloned().unwrap_or_else(|| json!("native")),
    );
    if let Some(model) = op.get("model") {
        config.insert("model".to_string(), model.clone());
    }
    if let Some(inputs) = op.get("inputs") {
        config.insert("inputs".to_string(), inputs.clone());
    }
    if let Some(output) = op.get("output") {
        config.insert("output".to_string(), output.clone());
    }
    if let Some(prompt) = op.get("prompt") {
        config.insert("prompt".to_string(), compact_prompt(prompt));
    }
    Value::Object(config)
}

fn compact_prompt(prompt: &Value) -> Value {
    if let Some(template) = prompt.as_str() {
        return json!({
            "mode": "inline",
            "template": template,
        });
    }
    prompt.clone()
}

fn compact_node_pins(op: &Value, node_type: NodeType) -> Vec<NodePin> {
    if let Some(pins) = op
        .get("pins")
        .and_then(|value| serde_json::from_value::<Vec<NodePin>>(value.clone()).ok())
    {
        return pins;
    }

    let output_value_type = op
        .get("output")
        .and_then(|output| {
            string_field(output, "value_type").or_else(|| string_field(output, "artifact_type"))
        })
        .and_then(|value_type| {
            serde_json::from_value::<PinValueType>(Value::String(value_type)).ok()
        })
        .unwrap_or(PinValueType::Json);

    let pin_ids = op.get("pins");
    let mut pins = Vec::new();
    if node_type == NodeType::Llm || pin_ids.and_then(Value::as_object).is_some() {
        pins.push(exec_pin(
            pin_ids
                .and_then(|value| string_field(value, "exec_in"))
                .unwrap_or_else(|| "exec_in".to_string()),
            PinDirection::In,
            true,
        ));
        pins.push(exec_pin(
            pin_ids
                .and_then(|value| string_field(value, "exec_out"))
                .unwrap_or_else(|| "exec_out".to_string()),
            PinDirection::Out,
            false,
        ));
        pins.push(NodePin {
            id: pin_ids
                .and_then(|value| string_field(value, "output"))
                .unwrap_or_else(|| "output".to_string()),
            label: "Output".to_string(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(output_value_type),
            required: false,
        });
    }
    pins
}

fn exec_pin(id: String, direction: PinDirection, required: bool) -> NodePin {
    NodePin {
        label: match direction {
            PinDirection::In => "In".to_string(),
            PinDirection::Out => "Out".to_string(),
        },
        id,
        direction,
        category: PinCategory::Exec,
        value_type: None,
        required,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[must_use]
pub fn apply_mutation_requests(
    base_graph: &TaskGraphDefinition,
    base_revision: u64,
    new_revision: u64,
    superstep: u64,
    requests: Vec<GraphMutationRequest>,
) -> GraphMutationApply {
    let batch_id = mutation_batch_id(superstep);
    let conflicts = detect_conflicts(base_graph, &requests);
    if !conflicts.is_empty() {
        return GraphMutationApply {
            batch: GraphMutationBatch {
                id: batch_id,
                superstep,
                base_revision,
                requests,
                result: GraphMutationBatchResult::Rejected { conflicts },
            },
            graph: None,
        };
    }

    let mut graph = base_graph.clone();
    let mut summary = GraphMutationSummary::default();

    let mut remove_edge_ids = BTreeSet::new();
    let mut remove_node_ids = BTreeSet::new();
    let mut add_nodes = Vec::new();
    let mut patches = Vec::new();
    let mut add_edges = Vec::new();

    for request in &requests {
        match &request.op {
            GraphMutationOp::RemoveEdge { edge_id } => {
                remove_edge_ids.insert(edge_id.clone());
            }
            GraphMutationOp::RemoveNode { node_id } => {
                remove_node_ids.insert(node_id.clone());
                for edge in graph
                    .edges
                    .iter()
                    .filter(|edge| edge.from == *node_id || edge.to == *node_id)
                {
                    remove_edge_ids.insert(edge.id.clone());
                }
            }
            GraphMutationOp::AddNode { node } => add_nodes.push(node.clone()),
            GraphMutationOp::PatchNodeConfig { node_id, patch } => {
                patches.push((node_id.clone(), patch.clone()));
            }
            GraphMutationOp::AddEdge { edge } => add_edges.push(edge.clone()),
        }
    }

    if !remove_edge_ids.is_empty() {
        graph.edges.retain(|edge| {
            let remove = remove_edge_ids.contains(&edge.id);
            if remove {
                summary.removed_edges.push(edge.id.clone());
            }
            !remove
        });
    }

    if !remove_node_ids.is_empty() {
        graph.nodes.retain(|node| {
            let remove = remove_node_ids.contains(&node.id);
            if remove {
                summary.removed_nodes.push(node.id.clone());
            }
            !remove
        });
    }

    for node in add_nodes {
        summary.added_nodes.push(node.id.clone());
        graph.nodes.push(node);
    }

    for (node_id, patch) in patches {
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == node_id) {
            merge_patch(&mut node.config, &patch);
            summary.patched_nodes.push(node_id);
        }
    }

    for edge in add_edges {
        summary.added_edges.push(edge.id.clone());
        graph.edges.push(edge);
    }

    dedupe_sort(&mut summary.added_nodes);
    dedupe_sort(&mut summary.removed_nodes);
    dedupe_sort(&mut summary.added_edges);
    dedupe_sort(&mut summary.removed_edges);
    dedupe_sort(&mut summary.patched_nodes);

    GraphMutationApply {
        batch: GraphMutationBatch {
            id: batch_id,
            superstep,
            base_revision,
            requests,
            result: GraphMutationBatchResult::Applied {
                new_revision,
                summary,
            },
        },
        graph: Some(graph),
    }
}

pub fn merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(patch_object) => {
            if !target.is_object() {
                *target = Value::Object(serde_json::Map::new());
            }
            let Some(target_object) = target.as_object_mut() else {
                return;
            };
            for (key, value) in patch_object {
                if value.is_null() {
                    target_object.remove(key);
                } else {
                    let entry = target_object.entry(key.clone()).or_insert(Value::Null);
                    merge_patch(entry, value);
                }
            }
        }
        value => {
            *target = value.clone();
        }
    }
}

#[must_use]
pub fn conflict_validation_error(conflict: &GraphMutationConflict) -> TaskGraphValidationError {
    TaskGraphValidationError {
        path: conflict
            .node_id
            .as_ref()
            .map(|node_id| format!("nodes.{node_id}"))
            .or_else(|| {
                conflict
                    .edge_id
                    .as_ref()
                    .map(|edge_id| format!("edges.{edge_id}"))
            })
            .unwrap_or_else(|| "topology_mutation".to_string()),
        code: conflict.code.clone(),
        message: conflict.message.clone(),
    }
}

#[must_use]
pub fn migrate_checkpoint_channels(
    old_compiled: &CompiledGraph,
    new_compiled: &CompiledGraph,
    mut checkpoint: PregelCheckpoint,
    requests: &[GraphMutationRequest],
    completed_source_nodes: &BTreeSet<String>,
) -> PregelCheckpoint {
    let new_channels = new_compiled
        .channels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let old_channels = old_compiled
        .channels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    checkpoint
        .channel_values
        .retain(|channel, _| new_channels.contains(channel));
    checkpoint
        .channel_versions
        .retain(|channel, _| new_channels.contains(channel));
    checkpoint
        .updated_channels
        .retain(|channel| new_channels.contains(channel));
    for seen in checkpoint.versions_seen.values_mut() {
        seen.retain(|channel, _| new_channels.contains(channel));
    }

    for (channel, spec) in &new_compiled.channels {
        if !old_channels.contains(channel) {
            checkpoint
                .channel_versions
                .entry(channel.clone())
                .or_insert(0);
            if let Some(value) = zero_value_for_channel(&spec.class) {
                checkpoint
                    .channel_values
                    .entry(channel.clone())
                    .or_insert(value);
            }
        }
    }

    let mut seeded = BTreeSet::new();
    for request in requests {
        let GraphMutationOp::AddEdge { edge } = &request.op else {
            continue;
        };
        if request.source_node_id != edge.from || !completed_source_nodes.contains(&edge.from) {
            continue;
        }
        if let Some(channel) = writer_channel_for_target(new_compiled, &edge.from, &edge.to) {
            checkpoint
                .channel_values
                .insert(channel.clone(), Value::String(edge.from.clone()));
            bump_channel_version(&mut checkpoint, &channel);
            if channel_is_available(new_compiled, &checkpoint, &channel) {
                seeded.insert(channel);
            }
        }
    }
    let mut updated_channels = checkpoint
        .updated_channels
        .iter()
        .filter(|channel| new_channels.contains(*channel))
        .cloned()
        .collect::<BTreeSet<_>>();
    updated_channels.extend(seeded);
    checkpoint.updated_channels = updated_channels.into_iter().collect();
    checkpoint
}

fn zero_value_for_channel(class: &CompiledChannelClass) -> Option<Value> {
    match class {
        CompiledChannelClass::Topic { .. } => Some(Value::Array(Vec::new())),
        CompiledChannelClass::BinaryOperatorAggregate { reducer } => match reducer {
            CompiledReducer::Append => Some(Value::Array(Vec::new())),
            CompiledReducer::MergeObject => Some(Value::Object(serde_json::Map::new())),
            CompiledReducer::Sum => Some(serde_json::json!(0)),
        },
        CompiledChannelClass::NamedBarrierValue => Some(serde_json::json!({
            "ready": false,
            "arrived": [],
        })),
        CompiledChannelClass::EphemeralValue { .. }
        | CompiledChannelClass::LastValue
        | CompiledChannelClass::AnyValue => None,
    }
}

fn writer_channel_for_target(
    compiled: &CompiledGraph,
    source: &str,
    target: &str,
) -> Option<String> {
    compiled.processes.get(source).and_then(|process| {
        process
            .writers
            .iter()
            .find(|writer| {
                matches!(writer.value, CompiledWriteValue::SourceNode { .. })
                    && writer_targets_node(&writer.channel, target)
            })
            .map(|writer| writer.channel.clone())
    })
}

fn writer_targets_node(channel: &str, node_id: &str) -> bool {
    channel == format!("branch:to:{node_id}")
        || (channel.starts_with("join:") && channel.ends_with(&format!(":{node_id}")))
}

fn channel_is_available(
    compiled: &CompiledGraph,
    checkpoint: &PregelCheckpoint,
    channel: &str,
) -> bool {
    let Some(value) = checkpoint.channel_values.get(channel) else {
        return false;
    };
    match compiled.channels.get(channel).map(|channel| &channel.class) {
        Some(CompiledChannelClass::NamedBarrierValue) => {
            value.get("ready").and_then(Value::as_bool).unwrap_or(false)
        }
        Some(CompiledChannelClass::Topic { .. }) => {
            value.as_array().is_some_and(|items| !items.is_empty())
        }
        Some(_) => true,
        None => false,
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

fn detect_conflicts(
    base_graph: &TaskGraphDefinition,
    requests: &[GraphMutationRequest],
) -> Vec<GraphMutationConflict> {
    let mut conflicts = Vec::new();
    let node_ids = base_graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let edge_ids = base_graph
        .edges
        .iter()
        .map(|edge| edge.id.clone())
        .collect::<BTreeSet<_>>();

    let mut add_nodes: BTreeMap<String, Vec<&GraphMutationRequest>> = BTreeMap::new();
    let mut remove_nodes: BTreeMap<String, Vec<&GraphMutationRequest>> = BTreeMap::new();
    let mut add_edges: BTreeMap<String, Vec<&GraphMutationRequest>> = BTreeMap::new();
    let mut remove_edges: BTreeMap<String, Vec<&GraphMutationRequest>> = BTreeMap::new();
    let mut patches: BTreeMap<String, Vec<&GraphMutationRequest>> = BTreeMap::new();

    for request in requests {
        match &request.op {
            GraphMutationOp::AddNode { node } => {
                add_nodes.entry(node.id.clone()).or_default().push(request);
            }
            GraphMutationOp::RemoveNode { node_id } => {
                remove_nodes
                    .entry(node_id.clone())
                    .or_default()
                    .push(request);
            }
            GraphMutationOp::AddEdge { edge } => {
                add_edges.entry(edge.id.clone()).or_default().push(request);
            }
            GraphMutationOp::RemoveEdge { edge_id } => {
                remove_edges
                    .entry(edge_id.clone())
                    .or_default()
                    .push(request);
            }
            GraphMutationOp::PatchNodeConfig { node_id, .. } => {
                patches.entry(node_id.clone()).or_default().push(request);
            }
        }
    }

    for (node_id, requests_for_node) in &add_nodes {
        if !all_same_add_node(requests_for_node) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("multiple add_node requests for `{node_id}` have different specs"),
                requests_for_node,
                Some(node_id.clone()),
                None,
            ));
        }
        if remove_nodes.contains_key(node_id) {
            let mut combined = requests_for_node.clone();
            combined.extend(remove_nodes[node_id].iter().copied());
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!(
                    "remove_node and add_node for `{node_id}` in the same batch are not supported"
                ),
                &combined,
                Some(node_id.clone()),
                None,
            ));
        }
    }

    for (edge_id, requests_for_edge) in &add_edges {
        if !all_same_add_edge(requests_for_edge) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("multiple add_edge requests for `{edge_id}` have different specs"),
                requests_for_edge,
                None,
                Some(edge_id.clone()),
            ));
        }
        if remove_edges.contains_key(edge_id) {
            let mut combined = requests_for_edge.clone();
            combined.extend(remove_edges[edge_id].iter().copied());
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!(
                    "remove_edge and add_edge for `{edge_id}` in the same batch are not supported"
                ),
                &combined,
                None,
                Some(edge_id.clone()),
            ));
        }
    }

    for (node_id, requests_for_node) in &remove_nodes {
        if !node_ids.contains(node_id) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("remove_node target `{node_id}` does not exist"),
                requests_for_node,
                Some(node_id.clone()),
                None,
            ));
        }
    }

    for (edge_id, requests_for_edge) in &remove_edges {
        if !edge_ids.contains(edge_id) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("remove_edge target `{edge_id}` does not exist"),
                requests_for_edge,
                None,
                Some(edge_id.clone()),
            ));
        }
    }

    for (node_id, requests_for_node) in &patches {
        if !node_ids.contains(node_id) && !add_nodes.contains_key(node_id) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("patch_node_config target `{node_id}` does not exist"),
                requests_for_node,
                Some(node_id.clone()),
                None,
            ));
        }
        if remove_nodes.contains_key(node_id) {
            let mut combined = requests_for_node.clone();
            combined.extend(remove_nodes[node_id].iter().copied());
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!("remove_node and patch_node_config for `{node_id}` conflict"),
                &combined,
                Some(node_id.clone()),
                None,
            ));
        }
        detect_patch_path_conflicts(node_id, requests_for_node, &mut conflicts);
    }

    let mut candidate_nodes = node_ids;
    for node_id in remove_nodes.keys() {
        candidate_nodes.remove(node_id);
    }
    for node_id in add_nodes.keys() {
        candidate_nodes.insert(node_id.clone());
    }

    for requests_for_edge in add_edges.values() {
        let Some(GraphMutationOp::AddEdge { edge }) = requests_for_edge.first().map(|r| &r.op)
        else {
            continue;
        };
        if !candidate_nodes.contains(&edge.from) || !candidate_nodes.contains(&edge.to) {
            conflicts.push(conflict(
                "topology_mutation_conflict",
                format!(
                    "add_edge `{}` references missing endpoint {} -> {}",
                    edge.id, edge.from, edge.to
                ),
                requests_for_edge,
                None,
                Some(edge.id.clone()),
            ));
        }
    }

    conflicts
}

fn all_same_add_node(requests: &[&GraphMutationRequest]) -> bool {
    let mut specs = requests.iter().filter_map(|request| match &request.op {
        GraphMutationOp::AddNode { node } => Some(node),
        _ => None,
    });
    let Some(first) = specs.next() else {
        return true;
    };
    specs.all(|node| serde_json::to_value(node).ok() == serde_json::to_value(first).ok())
}

fn all_same_add_edge(requests: &[&GraphMutationRequest]) -> bool {
    let mut specs = requests.iter().filter_map(|request| match &request.op {
        GraphMutationOp::AddEdge { edge } => Some(edge),
        _ => None,
    });
    let Some(first) = specs.next() else {
        return true;
    };
    specs.all(|edge| serde_json::to_value(edge).ok() == serde_json::to_value(first).ok())
}

fn detect_patch_path_conflicts(
    node_id: &str,
    requests: &[&GraphMutationRequest],
    conflicts: &mut Vec<GraphMutationConflict>,
) {
    let mut paths: BTreeMap<String, (&GraphMutationRequest, Value)> = BTreeMap::new();
    for request in requests {
        let GraphMutationOp::PatchNodeConfig { patch, .. } = &request.op else {
            continue;
        };
        for (path, value) in flatten_patch_paths("", patch) {
            if let Some((existing_request, existing_value)) = paths.get(&path) {
                if existing_value != &value {
                    conflicts.push(GraphMutationConflict {
                        code: "topology_mutation_conflict".to_string(),
                        message: format!(
                            "patch_node_config requests for `{node_id}` write different values to `{path}`"
                        ),
                        request_ids: vec![existing_request.id.clone(), request.id.clone()],
                        node_id: Some(node_id.to_string()),
                        edge_id: None,
                    });
                }
            } else {
                paths.insert(path, (*request, value));
            }
        }
    }
}

fn flatten_patch_paths(prefix: &str, value: &Value) -> Vec<(String, Value)> {
    match value {
        Value::Object(object) => object
            .iter()
            .flat_map(|(key, child)| {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_patch_paths(&path, child)
            })
            .collect(),
        value => vec![(prefix.to_string(), value.clone())],
    }
}

fn conflict(
    code: impl Into<String>,
    message: impl Into<String>,
    requests: &[&GraphMutationRequest],
    node_id: Option<String>,
    edge_id: Option<String>,
) -> GraphMutationConflict {
    GraphMutationConflict {
        code: code.into(),
        message: message.into(),
        request_ids: requests.iter().map(|request| request.id.clone()).collect(),
        node_id,
        edge_id,
    }
}

fn dedupe_sort(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn graph_mutations_from_output_accepts_compact_llm_schema() {
        let output = json!({
            "graph_mutations": [
                {
                    "id": "node-scout-structure",
                    "reason": "fan out scout",
                    "op": {
                        "type": "add_node",
                        "id": "scout-structure",
                        "runtime": "opencode",
                        "agent": "native",
                        "model": "minimax-cn-coding-plan/MiniMax-M2.7-highspeed",
                        "prompt": "Output JSON only.",
                        "output": { "artifact_type": "json" },
                        "pins": {
                            "exec_in": "exec_in",
                            "exec_out": "exec_out",
                            "output": "output"
                        }
                    }
                },
                {
                    "id": "edge-scout-planner-to-scout-structure",
                    "op": {
                        "type": "add_edge",
                        "source": "scout-planner",
                        "target": "scout-structure",
                        "pins": {
                            "exec_out": "exec_out",
                            "exec_in": "exec_in"
                        }
                    }
                }
            ]
        });

        let mutations = graph_mutations_from_output(Some(&output), "task-1", "scout-planner");

        assert_eq!(mutations.len(), 2);
        assert_eq!(mutations[0].source_task_id, "task-1");
        assert_eq!(mutations[0].source_node_id, "scout-planner");
        match &mutations[0].op {
            GraphMutationOp::AddNode { node } => {
                assert_eq!(node.id, "scout-structure");
                assert_eq!(node.node_type, NodeType::Llm);
                assert_eq!(node.config["runtime"], "opencode");
                assert_eq!(
                    node.config["model"],
                    "minimax-cn-coding-plan/MiniMax-M2.7-highspeed"
                );
                assert_eq!(node.pins.len(), 3);
            }
            other => panic!("expected add_node, got {other:?}"),
        }
        match &mutations[1].op {
            GraphMutationOp::AddEdge { edge } => {
                assert_eq!(edge.from, "scout-planner");
                assert_eq!(edge.to, "scout-structure");
                assert_eq!(edge.kind, EdgeKind::Exec);
                assert_eq!(edge.from_pin.as_deref(), Some("exec_out"));
                assert_eq!(edge.to_pin.as_deref(), Some("exec_in"));
            }
            other => panic!("expected add_edge, got {other:?}"),
        }
    }
}
