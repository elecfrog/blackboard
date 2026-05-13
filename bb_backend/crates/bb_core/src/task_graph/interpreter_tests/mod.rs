//! Tests for the workflow interpreter �?branch evaluation, loop control, and full run execution.

use super::eval::*;
use super::interpreter::*;
use super::run_state::{self, GraphRef, RunContext, RunStatus};
use super::types::*;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ─── Branch evaluator tests ──────────────────────────────────────────────────

#[test]
fn branch_always_matches_first() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: None,
        rules: vec![BranchRule {
            id: "always-rule".to_string(),
            label: "Always".to_string(),
            when: json!({ "op": "always" }),
        }],
        default_rule_id: "always-rule".to_string(),
    };

    let context = empty_context();
    assert_eq!(evaluate_branch(&config, &context), "always-rule");
}

#[test]
fn branch_gt_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.smoke-task.output".to_string()),
        rules: vec![
            BranchRule {
                id: "has-failures".to_string(),
                label: "Has failures".to_string(),
                when: json!({ "path": "$.failures.length", "op": ">", "value": 0 }),
            },
            BranchRule {
                id: "clean".to_string(),
                label: "Clean".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "clean".to_string(),
    };

    // With failures
    let mut context = empty_context();
    context.node_outputs.insert(
        "smoke-task".to_string(),
        json!({ "failures": [{"page": "A", "message": "fail"}] }),
    );
    assert_eq!(evaluate_branch(&config, &context), "has-failures");

    // Without failures
    let mut context2 = empty_context();
    context2
        .node_outputs
        .insert("smoke-task".to_string(), json!({ "failures": [] }));
    assert_eq!(evaluate_branch(&config, &context2), "clean");
}

#[test]
fn branch_equals_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.gate.output".to_string()),
        rules: vec![
            BranchRule {
                id: "approved".to_string(),
                label: "Approved".to_string(),
                when: json!({ "path": "$.result", "op": "equals", "value": "resume" }),
            },
            BranchRule {
                id: "rejected".to_string(),
                label: "Rejected".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "rejected".to_string(),
    };

    let mut context = empty_context();
    context
        .node_outputs
        .insert("gate".to_string(), json!({ "result": "resume" }));
    assert_eq!(evaluate_branch(&config, &context), "approved");
}

#[test]
fn branch_fallback_to_default() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.missing.output".to_string()),
        rules: vec![BranchRule {
            id: "needs-data".to_string(),
            label: "Needs data".to_string(),
            when: json!({ "path": "$.exists", "op": "exists" }),
        }],
        default_rule_id: "needs-data".to_string(),
    };

    let context = empty_context();
    // input_ref resolves to null, "exists" check fails, goes to default
    assert_eq!(evaluate_branch(&config, &context), "needs-data");
}

#[test]
fn branch_truthy_falsy() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.check.output".to_string()),
        rules: vec![
            BranchRule {
                id: "truthy".to_string(),
                label: "Truthy".to_string(),
                when: json!({ "path": "$.flag", "op": "truthy" }),
            },
            BranchRule {
                id: "falsy".to_string(),
                label: "Falsy".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "falsy".to_string(),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("check".to_string(), json!({ "flag": true }));
    assert_eq!(evaluate_branch(&config, &ctx), "truthy");

    let mut ctx2 = empty_context();
    ctx2.node_outputs
        .insert("check".to_string(), json!({ "flag": false }));
    assert_eq!(evaluate_branch(&config, &ctx2), "falsy");
}

#[test]
fn branch_contains_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.data.output".to_string()),
        rules: vec![BranchRule {
            id: "has-item".to_string(),
            label: "Has item".to_string(),
            when: json!({ "path": "$.items", "op": "contains", "value": "foo" }),
        }],
        default_rule_id: "has-item".to_string(),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("data".to_string(), json!({ "items": ["foo", "bar"] }));
    assert_eq!(evaluate_branch(&config, &ctx), "has-item");
}

// ─── Loop condition tests ────────────────────────────────────────────────────

#[test]
fn loop_condition_true_when_failures_exist() {
    let config = LoopConfig {
        max_iterations: 3,
        max_iterations_ref: None,
        condition: Some(json!({
            "input_ref": "$.nodes.smoke.output",
            "path": "$.failures.length",
            "op": ">",
            "value": 0
        })),
        body_entry: "fix".to_string(),
        body_exit: "smoke".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("smoke".to_string(), json!({ "failures": ["x"] }));
    assert!(evaluate_loop_condition(&config, &ctx));
}

#[test]
fn loop_condition_false_when_no_failures() {
    let config = LoopConfig {
        max_iterations: 3,
        max_iterations_ref: None,
        condition: Some(json!({
            "input_ref": "$.nodes.smoke.output",
            "path": "$.failures.length",
            "op": ">",
            "value": 0
        })),
        body_entry: "fix".to_string(),
        body_exit: "smoke".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("smoke".to_string(), json!({ "failures": [] }));
    assert!(!evaluate_loop_condition(&config, &ctx));
}

#[test]
fn loop_no_condition_always_true() {
    let config = LoopConfig {
        max_iterations: 5,
        max_iterations_ref: None,
        condition: None,
        body_entry: "body".to_string(),
        body_exit: "body-end".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };
    assert!(evaluate_loop_condition(&config, &empty_context()));
}

// ─── Full interpreter dry-run tests ──────────────────────────────────────────

#[test]
fn execute_simple_linear_graph_dry_run() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Build a simple: start �?llm �?end graph
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
}

#[test]
fn execute_branch_selects_correct_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // start �?branch �?end-a (if truthy) / end-b (default)
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

    // Test with flag=true �?should go to end-a (succeeded)
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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

    // Test with flag=false �?should go to end-b (failed)
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

    let opts2 = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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

    // start �?gate �?end
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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

    // Execute �?should pause at the gate
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
    // on null �?false after first iteration �?exit via condition_false.
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

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
    // which has no "errors" key �?condition evaluates $.errors.length > 0 on null �?false
    // �?loop exits via condition_false �?takes exit edge �?end-ok (succeeded)
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let final_run = run_state::read_run(root, "test-project", &run.id).unwrap();
    assert_eq!(final_run.status, RunStatus::Succeeded);
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
        "Clean {{inputs.batch-count}} notes for {{env.project}} at {{env.root}}; continue={{nodes.cleanup.output.continue}}",
        "blackboard",
        root,
        &context,
        None,
    );

    assert!(rendered.contains("Clean 2 notes for blackboard"));
    assert!(rendered.contains(&format!("at {}", root.display())));
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
        &context,
        Some(&llm_inputs),
    );

    assert_eq!(rendered, "Clean 2 notes; static=5");
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn empty_context() -> RunContext {
    RunContext {
        input: json!({}),
        node_outputs: serde_json::Map::new(),
        branch_decisions: Vec::new(),
        loop_iterations: Vec::new(),
        loop_stack: Vec::new(),
        completed_branches: std::collections::HashMap::new(),
    }
}

fn fake_codex_path(root: &Path, message: &str) -> String {
    #[cfg(windows)]
    {
        let path = root.join("codex.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
echo {{\"type\":\"session_meta\",\"payload\":{{\"id\":\"fake-codex-session\"}}}}\r\n\
echo {{\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"{message}\"}}}}\r\n"
            ),
        )
        .unwrap();
        path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("codex");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
printf '%s\n' '{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"fake-codex-session\"}}}}'\n\
printf '%s\n' '{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"{message}\"}}}}'\n"
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path.to_string_lossy().to_string()
    }
}

fn fake_codebuddy_path(root: &Path, message: &str) -> String {
    #[cfg(windows)]
    {
        let path = root.join("codebuddy.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
echo {{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fake-codebuddy-session\",\"tools\":[\"Bash\"]}}\r\n\
echo {{\"type\":\"assistant\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-codebuddy-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":2}}}}}}\r\n\
echo {{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"{message}\",\"session_id\":\"fake-codebuddy-session\"}}\r\n"
            ),
        )
        .unwrap();
        path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("codebuddy");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
printf '%s\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fake-codebuddy-session\",\"tools\":[\"Bash\"]}}'\n\
printf '%s\n' '{{\"type\":\"assistant\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-codebuddy-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":2}}}}}}'\n\
printf '%s\n' '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"{message}\",\"session_id\":\"fake-codebuddy-session\"}}'\n"
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path.to_string_lossy().to_string()
    }
}

fn simple_llm_graph(config: serde_json::Value) -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "agent-session-codex-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "AgentSession Codex Test".to_string(),
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
                id: "llm".to_string(),
                node_type: NodeType::Llm,
                label: "LLM".to_string(),
                description: None,
                position: None,
                config,
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
                id: "start__llm".to_string(),
                from: "start".to_string(),
                to: "llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "llm__end".to_string(),
                from: "llm".to_string(),
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
    }
}

fn run_codex_agent_session_graph(
    root: &Path,
    graph: &TaskGraphDefinition,
    codex_path: String,
) -> (String, run_state::TaskGraphRunNode) {
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, graph, json!({})).unwrap();
    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path,
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: false,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };
    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "llm")
        .cloned()
        .unwrap();
    (run.id, node)
}

fn run_codebuddy_agent_session_graph(
    root: &Path,
    graph: &TaskGraphDefinition,
    codebuddy_path: String,
) -> (String, run_state::TaskGraphRunNode) {
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, graph, json!({})).unwrap();
    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path,
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: false,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };
    let outcome = execute_run(&opts).unwrap();
    assert!(matches!(outcome, RunOutcome::Succeeded));

    let detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "llm")
        .cloned()
        .unwrap();
    (run.id, node)
}

#[test]
fn codex_llm_node_creates_agent_session() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let codex_path = fake_codex_path(root, "codex llm ok");
    let graph = simple_llm_graph(json!({
        "runtime": "codex",
        "agent": "codex",
        "prompt": { "mode": "inline", "template": "hello" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (_run_id, node) = run_codex_agent_session_graph(root, &graph, codex_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let session_id = node.agent_session_id.expect("codex node has AgentSession");
    assert_eq!(node.log_tail, None);
    let events =
        crate::agent_session::read_events(root, "test-project", &session_id, None).unwrap();
    assert!(events.iter().any(|event| event.event_type
        == crate::agent_session::AgentEventType::Text
        && event.content.as_deref() == Some("codex llm ok")));
}

#[test]
fn codex_agent_profile_node_creates_agent_session() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("agents")).unwrap();
    fs::write(
        root.join("agents/agents.toml"),
        r#"
[[agents]]
id = "codex-worker"
display_name = "Codex Worker"
kind = "platform_agent"
runtime = "codex"
"#,
    )
    .unwrap();
    let codex_path = fake_codex_path(root, "codex agent ok");
    let graph = simple_llm_graph(json!({
        "run_as": "agent",
        "agent_profile": "codex-worker",
        "prompt": { "mode": "inline", "template": "profile task" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (_run_id, node) = run_codex_agent_session_graph(root, &graph, codex_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let session_id = node
        .agent_session_id
        .expect("codex agent node has AgentSession");
    let session = crate::agent_session::read_session(root, "test-project", &session_id).unwrap();
    assert_eq!(session.runtime, "codex");
    assert_eq!(session.agent, "codex-worker");
}

#[test]
fn codebuddy_llm_node_creates_agent_session() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let codebuddy_path = fake_codebuddy_path(root, "codebuddy llm ok");
    let graph = simple_llm_graph(json!({
        "runtime": "codebuddy",
        "agent": "native",
        "prompt": { "mode": "inline", "template": "hello" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (_run_id, node) = run_codebuddy_agent_session_graph(root, &graph, codebuddy_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let session_id = node.agent_session_id.expect("codebuddy node has AgentSession");
    assert_eq!(node.log_tail, None);
    let events =
        crate::agent_session::read_events(root, "test-project", &session_id, None).unwrap();
    assert!(events.iter().any(|event| event.event_type
        == crate::agent_session::AgentEventType::Text
        && event.content.as_deref() == Some("codebuddy llm ok")));
    let artifact_path = crate::agent_session::session_dir(root, "test-project", &session_id)
        .join("artifacts/codebuddy-mcp.json");
    assert!(artifact_path.exists());
}

#[test]
fn codebuddy_agent_profile_node_creates_agent_session() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("agents")).unwrap();
    fs::write(
        root.join("agents/agents.toml"),
        r#"
[[agents]]
id = "codebuddy-worker"
display_name = "CodeBuddy Worker"
kind = "platform_agent"
runtime = "codebuddy"
"#,
    )
    .unwrap();
    let codebuddy_path = fake_codebuddy_path(root, "codebuddy agent ok");
    let graph = simple_llm_graph(json!({
        "run_as": "agent",
        "agent_profile": "codebuddy-worker",
        "prompt": { "mode": "inline", "template": "profile task" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (_run_id, node) = run_codebuddy_agent_session_graph(root, &graph, codebuddy_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let session_id = node
        .agent_session_id
        .expect("codebuddy agent node has AgentSession");
    let session = crate::agent_session::read_session(root, "test-project", &session_id).unwrap();
    assert_eq!(session.runtime, "codebuddy");
    assert_eq!(session.agent, "codebuddy-worker");
}

// ─── Parallel Fork/Join Tests ────────────────────────────────────────────────

/// 测试基本并行 fork/join：Start �?[LLM-A, LLM-B] �?End
/// 两个 LLM 节点并行执行（dry_run），End 节点等待两者都完成后才执行�?
#[test]
fn parallel_fork_join_basic() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "parallel-basic".to_string(),
        scope: TaskGraphScope::Project,
        title: "Parallel Basic".to_string(),
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
                id: "llm-a".to_string(),
                node_type: NodeType::Llm,
                label: "LLM A".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Task A" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "llm-b".to_string(),
                node_type: NodeType::Llm,
                label: "LLM B".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Task B" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
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
            // Start �?LLM-A
            TaskGraphEdge {
                id: "start__llm-a".to_string(),
                from: "start".to_string(),
                to: "llm-a".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            // Start �?LLM-B
            TaskGraphEdge {
                id: "start__llm-b".to_string(),
                from: "start".to_string(),
                to: "llm-b".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            // LLM-A �?End
            TaskGraphEdge {
                id: "llm-a__end".to_string(),
                from: "llm-a".to_string(),
                to: "end".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            // LLM-B �?End
            TaskGraphEdge {
                id: "llm-b__end".to_string(),
                from: "llm-b".to_string(),
                to: "end".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "parallel-basic".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
    assert!(
        matches!(outcome, RunOutcome::Succeeded),
        "Expected Succeeded, got {:?}",
        outcome
    );

    // 验证两个 LLM 节点都被执行�?
    let detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    let llm_a_state = detail.nodes.iter().find(|n| n.node_id == "llm-a");
    let llm_b_state = detail.nodes.iter().find(|n| n.node_id == "llm-b");
    assert!(llm_a_state.is_some(), "LLM-A node state should exist");
    assert!(llm_b_state.is_some(), "LLM-B node state should exist");
    assert_eq!(
        llm_a_state.unwrap().status,
        run_state::NodeRunStatus::Succeeded
    );
    assert_eq!(
        llm_b_state.unwrap().status,
        run_state::NodeRunStatus::Succeeded
    );
}

/// 测试并行 fork 到中�?join 点：Start �?[LLM-A, LLM-B] �?LLM-C �?End
/// LLM-C 作为 join 点等�?A �?B 都完成后才执行�?
#[test]
fn parallel_fork_join_intermediate_node() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "parallel-intermediate".to_string(),
        scope: TaskGraphScope::Project,
        title: "Parallel Intermediate".to_string(),
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
                id: "llm-a".to_string(),
                node_type: NodeType::Llm,
                label: "LLM A".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Task A" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "llm-b".to_string(),
                node_type: NodeType::Llm,
                label: "LLM B".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Task B" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "llm-c".to_string(),
                node_type: NodeType::Llm,
                label: "LLM C".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Task C (join)" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
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
                id: "start__llm-a".to_string(),
                from: "start".to_string(),
                to: "llm-a".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "start__llm-b".to_string(),
                from: "start".to_string(),
                to: "llm-b".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "llm-a__llm-c".to_string(),
                from: "llm-a".to_string(),
                to: "llm-c".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "llm-b__llm-c".to_string(),
                from: "llm-b".to_string(),
                to: "llm-c".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "llm-c__end".to_string(),
                from: "llm-c".to_string(),
                to: "end".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "parallel-intermediate".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
    assert!(
        matches!(outcome, RunOutcome::Succeeded),
        "Expected Succeeded, got {:?}",
        outcome
    );

    // 验证所有节点都被执行了
    let detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    let llm_c_state = detail.nodes.iter().find(|n| n.node_id == "llm-c");
    assert!(
        llm_c_state.is_some(),
        "LLM-C (join) node state should exist"
    );
    assert_eq!(
        llm_c_state.unwrap().status,
        run_state::NodeRunStatus::Succeeded
    );
}

/// 测试单分支向后兼容：Start �?LLM-A �?End（无并行�?
#[test]
fn parallel_single_branch_backward_compatible() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let graph = TaskGraphDefinition {
        schema_version: 1,
        id: "single-branch".to_string(),
        scope: TaskGraphScope::Project,
        title: "Single Branch".to_string(),
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
                id: "llm-a".to_string(),
                node_type: NodeType::Llm,
                label: "LLM A".to_string(),
                description: None,
                position: None,
                config: json!({
                    "prompt": { "mode": "template", "template": "Single task" },
                    "runtime": "opencode",
                    "agent": "default",
                    "model": "test-model"
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
                id: "start__llm-a".to_string(),
                from: "start".to_string(),
                to: "llm-a".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "llm-a__end".to_string(),
                from: "llm-a".to_string(),
                to: "end".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
        ],
        layout: None,
    };

    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: "single-branch".to_string(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, json!({})).unwrap();

    let opts = InterpreterOptions {
        workspace_root: root.to_path_buf(),
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
    assert!(
        matches!(outcome, RunOutcome::Succeeded),
        "Expected Succeeded for single branch, got {:?}",
        outcome
    );
}
