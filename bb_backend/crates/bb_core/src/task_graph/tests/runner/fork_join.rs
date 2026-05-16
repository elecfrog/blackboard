//! Fork/join runner tests.

use crate::task_graph::definition::types::*;
use crate::task_graph::pregel::runner::*;
use crate::task_graph::run_state::{self, GraphRef};
use serde_json::json;
use tempfile::TempDir;

// ─── Parallel Fork/Join Tests ────────────────────────────────────────────────

/// Basic parallel fork/join: Start -> [LLM-A, LLM-B] -> End.
/// The two LLM nodes run in parallel in dry-run mode; End waits for both.
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
            // Start -> LLM-A
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
            // Start -> LLM-B
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
            // LLM-A -> End
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
            // LLM-B -> End
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
    assert!(
        matches!(outcome, RunOutcome::Succeeded),
        "Expected Succeeded, got {:?}",
        outcome
    );

    // Verify both LLM nodes executed.
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

/// Parallel fork with an intermediate join: Start -> [LLM-A, LLM-B] -> LLM-C -> End.
/// LLM-C acts as the join point and waits for both A and B.
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

/// Single-branch compatibility: Start -> LLM-A -> End.
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
    assert!(
        matches!(outcome, RunOutcome::Succeeded),
        "Expected Succeeded for single branch, got {:?}",
        outcome
    );
}
