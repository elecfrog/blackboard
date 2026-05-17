use crate::mcp_tools;
use bb_core::Workspace;
use serde_json::{json, Value};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn fixture() -> (TempDir, Workspace) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    let kb_root = root.join("projects/demo");
    fs::create_dir_all(kb_root.join("inbox")).unwrap();
    fs::create_dir_all(kb_root.join("tickets")).unwrap();
    fs::create_dir_all(root.join("agents")).unwrap();
    fs::write(root.join("agents/AGENTS.md"), "# Demo Agent Rules\n").unwrap();
    fs::write(root.join("agents/CODEX.md"), "# Demo Codex Rules\n").unwrap();
    fs::write(
        kb_root.join("__project__.json"),
        r##"{
  "name": "demo",
  "type": "workflow",
  "repos": [],
  "description": "demo fixture",
  "lanes": [
    { "id": "bbt", "label": "后端", "color": "#0f766e", "description": "", "status": "active" },
    { "id": "bbd", "label": "前端", "color": "#2563eb", "description": "", "status": "active" },
    { "id": "bbp", "label": "产品", "color": "#7c3aed", "description": "", "status": "active" },
    { "id": "bbq", "label": "质量", "color": "#ea580c", "description": "", "status": "active" }
  ]
}"##,
    )
    .unwrap();
    fs::write(
        kb_root.join("inbox/2026-05-04-codex-test.md"),
        "hello\nContext note",
    )
    .unwrap();
    fs::write(
            kb_root.join("tickets/bbt-000001-test.md"),
            "+++\nid = \"000001\"\nfamily = \"bbt\"\ntitle = \"Test Ticket\"\nupdated_at = \"2026-05-05\"\nassignee = \"alice\"\ncurrent = \"ticket hello\"\nstatus = \"todo\"\n+++\n\nTicket Context",
        )
        .unwrap();
    fs::write(
        root.join("projects/__projects__.json"),
        r#"{
  "projects": {
    "demo": {
      "uuid": "11111111-1111-4111-8111-111111111111",
      "locations": {
        "CURRENT": {
          "absolute_path": "",
          "relative_path": "./demo"
        }
      }
    }
  },
  "machines": {
    "CURRENT": "TEST"
  }
}"#,
    )
    .unwrap();
    (temp, Workspace::open(root).unwrap())
}

fn parse(line: String) -> Value {
    serde_json::from_str(&line).unwrap()
}

fn handle_line(workspace: &Workspace, line: &str) -> Option<String> {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                -32700,
                format!("parse error: {err}"),
            ));
        }
    };

    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let has_id = request.get("id").is_some();
    let method = request.get("method").and_then(Value::as_str);

    let response = match method {
        Some("tools/list") => Ok(mcp_tools::tools_list()),
        Some("tools/call") => {
            mcp_tools::handle_tool_call(workspace, request.get("params").unwrap_or(&Value::Null))
        }
        Some("notifications/initialized") => return None,
        Some(other) => Err((-32601, format!("method not found: {other}"))),
        None => Err((-32600, "missing method".to_string())),
    };

    if !has_id {
        return None;
    }

    Some(match response {
        Ok(result) => success_response(id, result),
        Err((code, message)) => error_response(id, code, message),
    })
}

fn call_tool(workspace: &Workspace, id: u64, name: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments,
        },
    });
    parse(handle_line(workspace, &request.to_string()).unwrap())
}

fn standard_tool_payload(response: &Value, tool_name: &str) -> Value {
    if response.get("error").is_some() {
        panic!("{tool_name} returned JSON-RPC error: {response}");
    }
    let Some(content) = response["result"]["content"].as_array() else {
        panic!("{tool_name} did not return result.content: {response}");
    };
    assert_eq!(
        content.len(),
        1,
        "{tool_name} should return exactly one content item"
    );
    assert_eq!(content[0]["type"], "text", "{tool_name} content type");
    let text = content[0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool_name} content text should be a string"));
    serde_json::from_str(text)
        .unwrap_or_else(|err| panic!("{tool_name} content text should be JSON: {err}; {text}"))
}

fn sample_tool_arguments(name: &str) -> Value {
    match name {
        "list_projects" => json!({}),
        "find_work_context" => json!({ "project": "demo", "query": "Context" }),
        "list_agents" => json!({}),
        "upsert_agent" => json!({
            "id": "test-agent",
            "display_name": "Test Agent",
            "kind": "platform_agent",
            "runtime": "codex",
            "roles": ["engineering"],
        }),
        "list_inbox_notes" => json!({ "project": "demo" }),
        "read_inbox_note" => json!({
            "project": "demo",
            "name": "2026-05-04-codex-test.md",
        }),
        "delete_inbox_note" => json!({
            "project": "demo",
            "name": "2026-05-04-codex-test.md",
        }),
        "list_tickets" => json!({ "project": "demo" }),
        "read_ticket" => json!({
            "project": "demo",
            "name": "bbt-000001-test.md",
        }),
        "read_ticket_by_id" => json!({ "project": "demo", "id": "000001" }),
        "create_ticket" => json!({
            "project": "demo",
            "lane": "bbd",
            "title": "Tool Contract Ticket",
            "status": "todo",
            "slug": "tool-contract-ticket",
            "sections": { "progress": ["created for tool contract test"] },
        }),
        "update_ticket" => json!({
            "project": "demo",
            "id": "000002",
            "frontmatter": {
                "title": "Tool Contract Ticket Updated",
                "status": "review",
            },
        }),
        "deprecate_ticket" => json!({
            "project": "demo",
            "id": "000002",
        }),
        "append_ticket_sections" => json!({
            "project": "demo",
            "id": "000001",
            "progress": ["tool contract append"],
        }),
        "begin_ticket_work" => json!({
            "project": "demo",
            "id": "000001",
            "agent": "test-agent",
            "note": "tool contract start",
        }),
        "complete_handoff" => json!({
            "project": "demo",
            "id": "000001",
            "source": "codex",
            "topic": "tool-contract",
            "done": ["tool contract complete"],
            "validation": ["stdio contract test"],
        }),
        "board_summary" => json!({ "project": "demo" }),
        "list_lanes" => json!({ "project": "demo" }),
        "upsert_lane" => json!({
            "project": "demo",
            "id": "bbx",
            "label": "Extra",
            "color": "#111827",
            "description": "contract test lane",
        }),
        "archive_lane" => json!({ "project": "demo", "id": "bbx" }),
        "upsert_project_agent" => json!({
            "project": "demo",
            "agent": "test-agent",
            "role": "engineering",
            "lanes": ["bbt"],
        }),
        "remove_project_agent" => json!({
            "project": "demo",
            "agent": "test-agent",
        }),
        "search_notes" => json!({ "project": "demo", "query": "context" }),
        "search_tickets" => json!({ "project": "demo", "query": "context" }),
        "create_inbox_note" => json!({
            "project": "demo",
            "source": "codex",
            "topic": "tool-contract-note",
            "done": ["created note"],
        }),
        "list_agent_connectors" => json!({}),
        "sync_agent_connector" => json!({ "id": "codex" }),
        "disconnect_agent_connector" => json!({ "id": "codex" }),
        other => panic!("missing sample call arguments for tool `{other}`"),
    }
}

fn assert_required_fields_have_samples(tool: &Value, sample: &Value) {
    let name = tool["name"].as_str().unwrap();
    let Some(sample_object) = sample.as_object() else {
        panic!("{name} sample arguments must be an object");
    };
    let Some(schema) = tool["inputSchema"].as_object() else {
        panic!("{name} missing inputSchema object");
    };
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{name} missing inputSchema.properties"));
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required {
            let field = field
                .as_str()
                .unwrap_or_else(|| panic!("{name} required entry should be string"));
            assert!(
                properties.contains_key(field),
                "{name} requires `{field}` but does not define it in properties"
            );
            assert!(
                sample_object.contains_key(field),
                "{name} test sample is missing required `{field}`"
            );
        }
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            unsafe {
                std::env::set_var(self.key, previous);
            }
        } else {
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }
}

fn set_home_for_test(home: &Path) -> EnvVarGuard {
    EnvVarGuard::set("HOME", home.as_os_str())
}

fn success_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i64, message: String) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

#[test]
fn lists_tools_and_calls_inbox_tools_with_project_routing() {
    let (_temp, workspace) = fixture();
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _daemon = EnvVarGuard::unset("BB_DAEMON");
    let _daemon_agent = EnvVarGuard::unset("BB_DAEMON_AGENT");

    let list = parse(
        handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .unwrap(),
    );
    let tool_names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        tool_names,
        vec![
            "list_projects",
            "find_work_context",
            "list_agents",
            "upsert_agent",
            "list_inbox_notes",
            "read_inbox_note",
            "list_tickets",
            "read_ticket",
            "read_ticket_by_id",
            "create_ticket",
            "update_ticket",
            "deprecate_ticket",
            "append_ticket_sections",
            "begin_ticket_work",
            "complete_handoff",
            "board_summary",
            "list_lanes",
            "upsert_lane",
            "archive_lane",
            "upsert_project_agent",
            "remove_project_agent",
            "search_notes",
            "search_tickets",
            "create_inbox_note",
            "list_agent_connectors",
            "sync_agent_connector",
            "disconnect_agent_connector",
        ]
    );

    let projects = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_projects","arguments":{}}}"#,
        ).unwrap());
    let text = projects["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload[0]["name"], "demo");
    assert_eq!(payload[0]["meta"]["type"], "workflow");

    let call = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read_inbox_note","arguments":{"project":"demo","name":"2026-05-04-codex-test.md"}}}"#,
        ).unwrap());
    assert!(call["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("hello"));

    let denied = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"delete_inbox_note","arguments":{"project":"demo","name":"2026-05-04-codex-test.md"}}}"#,
        ).unwrap());
    assert_eq!(denied["error"]["code"], -32603);
}

#[test]
fn all_listed_tools_return_standard_content_results() {
    let (temp, workspace) = fixture();
    let _env_lock = ENV_LOCK.lock().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let _home = set_home_for_test(&home);

    let list = parse(
        handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":100,"method":"tools/list"}"#,
        )
        .unwrap(),
    );
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());

    for (index, tool) in tools.iter().enumerate() {
        let name = tool["name"].as_str().unwrap();
        let sample = sample_tool_arguments(name);
        assert_required_fields_have_samples(tool, &sample);
        let response = call_tool(&workspace, 200 + index as u64, name, sample);
        let _payload = standard_tool_payload(&response, name);
    }
}

#[test]
fn daemon_only_delete_inbox_note_returns_standard_content_result() {
    let (_temp, workspace) = fixture();
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _daemon = EnvVarGuard::set("BB_DAEMON", "1");
    let _daemon_agent = EnvVarGuard::set("BB_DAEMON_AGENT", "bb-pm");

    let list = parse(
        handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":300,"method":"tools/list"}"#,
        )
        .unwrap(),
    );
    let tool_names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"delete_inbox_note"));

    let response = call_tool(
        &workspace,
        301,
        "delete_inbox_note",
        sample_tool_arguments("delete_inbox_note"),
    );
    let _payload = standard_tool_payload(&response, "delete_inbox_note");
}

#[test]
fn missing_project_is_rejected_with_parameter_error() {
    let (_temp, workspace) = fixture();

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_inbox_notes","arguments":{}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("missing required project"));

    let unknown = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_inbox_notes","arguments":{"project":"does-not-exist"}}}"#,
        ).unwrap());
    assert_eq!(unknown["error"]["code"], -32602);
    assert!(unknown["error"]["message"]
        .as_str()
        .unwrap()
        .contains("project not found"));
}

#[test]
fn creates_note_and_rejects_traversal_over_json_rpc() {
    let (_temp, workspace) = fixture();

    let create = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_inbox_note","arguments":{"project":"demo","title":"stdio smoke","source":"OpenCode","topic":"stdio create","done":["created"],"validation":["stdio test"],"next_step":["review"],"related_locations":["blackboard/bb-server"]}}}"#,
        ).unwrap());
    assert!(create["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("opencode-stdio-create.md"));

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"read_inbox_note","arguments":{"project":"demo","name":"../tickets/x.md"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
}

#[test]
fn lists_and_reads_ticket_tools() {
    let (_temp, workspace) = fixture();

    let listed = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_tickets","arguments":{"project":"demo"}}}"#,
        ).unwrap());
    let text = listed["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("tickets/bbt-000001-test.md"));
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["tickets"][0]["id"], "000001");
    assert_eq!(payload["tickets"][0]["status"], "todo");

    let read = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"read_ticket","arguments":{"project":"demo","name":"bbt-000001-test.md"}}}"#,
        ).unwrap());
    assert!(read["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("ticket hello"));

    let read_by_id = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":61,"method":"tools/call","params":{"name":"read_ticket_by_id","arguments":{"project":"demo","id":"000001"}}}"#,
        ).unwrap());
    let text = read_by_id["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["id"], "000001");
    assert!(payload["content"]
        .as_str()
        .unwrap()
        .contains("Ticket Context"));

    let summary = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":62,"method":"tools/call","params":{"name":"board_summary","arguments":{"project":"demo"}}}"#,
        ).unwrap());
    let text = summary["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["total"], 1);
    assert_eq!(payload["by_status"]["todo"], 1);
}

#[test]
fn creates_and_updates_tickets_over_json_rpc() {
    let (_temp, workspace) = fixture();

    // `assignee` is no longer a top-level parameter; it is provided via
    // the open `extra` KV map. `current` is gone entirely. The
    // `lane` field replaces the former hard-coded `family` enum and is
    // validated against the project's lane catalog.
    let created = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":64,"method":"tools/call","params":{"name":"create_ticket","arguments":{"project":"demo","lane":"bbd","title":"Structured Ticket","status":"todo","slug":"structured ticket","extra":{"assignee":"opencode"},"sections":{"progress":["created from MCP"]}}}}"#,
        ).unwrap());
    let text = created["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["ticket"]["id"], "000002");
    assert_eq!(payload["ticket"]["lane"], "bbd");
    assert_eq!(payload["ticket"]["status"], "todo");
    assert_eq!(payload["ticket"]["extra"]["assignee"], "opencode");
    assert_eq!(payload["maintenance"]["consistency"]["status"], "passed");

    // Update path mirrors the KV contract: title/status are first-class,
    // everything else flows through `extra` (upsert) and `remove` (delete).
    let updated = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":65,"method":"tools/call","params":{"name":"update_ticket","arguments":{"project":"demo","id":"000002","frontmatter":{"title":"Structured Done","status":"done","extra":{"assignee":"codex"}}}}}"#,
        ).unwrap());
    let text = updated["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["ticket"]["status"], "done");
    assert_eq!(payload["ticket"]["title"], "Structured Done");
    assert_eq!(payload["ticket"]["extra"]["assignee"], "codex");

    let read = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":66,"method":"tools/call","params":{"name":"read_ticket_by_id","arguments":{"project":"demo","id":"000002"}}}"#,
        ).unwrap());
    let text = read["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["status"], "done");
    assert!(payload["content"]
        .as_str()
        .unwrap()
        .contains("created from MCP"));
}

#[test]
fn rejects_raw_or_unknown_ticket_write_fields() {
    let (_temp, workspace) = fixture();

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":69,"method":"tools/call","params":{"name":"create_inbox_note","arguments":{"project":"demo","source":"opencode","topic":"unknown","raw_markdown":"patch"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `raw_markdown`"));

    // Caller-provided `id` must remain rejected so the server keeps full
    // ownership of ID allocation. Use an otherwise-valid KV-shaped body
    // (lane provided correctly) so the only unknown field is `id`.
    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":67,"method":"tools/call","params":{"name":"create_ticket","arguments":{"project":"demo","id":"000999","lane":"bbd","title":"Bad","status":"todo","extra":{"assignee":"opencode"}}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `id`"));

    // Post-KV migration: top-level `assignee` is no longer accepted. It
    // must be supplied via `extra`. The legacy `family` field has been
    // renamed to `lane` and is rejected at the same level here.
    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":70,"method":"tools/call","params":{"name":"create_ticket","arguments":{"project":"demo","lane":"bbd","title":"Bad","assignee":"opencode","status":"todo"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `assignee`"));

    // The former top-level `family` field is also unknown now.
    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":72,"method":"tools/call","params":{"name":"create_ticket","arguments":{"project":"demo","lane":"bbd","family":"bbd","title":"Bad","status":"todo"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `family`"));

    // update_ticket frontmatter patch: legacy `current` must be routed
    // through `remove` or `extra`, not as a first-class key.
    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":71,"method":"tools/call","params":{"name":"update_ticket","arguments":{"project":"demo","id":"000001","frontmatter":{"current":"still here"}}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `current`"));

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":68,"method":"tools/call","params":{"name":"update_ticket","arguments":{"project":"demo","id":"000001","frontmatter":{"status":"done","raw_markdown":"patch"}}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown field `raw_markdown`"));
}

#[test]
fn rejects_unsafe_ticket_tool_arguments() {
    let (_temp, workspace) = fixture();

    // Traversal in name is rejected
    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"read_ticket","arguments":{"project":"demo","name":"..%2Fdone%2Fbbt-000001-test.md"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
}

#[test]
fn searches_notes_and_tickets() {
    let (_temp, workspace) = fixture();

    let notes = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"search_notes","arguments":{"project":"demo","query":"context"}}}"#,
        ).unwrap());
    let note_text = notes["result"]["content"][0]["text"].as_str().unwrap();
    let note_payload: Value = serde_json::from_str(note_text).unwrap();
    assert_eq!(
        note_payload["matches"][0]["path"],
        "inbox/2026-05-04-codex-test.md"
    );
    assert_eq!(note_payload["matches"][0]["line"], 2);
    assert_eq!(note_payload["matches"][0]["snippet"], "Context note");

    let tickets = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"search_tickets","arguments":{"project":"demo","query":"context"}}}"#,
        ).unwrap());
    let ticket_text = tickets["result"]["content"][0]["text"].as_str().unwrap();
    let ticket_payload: Value = serde_json::from_str(ticket_text).unwrap();
    assert_eq!(ticket_payload["matches"][0]["status"], "todo");
    assert_eq!(
        ticket_payload["matches"][0]["path"],
        "tickets/bbt-000001-test.md"
    );
    assert_eq!(ticket_payload["matches"][0]["line"], 11);
    assert_eq!(ticket_payload["matches"][0]["snippet"], "Ticket Context");
}

#[test]
fn rejects_empty_search_queries() {
    let (_temp, workspace) = fixture();

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"search_notes","arguments":{"project":"demo","query":"   "}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);

    let rejected = parse(handle_line(
            &workspace,
            r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"search_tickets","arguments":{"project":"demo"}}}"#,
        ).unwrap());
    assert_eq!(rejected["error"]["code"], -32602);
}

#[test]
fn workflow_tools_find_begin_and_complete_ticket_work() {
    let (temp, workspace) = fixture();

    let context = parse(handle_line(
        &workspace,
        r#"{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"find_work_context","arguments":{"project":"demo","query":"Context"}}}"#,
    )
    .unwrap());
    let context_text = context["result"]["content"][0]["text"].as_str().unwrap();
    let context_payload: Value = serde_json::from_str(context_text).unwrap();
    let context_id = context_payload["projects"][0]["active_tickets"][0]["id"]
        .as_str()
        .unwrap();
    assert_eq!(context_id, "000001");

    let started = parse(handle_line(
        &workspace,
        r#"{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"begin_ticket_work","arguments":{"project":"demo","id":"000001","note":"test start"}}}"#,
    )
    .unwrap());
    let started_text = started["result"]["content"][0]["text"].as_str().unwrap();
    let started_payload: Value = serde_json::from_str(started_text).unwrap();
    assert_eq!(started_payload["ticket"]["status"], "in_progress");

    let completed = parse(handle_line(
        &workspace,
        r#"{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{"name":"complete_handoff","arguments":{"project":"demo","id":"000001","source":"codex","topic":"workflow-tools","done":["implemented"],"validation":["cargo test"]}}}"#,
    )
    .unwrap());
    let completed_text = completed["result"]["content"][0]["text"].as_str().unwrap();
    let completed_payload: Value = serde_json::from_str(completed_text).unwrap();
    assert!(completed_payload["note"]["path"]
        .as_str()
        .unwrap()
        .contains("codex-workflow-tools"));
    let inbox_files: Vec<_> = fs::read_dir(temp.path().join("blackboard/projects/demo/inbox"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(inbox_files
        .iter()
        .any(|name| name.contains("codex-workflow-tools")));
}

#[test]
fn notifications_do_not_write_protocol_frames() {
    let (_temp, workspace) = fixture();
    assert!(handle_line(
        &workspace,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
    )
    .is_none());
}
