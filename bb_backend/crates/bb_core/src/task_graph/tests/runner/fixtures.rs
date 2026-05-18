//! Shared fixtures for runner tests.

use crate::task_graph::definition::types::*;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::RunContext;
use crate::task_graph::{ChannelKind, ChannelSpec, ChannelValueType};
use serde_json::json;
use std::path::Path;

pub(super) fn empty_context() -> RunContext {
    RunContext {
        input: json!({}),
        node_outputs: serde_json::Map::new(),
        branch_decisions: Vec::new(),
        loop_iterations: Vec::new(),
        loop_stack: Vec::new(),
        completed_branches: std::collections::HashMap::new(),
    }
}

pub(super) fn smoke_runner_opts(root: &Path, run_id: &str) -> RunnerOptions {
    RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run_id.to_string(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        pi_path: "pi".to_string(),
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: true,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    }
}

pub(super) fn swe_e2e_channel_specs() -> Vec<ChannelSpec> {
    vec![
        channel_spec("findings", ChannelKind::Topic, ChannelValueType::Json),
        channel_spec("diff", ChannelKind::LastValue, ChannelValueType::Diff),
        channel_spec(
            "test_result",
            ChannelKind::LastValue,
            ChannelValueType::TestResult,
        ),
        channel_spec(
            "review_comments",
            ChannelKind::Topic,
            ChannelValueType::ReviewComment,
        ),
        channel_spec(
            "handoff_summary",
            ChannelKind::LastValue,
            ChannelValueType::HandoffSummary,
        ),
        ChannelSpec {
            name: "acceptance_gate".to_string(),
            kind: ChannelKind::Barrier,
            value_type: ChannelValueType::Any,
            reducer: None,
            barrier_nodes: vec!["reviewer".to_string(), "handoff".to_string()],
            description: Some(
                "Reviewer and handoff must both arrive before acceptance.".to_string(),
            ),
        },
    ]
}

fn channel_spec(name: &str, kind: ChannelKind, value_type: ChannelValueType) -> ChannelSpec {
    ChannelSpec {
        name: name.to_string(),
        kind,
        value_type,
        reducer: None,
        barrier_nodes: Vec::new(),
        description: None,
    }
}

pub(super) fn swe_e2e_smoke_graph() -> TaskGraphDefinition {
    let nodes = vec![
        control_node("start", NodeType::Start, "Start", json!({})),
        role_llm_node("explorer", "Explorer", "explorer_agent"),
        role_llm_node("implementer", "Implementer", "implementer_agent"),
        role_llm_node("verifier", "Verifier", "verifier_agent"),
        role_llm_node("reviewer", "Reviewer", "reviewer_agent"),
        role_llm_node("handoff", "Handoff", "handoff_writer"),
        control_node(
            "human-gate",
            NodeType::HumanGate,
            "Human Gate",
            json!({
                "title": "Accept smoke result",
                "instructions": "Review findings, diff, test result, review comments, and handoff summary.",
                "actions": [
                    { "id": "approve", "label": "Approve", "result": "resume" },
                    { "id": "reject", "label": "Reject", "result": "cancel" }
                ]
            }),
        ),
        control_node(
            "end",
            NodeType::End,
            "End",
            json!({ "result": "succeeded" }),
        ),
    ];
    let edges = vec![
        exec_edge("start", "explorer"),
        exec_edge("explorer", "implementer"),
        exec_edge("implementer", "verifier"),
        exec_edge("verifier", "reviewer"),
        exec_edge("reviewer", "handoff"),
        exec_edge("handoff", "human-gate"),
        exec_edge("human-gate", "end"),
    ];

    TaskGraphDefinition {
        schema_version: 1,
        id: "swe-e2e-smoke".to_string(),
        scope: TaskGraphScope::Project,
        title: "SWE E2E Smoke".to_string(),
        description: Some(
            "Start -> Explorer -> Implementer -> Verifier -> Reviewer -> Handoff -> HumanGate -> End"
                .to_string(),
        ),
        version: 1,
        readonly: false,
        origin: None,
        metadata: None,
        inputs: None,
        nodes,
        edges,
        layout: None,
    }
}

fn role_llm_node(id: &str, label: &str, role: &str) -> TaskGraphNode {
    TaskGraphNode {
        id: id.to_string(),
        node_type: NodeType::Llm,
        label: label.to_string(),
        description: None,
        position: None,
        config: json!({
            "role": role,
            "runtime": "codex",
            "agent": "native",
            "prompt": {
                "mode": "inline",
                "template": format!("{id} smoke node for #000066")
            },
            "output": { "artifact_type": "json", "required": false }
        }),
        pins: vec![],
    }
}

fn control_node(
    id: &str,
    node_type: NodeType,
    label: &str,
    config: serde_json::Value,
) -> TaskGraphNode {
    TaskGraphNode {
        id: id.to_string(),
        node_type,
        label: label.to_string(),
        description: None,
        position: None,
        config,
        pins: vec![],
    }
}

pub(super) fn exec_edge(from: &str, to: &str) -> TaskGraphEdge {
    TaskGraphEdge {
        id: format!("{from}__{to}"),
        from: from.to_string(),
        to: to.to_string(),
        kind: EdgeKind::Exec,
        label: None,
        source_handle: None,
        target_handle: None,
        from_pin: Some("exec_out".to_string()),
        to_pin: Some("exec_in".to_string()),
    }
}
