//! Runtime node tests for shell, Codex, and CodeBuddy execution.

use crate::task_graph::definition::types::*;
use crate::task_graph::pregel::runner::*;
use crate::task_graph::run_state::{self, GraphRef};
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

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

fn simple_shell_graph(config: serde_json::Value) -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "shell-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Shell Test".to_string(),
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
                id: "shell".to_string(),
                node_type: NodeType::Shell,
                label: "Shell".to_string(),
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
                id: "start__shell".to_string(),
                from: "start".to_string(),
                to: "shell".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "shell__end".to_string(),
                from: "shell".to_string(),
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

fn run_shell_graph(
    root: &Path,
    config: serde_json::Value,
) -> (RunOutcome, run_state::TaskGraphRunDetail) {
    let graph = simple_shell_graph(config);
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
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
        dry_run: false,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };
    let outcome = execute_run(&opts).unwrap();
    let detail = run_state::read_run_detail(root, "test-project", &run.id).unwrap();
    (outcome, detail)
}

#[test]
fn shell_node_git_status_succeeds_and_writes_artifact() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    std::process::Command::new("git")
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap();

    let (outcome, detail) = run_shell_graph(
        root,
        json!({
            "command": "git",
            "args": ["status", "--short"],
            "permission": "read_only",
            "capture": { "max_bytes": 32768 }
        }),
    );

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "shell")
        .unwrap();
    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    assert_eq!(node.exit_code, Some(0));
    let artifact = node.output_artifact.as_ref().expect("shell artifact");
    assert_eq!(artifact.content_type, run_state::ArtifactContentType::Json);
    let output = detail.run.context.node_outputs.get("shell").unwrap();
    assert_eq!(output["ok"], true);
    assert_eq!(output["command"], "git");
}

#[test]
fn shell_node_cwd_escape_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (outcome, detail) = run_shell_graph(
        root,
        json!({
            "cwd": "..",
            "command": "git",
            "args": ["status"]
        }),
    );

    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "shell")
        .unwrap();
    assert_eq!(node.status, run_state::NodeRunStatus::Failed);
    assert_eq!(node.error.as_ref().unwrap().code, "invalid_cwd");
}

#[test]
fn shell_node_expected_nonzero_exit_code_can_succeed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    std::process::Command::new("git")
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap();

    let (outcome, detail) = run_shell_graph(
        root,
        json!({
            "command": "git",
            "args": ["rev-parse", "--verify", "missing-ref"],
            "expected_exit_codes": [128]
        }),
    );

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let output = detail.run.context.node_outputs.get("shell").unwrap();
    assert_eq!(output["ok"], true);
    assert_eq!(output["exit_code"], 128);
}

#[test]
fn shell_node_permission_denies_git_write() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (outcome, detail) = run_shell_graph(
        root,
        json!({
            "command": "git",
            "args": ["commit", "-m", "x"],
            "permission": "read_only"
        }),
    );

    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "shell")
        .unwrap();
    assert_eq!(node.error.as_ref().unwrap().code, "permission_denied");
}

#[test]
fn shell_node_timeout_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let config = timeout_shell_config();
    let (outcome, detail) = run_shell_graph(root, config);

    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "shell")
        .unwrap();
    assert_eq!(node.error.as_ref().unwrap().code, "timeout");
    let output = detail.run.context.node_outputs.get("shell").unwrap();
    assert_eq!(output["timed_out"], true);
}

fn timeout_shell_config() -> serde_json::Value {
    #[cfg(windows)]
    {
        json!({
            "command": "powershell",
            "args": ["-NoProfile", "-Command", "Start-Sleep -Milliseconds 500"],
            "timeout_ms": 100
        })
    }

    #[cfg(not(windows))]
    {
        json!({
            "command": "sleep",
            "args": ["1"],
            "timeout_ms": 100
        })
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
    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
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
    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
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
    let session_id = node
        .agent_session_id
        .expect("codebuddy node has AgentSession");
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
