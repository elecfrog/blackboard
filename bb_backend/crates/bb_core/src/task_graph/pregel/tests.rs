use serde_json::{json, Value};

use super::writes::channel_is_available;
use super::*;
use crate::task_graph::compile::compiler::{
    compile_graph, CompiledChannel, CompiledChannelClass, CompiledChannelKind, CompiledReducer,
    START_CHANNEL,
};
use crate::task_graph::definition::types::{
    EdgeKind, NodeType, TaskGraphDefinition, TaskGraphEdge, TaskGraphError, TaskGraphInputParam,
    TaskGraphNode, TaskGraphScope,
};

#[test]
fn pregel_checkpoint_prepares_tasks_from_channel_versions() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({"intent": "go"}));

    let step = prepare_next_tasks(&compiled, &checkpoint, 1);
    assert_eq!(step.tasks.len(), 1);
    assert_eq!(step.tasks[0].node_id, "start");
    assert_eq!(step.tasks[0].triggers, vec![START_CHANNEL.to_string()]);
}

#[test]
fn apply_writes_opens_branch_channels_and_tracks_seen_versions() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let step = prepare_next_tasks(&compiled, &checkpoint, 1);
    let task = step.tasks[0].clone();
    let writes = writes_from_node_outcome(
        &compiled,
        &task,
        None,
        &["left".to_string(), "right".to_string()],
        None,
    )
    .unwrap();
    let next = apply_writes(&compiled, &checkpoint, &step.tasks, &writes, 1).unwrap();

    assert_eq!(
        next.versions_seen["start"][START_CHANNEL],
        checkpoint.channel_versions[START_CHANNEL]
    );
    assert!(next.channel_values.contains_key("branch:to:left"));
    assert!(next.channel_values.contains_key("branch:to:right"));

    let step2 = prepare_next_tasks(&compiled, &next, 2);
    let nodes = step2
        .tasks
        .iter()
        .map(|task| task.node_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(nodes, vec!["left", "right"]);
}

#[test]
fn prepare_next_tasks_replays_successful_pending_writes_without_rerun() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let step = prepare_next_tasks(&compiled, &checkpoint, 1);
    let task = step.tasks[0].clone();
    let pending = writes_from_node_outcome(
        &compiled,
        &task,
        None,
        &["left".to_string(), "right".to_string()],
        None,
    )
    .unwrap();

    let recovered = prepare_next_tasks_with_pending_writes(&compiled, &checkpoint, &pending, 1);

    assert!(recovered.tasks.is_empty());
    assert_eq!(recovered.replayed_tasks.len(), 1);
    assert_eq!(recovered.replayed_tasks[0].id, task.id);
    assert_eq!(recovered.replayed_writes.len(), pending.len());

    let next = apply_writes(
        &compiled,
        &checkpoint,
        &recovered.replayed_tasks,
        &recovered.replayed_writes,
        1,
    )
    .unwrap();
    assert!(next.channel_values.contains_key("branch:to:left"));
    assert!(next.channel_values.contains_key("branch:to:right"));
    assert_eq!(
        next.versions_seen["start"][START_CHANNEL],
        checkpoint.channel_versions[START_CHANNEL]
    );
}

#[test]
fn prepare_next_tasks_does_not_skip_task_with_only_error_pending_write() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let step = prepare_next_tasks(&compiled, &checkpoint, 1);
    let task = step.tasks[0].clone();
    let pending = vec![write(
        &task,
        "start",
        ERROR_CHANNEL,
        json!({"message": "boom"}),
    )];

    let recovered = prepare_next_tasks_with_pending_writes(&compiled, &checkpoint, &pending, 1);

    assert_eq!(recovered.tasks.len(), 1);
    assert!(recovered.replayed_tasks.is_empty());
    assert!(recovered.replayed_writes.is_empty());
}

#[test]
fn barrier_channel_waits_for_all_required_senders() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let start_step = prepare_next_tasks(&compiled, &checkpoint, 1);
    let start_writes = writes_from_node_outcome(
        &compiled,
        &start_step.tasks[0],
        None,
        &["left".to_string(), "right".to_string()],
        None,
    )
    .unwrap();
    let after_start =
        apply_writes(&compiled, &checkpoint, &start_step.tasks, &start_writes, 1).unwrap();
    let parallel_step = prepare_next_tasks(&compiled, &after_start, 2);
    let left = parallel_step
        .tasks
        .iter()
        .find(|task| task.node_id == "left")
        .unwrap()
        .clone();
    let left_writes = writes_from_node_outcome(
        &compiled,
        &left,
        Some(&json!("L")),
        &["end".to_string()],
        None,
    )
    .unwrap();
    let after_left = apply_writes(&compiled, &after_start, &[left], &left_writes, 2).unwrap();
    assert!(prepare_next_tasks(&compiled, &after_left, 3)
        .tasks
        .is_empty());

    let right = parallel_step
        .tasks
        .iter()
        .find(|task| task.node_id == "right")
        .unwrap()
        .clone();
    let right_writes = writes_from_node_outcome(
        &compiled,
        &right,
        Some(&json!("R")),
        &["end".to_string()],
        None,
    )
    .unwrap();
    let after_right = apply_writes(&compiled, &after_left, &[right], &right_writes, 3).unwrap();
    let end_step = prepare_next_tasks(&compiled, &after_right, 4);
    assert_eq!(end_step.tasks.len(), 1);
    assert_eq!(end_step.tasks[0].node_id, "end");
}

#[test]
fn push_tasks_are_prepared_from_tasks_channel_send_packets() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let mut checkpoint = initial_checkpoint(&compiled, json!({}));
    checkpoint.channel_values.insert(
        TASKS_CHANNEL.to_string(),
        json!([send_packet("left", json!({"work": 1}))]),
    );
    checkpoint
        .channel_versions
        .insert(TASKS_CHANNEL.to_string(), 2);
    checkpoint.updated_channels = vec![TASKS_CHANNEL.to_string()];

    let step = prepare_next_tasks(&compiled, &checkpoint, 2);
    let push = step
        .tasks
        .iter()
        .find(|task| task.kind == PregelTaskKind::Push)
        .expect("send packet should produce a PUSH task");
    assert_eq!(push.node_id, "left");
    assert_eq!(push.input, json!({"work": 1}));
    assert_eq!(push.triggers, vec![TASKS_CHANNEL.to_string()]);
}

#[test]
fn node_output_object_is_mapped_to_matching_state_channels() {
    let compiled = compile_graph(&stateful_graph()).unwrap();
    let checkpoint = initial_checkpoint(
        &compiled,
        json!({
            "items": ["seed"],
            "payload": {"seed": true}
        }),
    );
    let task = synthetic_task("left");

    let writes = writes_from_node_outcome(
        &compiled,
        &task,
        Some(&json!({
            "items": ["a"],
            "payload": {"x": 1},
            "ignored": true
        })),
        &[],
        None,
    )
    .unwrap();

    assert!(writes
        .iter()
        .any(|write| write.channel == "state:items" && write.value == json!(["a"])));
    assert!(writes
        .iter()
        .any(|write| write.channel == "state:payload" && write.value == json!({"x": 1})));
    assert!(!writes.iter().any(|write| write.channel == "state:ignored"));

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(next.channel_values["state:items"], json!(["seed", "a"]));
    assert_eq!(next.channel_values["state:payload"], json!({"x": 1}));
}

#[test]
fn command_output_writes_state_goto_and_send_packets() {
    let compiled = compile_graph(&stateful_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({"items": []}));
    let task = synthetic_task("left");

    let writes = writes_from_node_outcome(
        &compiled,
        &task,
        Some(&json!({
            "lg_name": "Command",
            "update": {
                "items": ["from-command"],
                "payload": {"source": "command"}
            },
            "goto": [
                "right",
                { "lg_name": "Send", "node": "right", "args": { "work": 2 } }
            ]
        })),
        &[],
        None,
    )
    .unwrap();

    assert!(writes
        .iter()
        .any(|write| write.channel == "state:items" && write.value == json!(["from-command"])));
    assert!(writes
        .iter()
        .any(|write| write.channel == "branch:to:right"));
    assert!(writes.iter().any(|write| {
        write.channel == TASKS_CHANNEL && write.value == send_packet("right", json!({"work": 2}))
    }));

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(next.channel_values["state:items"], json!(["from-command"]));

    let step = prepare_next_tasks(&compiled, &next, 2);
    assert!(step
        .tasks
        .iter()
        .any(|task| task.kind == PregelTaskKind::Pull && task.node_id == "right"));
    assert!(step.tasks.iter().any(|task| {
        task.kind == PregelTaskKind::Push
            && task.node_id == "right"
            && task.input == json!({"work": 2})
    }));
}

#[test]
fn initial_checkpoint_projects_graph_input_object_into_state_channels() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({"left": "L", "right": "R"}));

    assert_eq!(checkpoint.channel_values["state:left"], json!("L"));
    assert_eq!(checkpoint.channel_values["state:right"], json!("R"));
    assert_eq!(checkpoint.channel_versions["state:left"], 1);
}

#[test]
fn last_value_rejects_multiple_writes_in_one_step() {
    let compiled = compile_graph(&parallel_graph()).unwrap();
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let task = PregelTask {
        id: "task".to_string(),
        node_id: "left".to_string(),
        kind: PregelTaskKind::Pull,
        triggers: vec![],
        path: vec![],
        input: Value::Null,
    };
    let writes = vec![
        PregelWrite {
            task_id: task.id.clone(),
            source_node_id: "left".to_string(),
            channel: "node_outputs.left".to_string(),
            value: json!("first"),
        },
        PregelWrite {
            task_id: task.id.clone(),
            source_node_id: "left".to_string(),
            channel: "node_outputs.left".to_string(),
            value: json!("second"),
        },
    ];

    let err = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap_err();
    match err {
        TaskGraphError::ValidationFailed { errors, .. } => {
            assert_eq!(errors[0].code, "invalid_concurrent_last_value_update");
        }
        other => panic!("expected channel validation error, got {other:?}"),
    }
}

#[test]
fn topic_channel_updates_as_langgraph_pubsub_checkpoint_value() {
    let mut compiled = compile_graph(&parallel_graph()).unwrap();
    compiled.channels.insert(
        "topic:test".to_string(),
        CompiledChannel {
            name: "topic:test".to_string(),
            kind: CompiledChannelKind::Data,
            class: CompiledChannelClass::Topic {
                unique: false,
                accumulate: false,
            },
            required_senders: vec![],
        },
    );
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let task = PregelTask {
        id: "task".to_string(),
        node_id: "left".to_string(),
        kind: PregelTaskKind::Pull,
        triggers: vec![],
        path: vec![],
        input: Value::Null,
    };
    let writes = vec![
        PregelWrite {
            task_id: task.id.clone(),
            source_node_id: "left".to_string(),
            channel: "topic:test".to_string(),
            value: json!("a"),
        },
        PregelWrite {
            task_id: task.id.clone(),
            source_node_id: "right".to_string(),
            channel: "topic:test".to_string(),
            value: json!(["b", "c"]),
        },
    ];

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(next.channel_values["topic:test"], json!(["a", "b", "c"]));
    assert!(channel_is_available(&compiled, &next, "topic:test"));
}

#[test]
fn checkpoint_namespace_helpers_match_langgraph_shape() {
    assert_eq!(child_checkpoint_namespace("", "research"), "research");
    assert_eq!(
        child_checkpoint_namespace("parent", "research"),
        "parent|research"
    );
    assert_eq!(
        task_checkpoint_namespace("parent|research", "task-000001"),
        "parent|research:task-000001"
    );
}

#[test]
fn any_value_accepts_multiple_writes_and_keeps_last_value() {
    let mut compiled = compile_graph(&parallel_graph()).unwrap();
    compiled.channels.insert(
        "state:any".to_string(),
        CompiledChannel {
            name: "state:any".to_string(),
            kind: CompiledChannelKind::State,
            class: CompiledChannelClass::AnyValue,
            required_senders: vec![],
        },
    );
    let checkpoint = initial_checkpoint(&compiled, json!({}));
    let task = synthetic_task("left");
    let writes = vec![
        write(&task, "left", "state:any", json!({"v": 1})),
        write(&task, "right", "state:any", json!({"v": 2})),
    ];

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(next.channel_values["state:any"], json!({"v": 2}));
}

#[test]
fn binary_operator_aggregate_append_merges_arrays_and_values() {
    let mut compiled = compile_graph(&parallel_graph()).unwrap();
    compiled.channels.insert(
        "state:items".to_string(),
        CompiledChannel {
            name: "state:items".to_string(),
            kind: CompiledChannelKind::State,
            class: CompiledChannelClass::BinaryOperatorAggregate {
                reducer: CompiledReducer::Append,
            },
            required_senders: vec![],
        },
    );
    let checkpoint = initial_checkpoint(&compiled, json!({"items": ["seed"]}));
    let task = synthetic_task("left");
    let writes = vec![
        write(&task, "left", "state:items", json!("a")),
        write(&task, "right", "state:items", json!(["b", "c"])),
    ];

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(
        next.channel_values["state:items"],
        json!(["seed", "a", "b", "c"])
    );
}

#[test]
fn binary_operator_aggregate_merge_object_reduces_objects() {
    let mut compiled = compile_graph(&parallel_graph()).unwrap();
    compiled.channels.insert(
        "state:payload".to_string(),
        CompiledChannel {
            name: "state:payload".to_string(),
            kind: CompiledChannelKind::State,
            class: CompiledChannelClass::BinaryOperatorAggregate {
                reducer: CompiledReducer::MergeObject,
            },
            required_senders: vec![],
        },
    );
    let checkpoint = initial_checkpoint(&compiled, json!({"payload": {"a": 1}}));
    let task = synthetic_task("left");
    let writes = vec![
        write(&task, "left", "state:payload", json!({"b": 2})),
        write(&task, "right", "state:payload", json!({"a": 3})),
    ];

    let next = apply_writes(&compiled, &checkpoint, &[task], &writes, 1).unwrap();
    assert_eq!(
        next.channel_values["state:payload"],
        json!({"a": 3, "b": 2})
    );
}

fn parallel_graph() -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "pregel-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Pregel Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            node("start", NodeType::Start),
            node("left", NodeType::InputVar),
            node("right", NodeType::InputVar),
            node("end", NodeType::End),
        ],
        edges: vec![
            edge("start", "left"),
            edge("start", "right"),
            edge("left", "end"),
            edge("right", "end"),
        ],
        layout: None,
    }
}

fn stateful_graph() -> TaskGraphDefinition {
    let mut graph = parallel_graph();
    graph.id = "pregel-stateful-test".to_string();
    graph.inputs = Some(vec![
        input("items", "array<string>", json!([])),
        input("payload", "json", json!({})),
    ]);
    graph.edges = vec![
        edge("start", "left"),
        edge("left", "right"),
        edge("right", "end"),
    ];
    graph
}

fn input(id: &str, value_type: &str, default_value: Value) -> TaskGraphInputParam {
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

fn node(id: &str, node_type: NodeType) -> TaskGraphNode {
    TaskGraphNode {
        id: id.to_string(),
        node_type,
        label: id.to_string(),
        description: None,
        position: None,
        config: match node_type {
            NodeType::InputVar => json!({ "input_id": id }),
            NodeType::End => json!({ "result": "succeeded" }),
            _ => json!({}),
        },
        pins: vec![],
    }
}

fn edge(from: &str, to: &str) -> TaskGraphEdge {
    TaskGraphEdge {
        id: format!("{from}__{to}"),
        from: from.to_string(),
        to: to.to_string(),
        kind: EdgeKind::Exec,
        label: None,
        source_handle: None,
        target_handle: None,
        from_pin: None,
        to_pin: None,
    }
}

fn synthetic_task(node_id: &str) -> PregelTask {
    PregelTask {
        id: format!("task-{node_id}"),
        node_id: node_id.to_string(),
        kind: PregelTaskKind::Pull,
        triggers: vec![],
        path: vec![],
        input: Value::Null,
    }
}

fn write(task: &PregelTask, source: &str, channel: &str, value: Value) -> PregelWrite {
    PregelWrite {
        task_id: task.id.clone(),
        source_node_id: source.to_string(),
        channel: channel.to_string(),
        value,
    }
}
