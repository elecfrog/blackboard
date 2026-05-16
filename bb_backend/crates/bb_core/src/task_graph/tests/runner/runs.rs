//! Full runner, superstep, recovery, interrupt, and prompt rendering tests.

use crate::task_graph::definition::types::*;
use crate::task_graph::nodes::eval::render_prompt_template;
use crate::task_graph::pregel::runner::*;
use crate::task_graph::pregel::{prepare_next_tasks, writes_from_node_outcome};
use crate::task_graph::run_state::{self, GraphRef, RunStatus};
use crate::task_graph::{
    apply_channel_writes, changed_channels_for, compile_graph_for_execution, mark_versions_seen,
    node_role_from_config, node_spec_for, upgrade_graph, validate_graph, ChannelWrite, NodeRole,
    VersionsSeen,
};
use serde_json::json;
use tempfile::TempDir;

use super::fixtures::*;

// ─── Full runner dry-run tests ──────────────────────────────────────────

#[test]
fn execute_simple_linear_graph_dry_run() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Build a simple: start -> llm -> end graph.
    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "simple-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Simple Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "llm-1".to_string(),
                node_type: NodeType::Llm,
                label: "LLM".to_string(),
                description: None,
                position: None,
                config: json!({
                    "runtime": "codex",
                    "agent": "codex",
                    "prompt": { "mode": "inline", "template": "hello {{env.project}}" },
                    "output": { "artifact_type": "text", "required": false }
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-ok".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__llm-1".to_string(),
                from: "start".to_string(),
                to: "llm-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "llm-1__end-ok".to_string(),
                from: "llm-1".to_string(),
                to: "end-ok".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    // Create the run
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "simple-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    // Verify the run is now succeeded
    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Succeeded);
    assert!(final_run.current_superstep >= 2);
    assert!(final_run.last_checkpoint_id.is_some());

    let checkpoints = run_state::list_superstep_checkpoints(root, "test-project", &run.id).unwrap();
    assert!(checkpoints.len() >= 2);
    assert_eq!(
        checkpoints.last().unwrap().status,
        run_state::SuperstepStatus::Succeeded
    );
    assert!(checkpoints
        .iter()
        .any(|checkpoint| checkpoint.ready_nodes.contains(&"llm-1".to_string())));
    assert!(checkpoints.iter().any(|checkpoint| checkpoint
        .pending_writes
        .iter()
        .any(|write| write.target == "node_outputs.llm-1")));

    let events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    assert!(events.len() > checkpoints.len());
    assert!(events.iter().any(|event| event.kind == "run_started"));
    assert!(events.iter().any(|event| event.kind == "superstep_started"));
    assert!(events.iter().any(|event| event.kind == "node_started"));
    assert!(events.iter().any(|event| event.kind == "node_finished"));
    assert!(events.iter().any(|event| event.kind == "writes_committed"));
    assert!(events.iter().any(|event| event.kind == "checkpoint_saved"));
    assert!(events.iter().any(|event| event.kind == "run_completed"));
}

#[test]
fn superstep_barrier_records_parallel_pending_writes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "parallel-barrier".to_string(),
        scope: TaskGraphScope::Project,
        title: "Parallel Barrier".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "left".to_string(),
                node_type: NodeType::InputVar,
                label: "Left".to_string(),
                description: None,
                position: None,
                config: json!({ "input_id": "left" }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "right".to_string(),
                node_type: NodeType::InputVar,
                label: "Right".to_string(),
                description: None,
                position: None,
                config: json!({ "input_id": "right" }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            exec_edge("start", "left"),
            exec_edge("start", "right"),
            exec_edge("left", "end"),
            exec_edge("right", "end"),
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: graph.version,
    };
    let run = run_state::create_run(
        root,
        "test-project",
        graph_ref,
        &graph,
        json!({ "left": "L", "right": "R" }),
    )
    .unwrap();
    assert!(root
        .join("runtime/task_graph_runs/test-project")
        .join(&run.id)
        .join("graph.compiled.json")
        .exists());

    let opts = smoke_runner_opts(root, &run.id);
    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let checkpoints = run_state::list_superstep_checkpoints(root, "test-project", &run.id).unwrap();
    let parallel_step = checkpoints
        .iter()
        .find(|checkpoint| {
            checkpoint.ready_nodes.contains(&"left".to_string())
                && checkpoint.ready_nodes.contains(&"right".to_string())
        })
        .expect("parallel input nodes should share one superstep");
    assert_eq!(parallel_step.pending_writes.len(), 2);
    assert!(parallel_step
        .pending_writes
        .iter()
        .any(|write| write.target == "node_outputs.left" && write.value == json!("L")));
    assert!(parallel_step
        .pending_writes
        .iter()
        .any(|write| write.target == "node_outputs.right" && write.value == json!("R")));

    let events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    let writes_event = events
        .iter()
        .find(|event| {
            event.kind == "writes_committed" && event.superstep == parallel_step.superstep
        })
        .expect("barrier should emit writes_committed event");
    assert_eq!(writes_event.payload["count"], json!(2));
    assert!(events.iter().any(|event| event.kind == "checkpoint_saved"));
}

#[test]
fn recovery_replays_pending_start_writes_without_rerunning_start() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "recovery-replay".to_string(),
        scope: TaskGraphScope::Project,
        title: "Recovery Replay".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "left".to_string(),
                node_type: NodeType::InputVar,
                label: "Left".to_string(),
                description: None,
                position: None,
                config: json!({ "input_id": "left" }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![exec_edge("start", "left"), exec_edge("left", "end")],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: graph.version,
    };
    let run = run_state::create_run(
        root,
        "test-project",
        graph_ref,
        &graph,
        json!({ "left": "L" }),
    )
    .unwrap();
    let compiled = compile_graph_for_execution(&graph).unwrap();
    let checkpoint = run.pregel_checkpoint.clone().unwrap();
    let initial_step = prepare_next_tasks(&compiled, &checkpoint, 1);
    let start_task = initial_step
        .tasks
        .iter()
        .find(|task| task.node_id == "start")
        .unwrap();
    let pending = writes_from_node_outcome(&compiled, start_task, None, &[], None, true).unwrap();
    run_state::write_pending_pregel_writes(root, "test-project", &run.id, &pending).unwrap();

    let opts = smoke_runner_opts(root, &run.id);
    let outcome = execute_run(&opts).unwrap();

    assert!(matches!(outcome, RunOutcome::Succeeded));
    assert!(
        run_state::read_pending_pregel_writes(root, "test-project", &run.id)
            .unwrap()
            .is_empty()
    );

    let checkpoints = run_state::list_superstep_checkpoints(root, "test-project", &run.id).unwrap();
    let replay_step = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.superstep == 1)
        .expect("first superstep should replay pending start writes");
    assert!(replay_step.ready_nodes.is_empty());
    assert!(replay_step
        .pregel_checkpoint
        .as_ref()
        .unwrap()
        .channel_values
        .contains_key("branch:to:left"));

    let events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    let replay_event = events
        .iter()
        .find(|event| event.kind == "superstep_started" && event.superstep == 1)
        .expect("replayed step should be visible in event log");
    assert_eq!(replay_event.payload["replayed_tasks"], json!(["start"]));
}

#[test]
fn e2e_smoke_business_roles_supersteps_channels_and_resume() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let mut graph = swe_e2e_smoke_graph();
    upgrade_graph(&mut graph);
    let validation_errors = validate_graph(&graph);
    assert!(
        validation_errors.is_empty(),
        "smoke graph should validate: {validation_errors:?}"
    );

    let expected_roles = [
        ("explorer", NodeRole::ExplorerAgent, "findings"),
        ("implementer", NodeRole::ImplementerAgent, "diff"),
        ("verifier", NodeRole::VerifierAgent, "test_result"),
        ("reviewer", NodeRole::ReviewerAgent, "review_comments"),
        ("handoff", NodeRole::HandoffWriter, "handoff_summary"),
    ];
    for (node_id, role, artifact_name) in expected_roles {
        let node = graph.nodes.iter().find(|node| node.id == node_id).unwrap();
        assert_eq!(node_role_from_config(&node.config), Some(role));
        let spec = node_spec_for(node).unwrap();
        assert_eq!(spec.role, Some(role));
        assert!(
            spec.artifact_outputs
                .iter()
                .any(|artifact| artifact.name == artifact_name),
            "{node_id} should declare artifact `{artifact_name}`"
        );
    }

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: graph.version,
    };
    let run = run_state::create_run(
        root,
        "test-project",
        graph_ref,
        &graph,
        json!({ "ticket": "000066", "intent": "e2e smoke" }),
    )
    .unwrap();
    let opts = smoke_runner_opts(root, &run.id);

    let outcome = execute_run(&opts).unwrap();
    assert!(
        matches!(outcome, RunOutcome::Paused { ref node_id } if node_id == "human-gate"),
        "expected dry-run smoke to pause at human gate, got {outcome:?}"
    );

    let paused_detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    assert_eq!(paused_detail.run.status, RunStatus::Paused);
    assert_eq!(
        paused_detail.run.paused.as_ref().unwrap().node_id,
        "human-gate"
    );
    assert!(paused_detail.run.current_superstep >= expected_roles.len() as u64);
    assert!(paused_detail.run.last_checkpoint_id.is_some());
    for (node_id, _, _) in expected_roles {
        let state = paused_detail
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .unwrap();
        assert_eq!(state.status, run_state::NodeRunStatus::Succeeded);
        let output = paused_detail
            .run
            .context
            .node_outputs
            .get(node_id)
            .expect("role node should produce dry-run output");
        assert_eq!(output["dry_run"], json!(true));
        assert!(
            output["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains(node_id),
            "{node_id} prompt should survive into node output"
        );
    }

    let paused_checkpoints =
        run_state::list_superstep_checkpoints(root, "test-project", &run.id).unwrap();
    for (node_id, _, _) in expected_roles {
        assert!(
            paused_checkpoints
                .iter()
                .any(|checkpoint| checkpoint.ready_nodes.contains(&node_id.to_string())),
            "{node_id} should appear in a superstep checkpoint"
        );
    }
    assert!(paused_checkpoints
        .iter()
        .any(|checkpoint| checkpoint.ready_nodes.contains(&"human-gate".to_string())));
    assert!(paused_checkpoints
        .iter()
        .any(|checkpoint| checkpoint.status == run_state::SuperstepStatus::Paused));
    let paused_checkpoint = paused_checkpoints
        .iter()
        .find(|checkpoint| checkpoint.status == run_state::SuperstepStatus::Paused)
        .expect("paused checkpoint should be recorded");
    let paused_pregel = paused_checkpoint.pregel_checkpoint.as_ref().unwrap();
    assert!(paused_pregel.channel_values.contains_key("__interrupt__"));
    assert!(paused_pregel.versions_seen.contains_key("__interrupt__"));
    let paused_events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    assert!(paused_events.iter().any(|event| event.kind == "run_paused"));

    let channel_specs = swe_e2e_channel_specs();
    let states = apply_channel_writes(
        &channel_specs,
        &Default::default(),
        &[
            ChannelWrite {
                channel: "findings".to_string(),
                source_node_id: "explorer".to_string(),
                value: json!({ "files": ["src/app.rs"], "risk": "low" }),
            },
            ChannelWrite {
                channel: "diff".to_string(),
                source_node_id: "implementer".to_string(),
                value: json!("diff --git a/src/app.rs b/src/app.rs"),
            },
            ChannelWrite {
                channel: "test_result".to_string(),
                source_node_id: "verifier".to_string(),
                value: json!({ "passed": true, "commands": ["cargo test"] }),
            },
            ChannelWrite {
                channel: "review_comments".to_string(),
                source_node_id: "reviewer".to_string(),
                value: json!("No blocking issues."),
            },
            ChannelWrite {
                channel: "handoff_summary".to_string(),
                source_node_id: "handoff".to_string(),
                value: json!("Ready for user acceptance."),
            },
            ChannelWrite {
                channel: "acceptance_gate".to_string(),
                source_node_id: "reviewer".to_string(),
                value: json!("review_complete"),
            },
            ChannelWrite {
                channel: "acceptance_gate".to_string(),
                source_node_id: "handoff".to_string(),
                value: json!("handoff_complete"),
            },
        ],
        "2026-05-15T00:00:00Z",
    )
    .unwrap();
    assert_eq!(states["findings"].value.as_array().unwrap().len(), 1);
    assert_eq!(
        states["diff"].value,
        json!("diff --git a/src/app.rs b/src/app.rs")
    );
    assert_eq!(states["test_result"].value["passed"], json!(true));
    assert_eq!(states["handoff_summary"].version, 1);
    assert_eq!(states["acceptance_gate"].value["ready"], json!(true));
    let arrived = states["acceptance_gate"].value["arrived"]
        .as_array()
        .unwrap();
    assert!(arrived
        .iter()
        .any(|node_id| node_id.as_str() == Some("reviewer")));
    assert!(arrived
        .iter()
        .any(|node_id| node_id.as_str() == Some("handoff")));

    let mut versions_seen = VersionsSeen::new();
    assert_eq!(
        changed_channels_for("handoff", &states, &versions_seen).len(),
        6
    );
    mark_versions_seen("handoff", &states, &mut versions_seen);
    assert!(changed_channels_for("handoff", &states, &versions_seen).is_empty());

    let resumed = resume_run(&opts, "approve").unwrap();
    assert!(
        matches!(resumed, RunOutcome::Succeeded),
        "expected resume to finish at end node, got {resumed:?}"
    );
    let final_detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    assert_eq!(final_detail.run.status, RunStatus::Succeeded);
    assert_eq!(
        final_detail.run.context.node_outputs["human-gate"]["result"],
        json!("resume")
    );
    let final_checkpoints =
        run_state::list_superstep_checkpoints(root, "test-project", &run.id).unwrap();
    assert!(final_checkpoints
        .iter()
        .any(|checkpoint| checkpoint.status == run_state::SuperstepStatus::Succeeded));
    let final_events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    assert!(final_events
        .iter()
        .any(|event| event.kind == "run_completed"));
}

#[test]
fn interrupt_before_pauses_and_resume_runs_original_task() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "interrupt-before".to_string(),
        scope: TaskGraphScope::Project,
        title: "Interrupt Before".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: Some(GraphMetadata {
            tags: None,
            related_tickets: None,
            owner: None,
            created_by: None,
            updated_by: None,
            recursion_limit: None,
            interrupt_before: Some(vec!["llm-1".to_string()]),
            interrupt_after: None,
            run_policy: None,
        }),
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "llm-1".to_string(),
                node_type: NodeType::Llm,
                label: "LLM".to_string(),
                description: None,
                position: None,
                config: json!({
                    "runtime": "codex",
                    "agent": "codex",
                    "prompt": { "mode": "inline", "template": "hello" }
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-ok".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__llm-1".to_string(),
                from: "start".to_string(),
                to: "llm-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "llm-1__end-ok".to_string(),
                from: "llm-1".to_string(),
                to: "end-ok".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: graph.version,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();
    let opts = smoke_runner_opts(root, &run.id);

    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Paused { ref node_id } if node_id == "llm-1"));
    let interrupted = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(interrupted.status, RunStatus::Paused);
    assert_eq!(
        interrupted.paused.as_ref().unwrap().reason,
        "interrupt_before"
    );

    let events = run_state::list_run_events(root, "test-project", &run.id).unwrap();
    assert!(events.iter().any(|event| event.kind == "run_interrupted"));

    let resumed = resume_run(&opts, "resume").unwrap();
    assert!(matches!(resumed, RunOutcome::Succeeded));
    let finished = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    assert_eq!(finished.run.status, RunStatus::Succeeded);
    assert!(finished.run.context.node_outputs.contains_key("llm-1"));
}

#[test]
fn execute_branch_selects_correct_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // start -> branch -> end-a (if truthy) / end-b (default)
    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "branch-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Branch Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "branch-1".to_string(),
                node_type: NodeType::Branch,
                label: "Branch".to_string(),
                description: None,
                position: None,
                config: json!({
                    "mode": "first_match",
                    "input_ref": "$.input",
                    "rules": [
                        { "id": "go-a", "label": "A", "when": { "path": "$.flag", "op": "truthy" } },
                        { "id": "go-b", "label": "B", "when": { "op": "always" } }
                    ],
                    "default_rule_id": "go-b"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-a".to_string(),
                node_type: NodeType::End,
                label: "End A".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-b".to_string(),
                node_type: NodeType::End,
                label: "End B".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "failed" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__branch-1".to_string(),
                from: "start".to_string(),
                to: "branch-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "branch-1__end-a".to_string(),
                from: "branch-1".to_string(),
                to: "end-a".to_string(),
                kind: EdgeKind::Exec,
                label: Some("A".to_string()),
                source_handle: Some("rule:go-a".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "branch-1__end-b".to_string(),
                from: "branch-1".to_string(),
                to: "end-b".to_string(),
                kind: EdgeKind::Exec,
                label: Some("B".to_string()),
                source_handle: Some("rule:go-b".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    // Test with flag=true: should go to end-a (succeeded)
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "branch-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(
        root,
        "test-project",
        graph_ref,
        &graph,
        json!({ "flag": true }),
    )
    .unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    // Test with flag=false: should go to end-b (failed)
    let graph_ref2 = GraphRef {
        scope: TaskGraphScope::Project,
        id: "branch-test".to_string(),
        version: 1,
    };
    let run2 = run_state::create_run(
        root,
        "test-project",
        graph_ref2,
        &graph,
        json!({ "flag": false }),
    )
    .unwrap();

    let opts2 = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run2.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome2 = execute_run(&opts2).unwrap();
    assert!(matches!(outcome2, RunOutcome::Failed { .. }));
}

#[test]
fn execute_human_gate_pauses_run() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // start -> gate -> end
    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "gate-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Gate Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "gate-1".to_string(),
                node_type: NodeType::HumanGate,
                label: "Approval".to_string(),
                description: None,
                position: None,
                config: json!({
                    "title": "Approve?",
                    "instructions": "Please approve",
                    "actions": [
                        { "id": "approve", "label": "Approve", "result": "resume" },
                        { "id": "reject", "label": "Reject", "result": "cancel" }
                    ]
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-ok".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__gate-1".to_string(),
                from: "start".to_string(),
                to: "gate-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "gate-1__end-ok".to_string(),
                from: "gate-1".to_string(),
                to: "end-ok".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "gate-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    // Execute: should pause at the gate
    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Paused { .. }));

    // Verify run is paused
    let paused_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(paused_run.status, RunStatus::Paused);
    assert!(paused_run.paused.is_some());
    assert_eq!(paused_run.paused.as_ref().unwrap().node_id, "gate-1");

    // Resume with "approve" action
    let outcome2 = resume_run(&opts, "approve").unwrap();
    assert!(matches!(outcome2, RunOutcome::Succeeded));

    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Succeeded);
}

#[test]
fn execute_human_gate_reject_cancels_run() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "gate-cancel-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Gate Cancel Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "gate-1".to_string(),
                node_type: NodeType::HumanGate,
                label: "Gate".to_string(),
                description: None,
                position: None,
                config: json!({
                    "title": "Continue?",
                    "actions": [
                        { "id": "yes", "label": "Yes", "result": "resume" },
                        { "id": "no", "label": "No", "result": "cancel" }
                    ]
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end".to_string(),
                node_type: NodeType::End,
                label: "End".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__gate-1".to_string(),
                from: "start".to_string(),
                to: "gate-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "gate-1__end".to_string(),
                from: "gate-1".to_string(),
                to: "end".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "gate-cancel-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    execute_run(&opts).unwrap(); // Pauses
    let outcome = resume_run(&opts, "no").unwrap();
    assert!(matches!(outcome, RunOutcome::Cancelled));

    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Cancelled);
}

#[test]
fn execute_loop_max_iterations_reached() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // start -> loop (body: llm naturally returns via loop frame; exit: end-fail)
    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "loop-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Loop Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "loop-1".to_string(),
                node_type: NodeType::Loop,
                label: "Retry Loop".to_string(),
                description: None,
                position: None,
                config: json!({
                    "max_iterations": 2,
                    "body_entry": "body-llm",
                    "body_exit": "body-llm",
                    "on_max_iterations": "fail"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "body-llm".to_string(),
                node_type: NodeType::Llm,
                label: "Body LLM".to_string(),
                description: None,
                position: None,
                config: json!({
                    "runtime": "codex",
                    "agent": "codex",
                    "prompt": { "mode": "inline", "template": "fix {{env.project}}" }
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-fail".to_string(),
                node_type: NodeType::End,
                label: "Failed".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "failed" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__loop-1".to_string(),
                from: "start".to_string(),
                to: "loop-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__body-llm__body".to_string(),
                from: "loop-1".to_string(),
                to: "body-llm".to_string(),
                kind: EdgeKind::Exec,
                label: Some("body".to_string()),
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__end-fail__exit".to_string(),
                from: "loop-1".to_string(),
                to: "end-fail".to_string(),
                kind: EdgeKind::Exec,
                label: Some("exit".to_string()),
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "loop-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome = execute_run(&opts).unwrap();
    // Should reach end-fail after max_iterations
    assert!(matches!(outcome, RunOutcome::Failed { .. }));

    // Verify iteration state
    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Failed);

    let loop_iter = final_run
        .context
        .loop_iterations
        .iter()
        .find(|l| l.loop_node_id == "loop-1");
    assert!(loop_iter.is_some());
    let li = loop_iter.unwrap();
    assert_eq!(li.max_iterations, 2);
    assert!(li.exit_reason.is_some());
    assert_eq!(li.exit_reason.as_ref().unwrap(), "max_iterations_reached");
}

#[test]
fn execute_loop_condition_exit() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Loop with condition: will enter once (condition true), body runs in dry_run,
    // then on loop-frame return the condition check depends on output.
    // Since dry_run doesn't produce real output, the condition (> 0) will evaluate
    // on null: false after first iteration, then exit via condition_false.
    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "loop-cond-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Loop Condition Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "loop-1".to_string(),
                node_type: NodeType::Loop,
                label: "Conditional Loop".to_string(),
                description: None,
                position: None,
                config: json!({
                    "max_iterations": 5,
                    "condition": {
                        "input_ref": "$.nodes.body-llm.output",
                        "path": "$.errors.length",
                        "op": ">",
                        "value": 0
                    },
                    "body_entry": "body-llm",
                    "body_exit": "body-llm",
                    "on_max_iterations": "fail"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "body-llm".to_string(),
                node_type: NodeType::Llm,
                label: "Body".to_string(),
                description: None,
                position: None,
                config: json!({
                    "runtime": "codex",
                    "agent": "codex",
                    "prompt": { "mode": "inline", "template": "fix" }
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-ok".to_string(),
                node_type: NodeType::End,
                label: "OK".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-fail".to_string(),
                node_type: NodeType::End,
                label: "Fail".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "failed" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__loop-1".to_string(),
                from: "start".to_string(),
                to: "loop-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__body-llm__body".to_string(),
                from: "loop-1".to_string(),
                to: "body-llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__end-ok__exit".to_string(),
                from: "loop-1".to_string(),
                to: "end-ok".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__end-fail__exit2".to_string(),
                from: "loop-1".to_string(),
                to: "end-fail".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "loop-cond-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome = execute_run(&opts).unwrap();
    // After first iteration, the dry_run LLM output is {"dry_run": true, ...}
    // which has no "errors" key, so $.errors.length > 0 on null evaluates false.
    // The loop exits via condition_false and takes the exit edge to end-ok.
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Succeeded);
}

#[test]
fn execute_loop_condition_exit_on_final_allowed_iteration() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "loop-final-condition-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Loop Final Condition Test".to_string(),
        description: None,
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes: vec![
            TaskGraphNode {
                id: "start".to_string(),
                node_type: NodeType::Start,
                label: "Start".to_string(),
                description: None,
                position: None,
                config: json!({}),
                pins: vec![],
            },
            TaskGraphNode {
                id: "loop-1".to_string(),
                node_type: NodeType::Loop,
                label: "Conditional Loop".to_string(),
                description: None,
                position: None,
                config: json!({
                    "max_iterations": 1,
                    "condition": {
                        "input_ref": "$.nodes.body-llm.output",
                        "path": "$.needs_repair",
                        "op": "equals",
                        "value": true
                    },
                    "body_entry": "body-llm",
                    "body_exit": "body-llm",
                    "on_max_iterations": "fail"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "body-llm".to_string(),
                node_type: NodeType::Llm,
                label: "Body".to_string(),
                description: None,
                position: None,
                config: json!({
                    "runtime": "codex",
                    "agent": "codex",
                    "prompt": { "mode": "inline", "template": "review" }
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "end-ok".to_string(),
                node_type: NodeType::End,
                label: "OK".to_string(),
                description: None,
                position: None,
                config: json!({ "result": "succeeded" }),
                pins: vec![],
            },
        ],
        edges: vec![
            TaskGraphEdge {
                id: "start__loop-1".to_string(),
                from: "start".to_string(),
                to: "loop-1".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__body-llm__body".to_string(),
                from: "loop-1".to_string(),
                to: "body-llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "loop-1__end-ok__exit".to_string(),
                from: "loop-1".to_string(),
                to: "end-ok".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "loop-final-condition-test".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };

    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Succeeded);
    let loop_iter = final_run
        .context
        .loop_iterations
        .iter()
        .find(|l| l.loop_node_id == "loop-1")
        .unwrap();
    assert_eq!(loop_iter.exit_reason.as_deref(), Some("condition_false"));
}

#[test]
fn prompt_template_renders_graph_inputs_and_env() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let mut context = empty_context();
    context.input = json!({
        "batch-count": 2,
        "max-iterations": 3
    });
    context
        .node_outputs
        .insert("cleanup".to_string(), json!({ "continue": true }));

    let rendered = render_prompt_template(
        "Clean {{inputs.batch-count}} notes for {{env.project}} at {{env.root}} using {{env.scripts_dir}}; continue={{nodes.cleanup.output.continue}}",
        "blackboard",
        root,
        &root.join("scripts"),
        &context,
        None,
    );

    assert!(rendered.contains("Clean 2 notes for blackboard"));
    assert!(rendered.contains(&format!(
        "at {}",
        root.display().to_string().replace('\\', "/")
    )));
    assert!(rendered.contains(&format!(
        "using {}",
        root.join("scripts")
            .display()
            .to_string()
            .replace('\\', "/")
    )));
    assert!(rendered.contains("continue=true"));
}

#[test]
fn prompt_template_renders_explicit_llm_inputs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let mut context = empty_context();
    context.input = json!({
        "batch-count": 2
    });

    let llm_inputs = json!({
        "batch-count": "{{inputs.batch-count}}",
        "static-limit": 5
    });
    let rendered = render_prompt_template(
        "Clean {{inputs.batch-count}} notes; static={{inputs.static-limit}}",
        "blackboard",
        root,
        &root.join("scripts"),
        &context,
        Some(&llm_inputs),
    );

    assert_eq!(rendered, "Clean 2 notes; static=5");
}
