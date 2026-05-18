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

fn fake_pi_path(root: &Path, message: &str) -> String {
    let args_path = root.join("pi-args.txt");

    #[cfg(windows)]
    {
        let path = root.join("pi.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
echo %* > \"{}\"\r\n\
echo {{\"type\":\"session\",\"version\":3,\"id\":\"fake-pi-session\",\"timestamp\":\"2026-05-21T00:00:00Z\",\"cwd\":\".\"}}\r\n\
echo {{\"type\":\"agent_start\"}}\r\n\
echo {{\"type\":\"turn_start\"}}\r\n\
echo {{\"type\":\"message_update\",\"assistantMessageEvent\":{{\"type\":\"text_delta\",\"delta\":\"{message}\"}}}}\r\n\
echo {{\"type\":\"turn_end\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-pi-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input\":1,\"output\":2}}}},\"toolResults\":[]}}\r\n\
echo {{\"type\":\"agent_end\",\"messages\":[]}}\r\n",
                args_path.display()
            ),
        )
        .unwrap();
        path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("pi");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
printf '%s\n' \"$@\" > '{}'\n\
printf '%s\n' '{{\"type\":\"session\",\"version\":3,\"id\":\"fake-pi-session\",\"timestamp\":\"2026-05-21T00:00:00Z\",\"cwd\":\"$PWD\"}}'\n\
printf '%s\n' '{{\"type\":\"agent_start\"}}'\n\
printf '%s\n' '{{\"type\":\"turn_start\"}}'\n\
printf '%s\n' '{{\"type\":\"message_update\",\"assistantMessageEvent\":{{\"type\":\"text_delta\",\"delta\":\"{message}\"}}}}'\n\
printf '%s\n' '{{\"type\":\"turn_end\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-pi-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input\":1,\"output\":2}}}},\"toolResults\":[]}}'\n\
printf '%s\n' '{{\"type\":\"agent_end\",\"messages\":[]}}'\n",
                args_path.display()
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path.to_string_lossy().to_string()
    }
}

fn fake_pi_tool_path(root: &Path, message: &str) -> String {
    let long_output = "x".repeat(2300);
    #[cfg(windows)]
    {
        let path = root.join("pi-tools.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
echo {{\"type\":\"session\",\"version\":3,\"id\":\"fake-pi-session\",\"timestamp\":\"2026-05-21T00:00:00Z\",\"cwd\":\".\"}}\r\n\
echo {{\"type\":\"agent_start\"}}\r\n\
echo {{\"type\":\"turn_start\"}}\r\n\
echo {{\"type\":\"tool_execution_start\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"args\":{{\"project\":\"blackboard\",\"id\":\"000080\"}}}}\r\n\
echo {{\"type\":\"tool_execution_update\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"partialResult\":{{\"content\":\"mcp running\"}}}}\r\n\
echo {{\"type\":\"tool_execution_end\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"isError\":false,\"result\":{{\"content\":\"mcp done\"}}}}\r\n\
echo {{\"type\":\"tool_execution_start\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"args\":{{\"command\":\"cargo check\"}}}}\r\n\
echo {{\"type\":\"tool_execution_update\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"partialResult\":{{\"content\":\"{long_output}\"}}}}\r\n\
echo {{\"type\":\"tool_execution_end\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"isError\":false,\"result\":{{\"content\":\"bash done\"}}}}\r\n\
echo {{\"type\":\"message_update\",\"assistantMessageEvent\":{{\"type\":\"text_delta\",\"delta\":\"{message}\"}}}}\r\n\
echo {{\"type\":\"turn_end\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-pi-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input\":1,\"output\":2}}}},\"toolResults\":[]}}\r\n\
echo {{\"type\":\"agent_end\",\"messages\":[]}}\r\n"
            ),
        )
        .unwrap();
        path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("pi-tools");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
printf '%s\n' '{{\"type\":\"session\",\"version\":3,\"id\":\"fake-pi-session\",\"timestamp\":\"2026-05-21T00:00:00Z\",\"cwd\":\"$PWD\"}}'\n\
printf '%s\n' '{{\"type\":\"agent_start\"}}'\n\
printf '%s\n' '{{\"type\":\"turn_start\"}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_start\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"args\":{{\"project\":\"blackboard\",\"id\":\"000080\"}}}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_update\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"partialResult\":{{\"content\":\"mcp running\"}}}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_end\",\"toolName\":\"server__bb__read_ticket_by_id\",\"toolCallId\":\"call-mcp\",\"isError\":false,\"result\":{{\"content\":\"mcp done\"}}}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_start\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"args\":{{\"command\":\"cargo check\"}}}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_update\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"partialResult\":{{\"content\":\"{long_output}\"}}}}'\n\
printf '%s\n' '{{\"type\":\"tool_execution_end\",\"toolName\":\"bash\",\"toolCallId\":\"call-bash\",\"isError\":false,\"result\":{{\"content\":\"bash done\"}}}}'\n\
printf '%s\n' '{{\"type\":\"message_update\",\"assistantMessageEvent\":{{\"type\":\"text_delta\",\"delta\":\"{message}\"}}}}'\n\
printf '%s\n' '{{\"type\":\"turn_end\",\"message\":{{\"role\":\"assistant\",\"model\":\"fake-pi-model\",\"content\":[{{\"type\":\"text\",\"text\":\"{message}\"}}],\"usage\":{{\"input\":1,\"output\":2}}}},\"toolResults\":[]}}'\n\
printf '%s\n' '{{\"type\":\"agent_end\",\"messages\":[]}}'\n"
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path.to_string_lossy().to_string()
    }
}

fn fake_direct_llm_runtime_path(root: &Path) -> (String, std::path::PathBuf) {
    let cwd_path = root.join("direct-llm-cwd.txt");

    #[cfg(windows)]
    {
        let path = root.join("direct-llm.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
cd > \"{}\"\r\n\
echo direct llm ok\r\n",
                cwd_path.display()
            ),
        )
        .unwrap();
        (path.to_string_lossy().to_string(), cwd_path)
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("direct-llm");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
pwd > '{}'\n\
printf '%s\n' 'direct llm ok'\n",
                cwd_path.display()
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        (path.to_string_lossy().to_string(), cwd_path)
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

fn simple_data_value_graph(config: serde_json::Value) -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "data-value-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Data Value Test".to_string(),
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
                id: "prompt".to_string(),
                node_type: NodeType::DataValue,
                label: "Prompt".to_string(),
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
        edges: vec![TaskGraphEdge {
            id: "start__end".to_string(),
            from: "start".to_string(),
            to: "end".to_string(),
            kind: EdgeKind::Exec,
            label: None,
            source_handle: None,
            target_handle: None,
            from_pin: Some("exec_out".to_string()),
            to_pin: Some("exec_in".to_string()),
        }],
        layout: None,
    }
}

fn run_data_value_graph(
    root: &Path,
    config: serde_json::Value,
    input: serde_json::Value,
) -> (RunOutcome, run_state::TaskGraphRunDetail) {
    let graph = simple_data_value_graph(config);
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, input).unwrap();
    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        pi_path: "pi".to_string(),
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
        pi_path: "pi".to_string(),
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

fn simple_coordinator_graph(config: serde_json::Value) -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "coordinator-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Coordinator Test".to_string(),
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
                id: "coordinator".to_string(),
                node_type: NodeType::LlmCoordinator,
                label: "Coordinator".to_string(),
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
                id: "start__coordinator".to_string(),
                from: "start".to_string(),
                to: "coordinator".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "coordinator__end".to_string(),
                from: "coordinator".to_string(),
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

fn data_value_to_coordinator_graph(config: serde_json::Value) -> TaskGraphDefinition {
    TaskGraphDefinition {
        schema_version: 1,
        id: "coordinator-data-input-test".to_string(),
        scope: TaskGraphScope::Project,
        title: "Coordinator Data Input Test".to_string(),
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
                id: "prompt".to_string(),
                node_type: NodeType::DataValue,
                label: "Prompt".to_string(),
                description: None,
                position: None,
                config: json!({
                    "value_type": "markdown",
                    "value": "{{inputs.intent}}"
                }),
                pins: vec![],
            },
            TaskGraphNode {
                id: "coordinator".to_string(),
                node_type: NodeType::LlmCoordinator,
                label: "Coordinator".to_string(),
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
                id: "start__coordinator".to_string(),
                from: "start".to_string(),
                to: "coordinator".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: Some("exec_out".to_string()),
                to_pin: Some("exec_in".to_string()),
            },
            TaskGraphEdge {
                id: "prompt__coordinator_data".to_string(),
                from: "prompt".to_string(),
                to: "coordinator".to_string(),
                kind: EdgeKind::Data,
                label: None,
                source_handle: Some("value".to_string()),
                target_handle: Some("input".to_string()),
                from_pin: Some("value".to_string()),
                to_pin: Some("input".to_string()),
            },
            TaskGraphEdge {
                id: "coordinator__end".to_string(),
                from: "coordinator".to_string(),
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
    }
}

fn run_data_value_to_coordinator_graph(
    root: &Path,
    config: serde_json::Value,
    input: serde_json::Value,
) -> (RunOutcome, run_state::TaskGraphRunDetail) {
    let graph = data_value_to_coordinator_graph(config);
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: graph.id.clone(),
        version: 1,
    };
    let run = run_state::create_run(root, "test-project", graph_ref, &graph, input).unwrap();
    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: run.id.clone(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        pi_path: "pi".to_string(),
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

fn static_input_subgraph() -> serde_json::Value {
    json!({
        "schema_version": 1,
        "id": "coordinator-child",
        "scope": "project",
        "title": "Coordinator Child",
        "version": 1,
        "readonly": false,
        "inputs": [
            {
                "id": "value",
                "type": "string",
                "default": ""
            }
        ],
        "nodes": [
            {
                "id": "start",
                "type": "start",
                "label": "Start",
                "config": {}
            },
            {
                "id": "read-value",
                "type": "input_var",
                "label": "Read Value",
                "config": { "input_id": "value" }
            },
            {
                "id": "end",
                "type": "end",
                "label": "End",
                "config": { "result": "succeeded" }
            }
        ],
        "edges": [
            {
                "id": "start__read",
                "from": "start",
                "to": "read-value",
                "kind": "exec"
            },
            {
                "id": "read__end",
                "from": "read-value",
                "to": "end",
                "kind": "exec"
            }
        ]
    })
}

fn static_input_subgraph_for(input_id: &str) -> serde_json::Value {
    let mut graph = static_input_subgraph();
    graph["inputs"][0]["id"] = json!(input_id);
    graph["nodes"][1]["config"]["input_id"] = json!(input_id);
    graph
}

fn run_coordinator_graph(
    root: &Path,
    config: serde_json::Value,
) -> (RunOutcome, run_state::TaskGraphRunDetail) {
    let graph = simple_coordinator_graph(config);
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
        pi_path: "pi".to_string(),
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
fn data_value_resolves_graph_input_template() {
    let tmp = TempDir::new().unwrap();
    let (outcome, detail) = run_data_value_graph(
        tmp.path(),
        json!({
            "value_type": "markdown",
            "value": "任务: {{inputs.intent}}"
        }),
        json!({ "intent": "调研 078" }),
    );

    assert!(matches!(outcome, RunOutcome::Succeeded));
    assert_eq!(
        detail.run.context.node_outputs.get("prompt").unwrap(),
        "任务: 调研 078"
    );
}

#[test]
fn data_value_resolves_nested_resource_bundle_templates() {
    let tmp = TempDir::new().unwrap();
    let (outcome, detail) = run_data_value_graph(
        tmp.path(),
        json!({
            "value_type": "json",
            "value": {
                "task": {
                    "input": "{{inputs.input}}"
                },
                "data_sources": {
                    "source_root": "{{inputs.source-root}}"
                },
                "runtime": {
                    "workspace": "{{env.workspace}}"
                },
                "skills": "{{inputs.skills}}"
            }
        }),
        json!({
            "input": "调研 078",
            "source-root": "C:/Dev/blackboard",
            "skills": ["code-research"]
        }),
    );

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let output = detail.run.context.node_outputs.get("prompt").unwrap();
    assert_eq!(output["task"]["input"], "调研 078");
    assert_eq!(output["data_sources"]["source_root"], "C:/Dev/blackboard");
    assert!(output["runtime"]["workspace"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(output["skills"], json!(["code-research"]));
}

#[test]
fn llm_coordinator_executes_static_isolated_subgraph_and_exposes_result() {
    let tmp = TempDir::new().unwrap();
    let config = json!({
        "run_as": "llm",
        "runtime": "opencode",
        "agent": "native",
        "prompt": { "mode": "inline", "template": "unused for static_subgraph" },
        "coordinator": {
            "max_nodes": 8,
            "max_edges": 8
        },
        "static_subgraph": static_input_subgraph(),
        "input_bindings": {
            "value": "coordinator-output"
        }
    });

    let (outcome, detail) = run_coordinator_graph(tmp.path(), config);

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "coordinator")
        .unwrap();
    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    assert!(node.child_run_id.is_some());
    let output = detail.run.context.node_outputs.get("coordinator").unwrap();
    assert_eq!(output["status"], "completed");
    assert_eq!(output["outputs"], "coordinator-output");
}

#[test]
fn llm_coordinator_uses_connected_data_value_as_default_child_input() {
    let tmp = TempDir::new().unwrap();
    let config = json!({
        "run_as": "llm",
        "runtime": "opencode",
        "agent": "native",
        "prompt": { "mode": "inline", "template": "unused for static_subgraph" },
        "coordinator": {
            "max_nodes": 8,
            "max_edges": 8
        },
        "static_subgraph": static_input_subgraph_for("input")
    });

    let (outcome, detail) =
        run_data_value_to_coordinator_graph(tmp.path(), config, json!({ "intent": "调研 078" }));

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let output = detail.run.context.node_outputs.get("coordinator").unwrap();
    assert_eq!(output["outputs"], "调研 078");
}

#[test]
fn llm_coordinator_rejects_invalid_subgraph_without_child_run() {
    let tmp = TempDir::new().unwrap();
    let mut invalid = static_input_subgraph();
    invalid["nodes"] = json!([]);
    let config = json!({
        "run_as": "llm",
        "runtime": "opencode",
        "agent": "native",
        "prompt": { "mode": "inline", "template": "unused for static_subgraph" },
        "static_subgraph": invalid
    });

    let (outcome, detail) = run_coordinator_graph(tmp.path(), config);

    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let node = detail
        .nodes
        .iter()
        .find(|node| node.node_id == "coordinator")
        .unwrap();
    assert_eq!(node.status, run_state::NodeRunStatus::Failed);
    assert_eq!(
        node.error.as_ref().unwrap().code,
        "validate_subgraph_failed"
    );
    assert!(node.child_run_id.is_none());
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
fn shell_node_exposes_stdout_json_when_stdout_is_json() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config = shell_json_stdout_config();
    let (outcome, detail) = run_shell_graph(root, config);

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let output = detail.run.context.node_outputs.get("shell").unwrap();
    assert_eq!(output["ok"], true);
    assert_eq!(output["stdout_json"]["continue"], true);
    assert_eq!(output["stdout_json"]["processed"], 2);
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
fn shell_node_template_workspace_cwd_uses_repo_parent() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();
    let root = repo_root.join(".bb_template");
    let backend = repo_root.join("bb_backend");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&backend).unwrap();

    let (outcome, detail) = run_shell_graph(
        &root,
        json!({
            "cwd": "bb_backend",
            "command": "git",
            "args": ["--version"],
            "permission": "read_only"
        }),
    );

    assert!(matches!(outcome, RunOutcome::Succeeded));
    let output = detail.run.context.node_outputs.get("shell").unwrap();
    let actual_cwd = std::path::PathBuf::from(output["cwd"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(actual_cwd, backend.canonicalize().unwrap());
}

#[test]
fn shell_node_template_workspace_parent_escape_fails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join(".bb_template");
    fs::create_dir_all(&root).unwrap();

    let (outcome, detail) = run_shell_graph(
        &root,
        json!({
            "cwd": "..",
            "command": "git",
            "args": ["--version"],
            "permission": "read_only"
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

fn shell_json_stdout_config() -> serde_json::Value {
    #[cfg(windows)]
    {
        json!({
            "command": "powershell",
            "args": [
                "-NoProfile",
                "-Command",
                "Write-Output '{\"continue\":true,\"processed\":2}'"
            ],
            "capture": { "max_bytes": 32768, "strip_ansi": true }
        })
    }

    #[cfg(not(windows))]
    {
        json!({
            "command": "printf",
            "args": ["%s", "{\"continue\":true,\"processed\":2}"],
            "capture": { "max_bytes": 32768, "strip_ansi": true }
        })
    }
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
        pi_path: "pi".to_string(),
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
        pi_path: "pi".to_string(),
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

fn run_pi_agent_session_graph(
    root: &Path,
    graph: &TaskGraphDefinition,
    pi_path: String,
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
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: "opencode".to_string(),
        opencode_config_content: None,
        pi_path,
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
fn direct_llm_runtime_uses_workspace_root_as_current_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (runtime_path, cwd_file) = fake_direct_llm_runtime_path(root);
    let opts = RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: root.join("scripts"),
        project: "test-project".to_string(),
        run_id: "run-direct-cwd".to_string(),
        codex_path: "codex".to_string(),
        codebuddy_path: "codebuddy".to_string(),
        opencode_path: runtime_path,
        opencode_config_content: None,
        pi_path: "pi".to_string(),
        model: None,
        node_timeout: std::time::Duration::from_secs(10),
        run_timeout: std::time::Duration::from_secs(300),
        dry_run: false,
        custom_env: Default::default(),
        custom_args: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    };
    let invocation = crate::task_graph::llm::ResolvedLlmInvocation {
        run_as: LlmRunAs::Llm,
        runtime: "opencode".to_string(),
        agent: "native".to_string(),
        model: None,
        variant: None,
        prompt: "hello".to_string(),
        output: None,
        skills: Vec::new(),
        mcp_servers: Vec::new(),
        custom_env: Default::default(),
        custom_args: Vec::new(),
        tool_policy: None,
        opencode_config_content: None,
        codex_config_args: Vec::new(),
        codebuddy_mcp_config_content: None,
        codebuddy_settings_json: None,
        pi_mcp_config_content: None,
    };

    let output =
        crate::task_graph::runtime::run_runtime_command(&opts, &invocation, "test-project", "llm")
            .unwrap();

    assert!(output.status.success());
    let actual_cwd = std::path::PathBuf::from(fs::read_to_string(cwd_file).unwrap().trim());
    assert_eq!(
        actual_cwd.canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
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
fn pi_llm_node_creates_agent_session() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let pi_path = fake_pi_path(root, "pi llm ok");
    let graph = simple_llm_graph(json!({
        "runtime": "pi",
        "agent": "pi",
        "prompt": { "mode": "inline", "template": "hello" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (_run_id, node) = run_pi_agent_session_graph(root, &graph, pi_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let session_id = node.agent_session_id.expect("pi node has AgentSession");
    assert_eq!(node.log_tail, None);
    let session = crate::agent_session::read_session(root, "test-project", &session_id).unwrap();
    assert_eq!(session.runtime, "pi");
    let mcp_config_path = crate::agent_session::session_dir(root, "test-project", &session_id)
        .join("artifacts/pi-mcp/mcp.json");
    assert!(mcp_config_path.is_file());
    assert!(!root.join(".pi/mcp.json").exists());
    assert!(!root.join(".mcp.json").exists());
    let pi_args = fs::read_to_string(root.join("pi-args.txt")).unwrap();
    assert!(pi_args.contains("--mcp-config"));
    assert!(pi_args
        .replace('\\', "/")
        .contains("artifacts/pi-mcp/mcp.json"));
    assert_eq!(
        session.provider_session_id.as_deref(),
        Some("fake-pi-session")
    );
    let events =
        crate::agent_session::read_events(root, "test-project", &session_id, None).unwrap();
    assert!(events.iter().any(|event| event.event_type
        == crate::agent_session::AgentEventType::Text
        && event.content.as_deref() == Some("pi llm ok")));
    assert!(events
        .iter()
        .any(|event| event.event_type == crate::agent_session::AgentEventType::UsageUpdate));
}

#[test]
fn pi_provider_tool_events_project_to_task_graph_tool_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let pi_path = fake_pi_tool_path(root, "pi tool lifecycle ok");
    let graph = simple_llm_graph(json!({
        "runtime": "pi",
        "agent": "pi",
        "prompt": { "mode": "inline", "template": "hello" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (run_id, node) = run_pi_agent_session_graph(root, &graph, pi_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    let events = run_state::list_run_events(root, "test-project", &run_id).unwrap();
    let mcp_start = events
        .iter()
        .find(|event| {
            event.kind == "tool_start"
                && event.payload["tool_kind"] == json!("mcp_tool")
                && event.payload["tool_name"] == json!("bb.read_ticket_by_id")
        })
        .expect("Pi MCP tool start is projected into task graph lifecycle");
    assert_eq!(
        mcp_start.payload["input_summary"]["daemon_backed"],
        json!(true)
    );
    assert_eq!(
        mcp_start.payload["input_summary"]["mcp_tool"],
        json!("read_ticket_by_id")
    );

    let mcp_call_id = mcp_start.payload["tool_call_id"].as_str().unwrap();
    let mcp_end = events
        .iter()
        .find(|event| {
            event.kind == "tool_end"
                && event.payload["tool_kind"] == json!("mcp_tool")
                && event.payload["tool_call_id"] == json!(mcp_call_id)
        })
        .expect("Pi MCP tool end is projected into task graph lifecycle");
    assert_eq!(mcp_end.payload["status"], json!("succeeded"));

    let bash_update = events
        .iter()
        .find(|event| event.kind == "tool_update" && event.payload["tool_name"] == json!("bash"))
        .expect("Pi shell tool update is projected into task graph lifecycle");
    assert_eq!(bash_update.payload["status"], json!("running"));
    assert_eq!(
        bash_update.payload["output_summary"]["output_artifactized"],
        json!(true)
    );
    assert_eq!(bash_update.payload["sequence"], json!(bash_update.seq));
    let artifact_path = bash_update.payload["artifacts"][0]["path"]
        .as_str()
        .expect("tool update stores long output as artifact");
    assert!(root
        .join("runtime/task_graph_runs/test-project")
        .join(&run_id)
        .join(artifact_path)
        .exists());
}

#[test]
fn pi_llm_node_uses_committed_skill_snapshot_instead_of_shared_pi_skills() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let skill_dir = root.join("skills/code-research");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: code-research\ndescription: Code research skill\n---\n\n# Code Research",
    )
    .unwrap();

    let pi_path = fake_pi_path(root, "pi llm ok");
    let graph = simple_llm_graph(json!({
        "runtime": "pi",
        "agent": "pi",
        "skills": ["code-research"],
        "prompt": { "mode": "inline", "template": "hello" },
        "output": { "artifact_type": "text", "required": false }
    }));

    let (run_id, node) = run_pi_agent_session_graph(root, &graph, pi_path);

    assert_eq!(node.status, run_state::NodeRunStatus::Succeeded);
    assert!(!root.join(".pi/skills/code-research/SKILL.md").exists());

    let snapshot_root = root
        .join("runtime/task_graph_runs/test-project")
        .join(run_id)
        .join("skill_snapshots");
    let snapshots = fs::read_dir(snapshot_root).unwrap().count();
    assert_eq!(snapshots, 1);
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
