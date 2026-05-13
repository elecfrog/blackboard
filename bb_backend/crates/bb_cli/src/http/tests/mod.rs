use super::*;
use axum::body::{to_bytes, Body};
use axum::http::{Method, Request};
use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

fn fixture() -> (TempDir, Workspace) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    let kb = root.join("projects/demo");
    fs::create_dir_all(kb.join("inbox")).unwrap();
    fs::create_dir_all(kb.join("tickets")).unwrap();
    fs::write(
        kb.join("__project__.json"),
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
    fs::write(kb.join("tickets/sentinel.md"), "unchanged").unwrap();
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

async fn json_response(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn healthz_returns_ok() {
    let (_temp, workspace) = fixture();
    let app = app(workspace);

    let response = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "ok\n");
}

#[tokio::test]
async fn static_dir_serves_assets_and_spa_fallback_without_masking_api() {
    let temp = TempDir::new().unwrap();
    let static_dir = temp.path().join("dist");
    fs::create_dir_all(static_dir.join("assets")).unwrap();
    fs::write(static_dir.join("index.html"), "<html>blackboard</html>").unwrap();
    fs::write(static_dir.join("assets/app.js"), "console.log('bb')").unwrap();

    let (_workspace_temp, workspace) = fixture();
    let app = app_with_static(workspace, Some(static_dir));

    let response = app
        .clone()
        .oneshot(Request::get("/assets/app.js").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "console.log('bb')");

    let response = app
        .clone()
        .oneshot(Request::get("/tickets").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "<html>blackboard</html>");

    let response = app
        .oneshot(Request::get("/api/unknown").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rest_lists_projects_and_routes_inbox_by_project() {
    let (temp, workspace) = fixture();
    fs::write(
        workspace
            .projects_root()
            .join("demo/inbox/2026-05-04-codex-test.md"),
        "hello",
    )
    .unwrap();
    let app = app(workspace);

    let response = app
        .clone()
        .oneshot(Request::get("/api/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["projects"][0]["name"], "demo");

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/projects/demo/inbox/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await["notes"][0]["name"],
        "2026-05-04-codex-test.md"
    );

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/projects/demo/inbox/notes/2026-05-04-codex-test.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["content"], "hello");

    let body = json!({
        "title": "REST smoke",
        "time": "2026-05-04T00:00:00Z",
        "source": "OpenCode",
        "topic": "REST create",
        "done": ["created a note"],
        "validation": ["rest test"],
        "next_step": ["review"],
        "related_locations": ["blackboard/bb-server"]
    });
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/projects/demo/inbox/notes")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_response(response).await;
    assert!(created["name"]
        .as_str()
        .unwrap()
        .ends_with("opencode-rest-create.md"));
    assert_eq!(
        created["path"].as_str().unwrap(),
        format!("inbox/{}", created["name"].as_str().unwrap())
    );

    assert_eq!(
        fs::read_to_string(
            temp.path()
                .join("blackboard/projects/demo/tickets/sentinel.md")
        )
        .unwrap(),
        "unchanged"
    );
}

#[tokio::test]
async fn rest_routes_project_requests_to_external_data_root() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    let external = temp.path().join("sample-harness");
    fs::create_dir_all(root.join("projects")).unwrap();
    fs::create_dir_all(external.join("inbox")).unwrap();
    fs::create_dir_all(external.join("tickets")).unwrap();
    fs::create_dir_all(external.join("wiki")).unwrap();

    let data_root = external.to_string_lossy().to_string();
    fs::write(
        root.join("projects/__projects__.json"),
        format!(
            r#"{{
  "projects": {{
    "Sample-BB": {{
      "uuid": "14080830-ebb4-40ef-8228-95da445b5220",
      "locations": {{
        "CURRENT": {{
          "absolute_path": {data_root:?},
          "relative_path": ""
        }}
      }}
    }}
  }},
  "machines": {{
    "CURRENT": "TEST"
  }}
}}"#
        ),
    )
    .unwrap();
    fs::write(
        external.join("__project__.json"),
        r##"{
  "name": "Sample",
  "type": "tool",
  "repos": [],
  "description": "external fixture",
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
        external.join("__tickets__.json"),
        "{\n  \"current_counter\": \"000000\",\n  \"tickets\": []\n}",
    )
    .unwrap();
    fs::write(external.join("__inbox__.json"), "{\n  \"notes\": []\n}").unwrap();
    fs::write(external.join("inbox/2026-05-08-note.md"), "external").unwrap();

    let app = app(Workspace::open(root).unwrap());

    let response = app
        .clone()
        .oneshot(Request::get("/api/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let canonical_data_root = bb_core::path_to_string(&external.canonicalize().unwrap());
    assert_eq!(body["projects"][0]["name"], "Sample-BB");
    assert_eq!(
        body["projects"][0]["uuid"],
        "14080830-ebb4-40ef-8228-95da445b5220"
    );
    assert_eq!(body["projects"][0]["meta"]["name"], "Sample");
    assert_eq!(
        body["projects"][0]["meta"]["data_root"],
        canonical_data_root
    );

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/projects/Sample-BB/inbox/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await["notes"][0]["name"],
        "2026-05-08-note.md"
    );
}

#[tokio::test]
async fn rest_creates_opens_with_explicit_user_project_name() {
    let (temp, workspace) = fixture();
    // The directory basename is deliberately different from the user-chosen
    // Project ID; paths must not define project identity.
    let data_root = temp.path().join("Sample.BB");
    let data_root_text = data_root.to_string_lossy().to_string();
    let app = app(workspace);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/projects/create")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "aa",
                        "display_name": "AA",
                        "type": "tool",
                        "data_root": data_root_text,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["name"], "aa");
    assert_eq!(body["meta"]["name"], "AA");
    assert!(data_root.join("__project__.json").exists());
    assert!(data_root.join("__tickets__.json").exists());
    assert!(data_root.join("__inbox__.json").exists());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/projects/open")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "aa-copy",
                        "data_root": data_root.to_string_lossy(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn workspace_folder_open_initializes_dotbb_and_registers_global_project() {
    let (temp, workspace) = fixture();
    let app = app(workspace);

    let code_root = temp.path().join("Code Root");
    fs::create_dir_all(code_root.join("src")).unwrap();
    fs::write(code_root.join("src/main.rs"), "fn main() {}\n").unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/workspace/folder/open")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "root": code_root.to_string_lossy(),
                        "initialize": true,
                        "project_name": "Ignored Choice",
                        "display_name": "Ignored Choice"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["active_project"], "code-root");
    assert!(!body.to_string().contains(r"\\?\"));
    assert!(code_root.join(".bb/__project__.json").is_file());
    assert!(code_root.join(".bb/__tickets__.json").is_file());
    assert!(code_root.join(".bb/__inbox__.json").is_file());
    assert!(code_root.join(".bb/inbox").is_dir());
    assert!(code_root.join(".bb/tickets").is_dir());
    assert!(code_root.join(".bb/wiki").is_dir());
    assert!(!code_root.join(".bb/blackboard.json").exists());
    assert!(!code_root.join(".bb/projects").exists());
    assert!(!code_root.join(".bb/agents").exists());
    assert!(!code_root.join(".bb/task_graphs").exists());
    assert!(!code_root.join(".bb/templates").exists());
    assert!(!code_root.join(".bb/runtime").exists());
    assert!(!code_root.join(".gitignore").exists());
    let meta =
        bb_core::read_json_file::<bb_core::ProjectMeta>(&code_root.join(".bb/__project__.json"))
            .unwrap();
    assert_eq!(meta.name, "Code Root");

    let response = app
        .clone()
        .oneshot(Request::get("/api/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let projects = json_response(response).await;
    assert!(!projects.to_string().contains(r"\\?\"));
    let project_names: Vec<_> = projects["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|project| project["name"].as_str().unwrap())
        .collect();
    assert_eq!(project_names, vec!["code-root", "demo"]);

    let response = app
        .oneshot(
            Request::get("/api/projects/code-root/inbox/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn workspace_folder_open_rejects_invalid_existing_dotbb() {
    let (temp, workspace) = fixture();
    let app = app(workspace);

    let code_root = temp.path().join("invalid");
    fs::create_dir_all(code_root.join(".bb")).unwrap();
    fs::write(code_root.join(".bb/sentinel.txt"), "keep").unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/workspace/folder/open")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "root": code_root.to_string_lossy(),
                        "initialize": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        fs::read_to_string(code_root.join(".bb/sentinel.txt")).unwrap(),
        "keep"
    );
    assert!(!code_root.join(".bb/__project__.json").exists());
}

#[tokio::test]
async fn remote_mcp_initialize_and_list_tools() {
    let (_temp, workspace) = fixture();
    let router = app(workspace);

    // Step 1: Send initialize request (creates a session)
    let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(init_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // Extract session ID from response headers
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .expect("initialize response must include Mcp-Session-Id header")
        .to_str()
        .unwrap()
        .to_string();
    assert!(!session_id.is_empty());

    // Parse SSE response body to extract the initialize result
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    // SSE format: lines starting with "data:" contain the JSON-RPC response
    let init_result = extract_json_from_sse(&body_str);
    assert_eq!(init_result["result"]["serverInfo"]["name"], "bb");

    // Step 2: Send notifications/initialized notification
    let initialized_body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", &session_id)
                .body(Body::from(initialized_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Step 3: Send tools/list request with session ID
    let tools_body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", &session_id)
                .body(Body::from(tools_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let tools_result = extract_json_from_sse(&body_str);
    assert_eq!(tools_result["jsonrpc"], "2.0");
    assert_eq!(tools_result["result"]["tools"][0]["name"], "list_projects");
}

/// Extract the first JSON-RPC message from an SSE response body.
fn extract_json_from_sse(sse_body: &str) -> Value {
    for line in sse_body.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if let Ok(value) = serde_json::from_str::<Value>(data) {
                if value.get("jsonrpc").is_some() {
                    return value;
                }
            }
        }
    }
    panic!("No JSON-RPC message found in SSE response: {sse_body}");
}

/// Helper: perform the full MCP initialize handshake and return the session ID.
async fn mcp_initialize(router: &Router) -> String {
    let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(init_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Send initialized notification
    let initialized_body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", &session_id)
                .body(Body::from(initialized_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    session_id
}

async fn mcp_call_tool(router: &Router, session_id: &str, name: &str, arguments: Value) -> Value {
    let call_body = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    });
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", session_id)
                .body(Body::from(call_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    extract_json_from_sse(&body_str)
}

#[tokio::test]
async fn mcp_list_projects_follows_global_folder_registry() {
    let (temp, workspace) = fixture();
    let router = app(workspace);
    let session_id = mcp_initialize(&router).await;

    let code_root = temp.path().join("MCP Code");
    fs::create_dir_all(&code_root).unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/workspace/folder/open")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "root": code_root.to_string_lossy(),
                        "initialize": true,
                        "project_name": "Ignored MCP Choice",
                        "display_name": "Ignored MCP Choice"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let result = mcp_call_tool(&router, &session_id, "list_projects", json!({})).await;
    let text = result["result"]["content"][0]["text"].as_str().unwrap();
    let projects: Value = serde_json::from_str(text).unwrap();
    let project_names: Vec<_> = projects
        .as_array()
        .unwrap()
        .iter()
        .map(|project| project["name"].as_str().unwrap())
        .collect();
    assert_eq!(project_names, vec!["demo", "mcp-code"]);
}

#[tokio::test]
async fn mcp_tools_call_returns_content() {
    let (_temp, workspace) = fixture();
    let router = app(workspace);
    let session_id = mcp_initialize(&router).await;

    // Call list_projects tool
    let result = mcp_call_tool(&router, &session_id, "list_projects", json!({})).await;
    assert_eq!(result["jsonrpc"], "2.0");
    // The result should have content array with text type
    let content = &result["result"]["content"];
    assert!(content.is_array());
    assert_eq!(content[0]["type"], "text");
    // The text should be valid JSON containing projects
    let text = content[0]["text"].as_str().unwrap();
    let projects: Value = serde_json::from_str(text).unwrap();
    assert!(projects.is_array());
}

#[tokio::test]
async fn mcp_missing_session_id_returns_not_found() {
    let (_temp, workspace) = fixture();
    let router = app(workspace);
    let session_id = mcp_initialize(&router).await;
    let _ = session_id; // just to create a valid session

    // Send tools/list with a bogus session ID
    let tools_body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", "nonexistent-session-id")
                .body(Body::from(tools_body))
                .unwrap(),
        )
        .await
        .unwrap();

    // MCP spec: server MUST respond with 404 for unknown sessions
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mcp_delete_terminates_session() {
    let (_temp, workspace) = fixture();
    let router = app(workspace);
    let session_id = mcp_initialize(&router).await;

    // DELETE the session
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/mcp")
                .header("host", "localhost")
                .header("mcp-session-id", &session_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Subsequent request with the same session ID should return 404
    let tools_body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-session-id", &session_id)
                .body(Body::from(tools_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rest_returns_not_found_for_unknown_project() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/unknown/inbox/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_response(response).await["error"]["code"], "not_found");
}

#[tokio::test]
async fn rest_rejects_invalid_project_name() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/bad.name/inbox/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_response(response).await["error"]["code"],
        "bad_request"
    );
}

#[tokio::test]
async fn rest_rejects_traversal_with_json_error() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/inbox/notes/..%2Ftickets%2Factive%2Fx.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_response(response).await["error"]["code"],
        "bad_request"
    );
}

#[tokio::test]
async fn rest_board_summary_reports_aggregated_counts() {
    let (temp, workspace) = fixture();
    // Replace the metadata-less sentinel with a well-formed ticket so the
    // summary captures by_status / by_lane non-trivially.
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    fs::write(
            tickets_dir.join("bbt-000001-sample.md"),
            "+++\nid = \"000001\"\nfamily = \"bbt\"\ntitle = \"Sample\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nassignee = \"alice\"\ncurrent = \"推进中\"\nstatus = \"in_progress\"\n+++\n\n# Sample\n\nbody\n",
        )
        .unwrap();
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000001\n",
    )
    .unwrap();

    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/board/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["by_status"]["in_progress"], 1);
    // Aggregation key changed from `by_family` to `by_lane` after the
    // lane refactor. The legacy fixture above writes `family = "bbt"`
    // which the lister bridges into `lane`; the count should land under
    // `by_lane["bbt"]` unchanged.
    assert_eq!(body["by_lane"]["bbt"], 1);
    assert!(body["metadata_warnings"].is_array());
    assert!(body["metadata_errors"].is_array());
}

#[tokio::test]
async fn rest_board_summary_returns_not_found_for_unknown_project() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/unknown/board/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_response(response).await["error"]["code"], "not_found");
}

#[tokio::test]
async fn rest_board_summary_rejects_invalid_project_name() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/bad.name/board/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_response(response).await["error"]["code"],
        "bad_request"
    );
}

#[tokio::test]
async fn rest_board_view_settings_round_trip_to_project_meta() {
    let (temp, workspace) = fixture();
    let app_built = app(workspace);

    let response = app_built
        .clone()
        .oneshot(
            Request::get("/api/projects/demo/board/view")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["hidden_statuses"], json!([]));

    let response = app_built
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/board/view")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"hidden_statuses":["archived","blocked","archived"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await["hidden_statuses"],
        json!(["blocked", "archived"])
    );

    let meta = fs::read_to_string(
        temp.path()
            .join("blackboard/projects/demo/__project__.json"),
    )
    .unwrap();
    assert!(meta.contains(r#""board_view""#));
    assert!(meta.contains(r#""hidden_statuses""#));
}

#[tokio::test]
async fn rest_board_view_settings_reject_hiding_every_status() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/board/view")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"hidden_statuses":["todo","in_progress","blocked","review","done","archived"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rest_lists_live_tickets_for_dashboard() {
    let (temp, workspace) = fixture();
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    fs::write(
            tickets_dir.join("000001-sample.md"),
            "+++\nid = \"000001\"\nlane = \"bbt\"\ntitle = \"Sample\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nassignee = \"codex\"\ndepends_on = \"000002\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\n正文提到 `000003` 但不会变成 RDG 依赖。\n",
        )
        .unwrap();
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000001\n",
    )
    .unwrap();

    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/tickets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["project"], "demo");
    assert_eq!(body["tickets"][0]["id"], "000001");
    assert_eq!(body["tickets"][0]["status"], "todo");
    assert_eq!(body["tickets"][0]["dependencies"][0], "000002");
    assert_eq!(
        body["tickets"][0]["dependencies"].as_array().unwrap().len(),
        1
    );
    assert_eq!(body["tickets"][0]["extra"]["assignee"], "codex");
    // After the index/content split, the list endpoint no longer returns
    // ticket body content — only structural index fields.
    assert!(body["tickets"][0]["content"].is_null());
}

#[tokio::test]
async fn rest_ticket_content_returns_markdown_body() {
    let (temp, workspace) = fixture();
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    fs::write(
            tickets_dir.join("000001-sample.md"),
            "+++\nid = \"000001\"\nlane = \"bbt\"\ntitle = \"Sample\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\n正文内容在这里。\n",
        )
        .unwrap();
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000001\n",
    )
    .unwrap();

    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/tickets/000001/content")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let content = body["content"].as_str().unwrap();
    assert!(!content.contains("+++"));
    assert!(content.contains("当前进展"));
    assert!(content.contains("正文内容在这里"));
}

#[tokio::test]
async fn rest_ticket_content_returns_not_found_for_missing_id() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/tickets/999999/content")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rest_patch_ticket_updates_status() {
    let (temp, workspace) = fixture();
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    fs::write(
            tickets_dir.join("000001-sample.md"),
            "+++\nid = \"000001\"\nlane = \"bbd\"\ntitle = \"Move me\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\nbody\n",
        )
        .unwrap();
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000001\n",
    )
    .unwrap();

    let response = app(workspace)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/tickets/000001")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"done"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["ticket"]["id"], "000001");
    assert_eq!(body["ticket"]["status"], "done");
    // File stays in place (flat directory)
    assert!(tickets_dir.join("000001-sample.md").exists());
    let updated = fs::read_to_string(tickets_dir.join("000001-sample.md")).unwrap();
    assert!(updated.contains("status = \"done\""));
}

#[tokio::test]
async fn rest_patch_ticket_updates_assignee_extra() {
    let (temp, workspace) = fixture();
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    fs::write(
            tickets_dir.join("000001-sample.md"),
            "+++\nid = \"000001\"\nlane = \"bbd\"\ntitle = \"Assign me\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\nbody\n",
        )
        .unwrap();
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000001\n",
    )
    .unwrap();

    let response = app(workspace)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/tickets/000001")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"assignee":" product "}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["ticket"]["extra"]["assignee"], "product");
    let updated = fs::read_to_string(tickets_dir.join("000001-sample.md")).unwrap();
    assert!(updated.contains("assignee = \"product\""));
}

#[tokio::test]
async fn rest_patch_ticket_updates_dependencies_and_rejects_cycles() {
    let (temp, workspace) = fixture();
    let tickets_dir = temp.path().join("blackboard/projects/demo/tickets");
    fs::remove_file(tickets_dir.join("sentinel.md")).unwrap();
    for id in ["000001", "000002", "000003"] {
        fs::write(
            tickets_dir.join(format!("{id}-sample.md")),
            format!(
                "+++\nid = \"{id}\"\nlane = \"bbd\"\ntitle = \"{id}\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\nbody\n",
            ),
        )
        .unwrap();
    }
    fs::write(
        temp.path().join("blackboard/projects/demo/.ticket-id"),
        "000003\n",
    )
    .unwrap();

    let app_built = app(workspace);
    let response = app_built
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/tickets/000002")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"depends_on":["000001"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["ticket"]["extra"]["depends_on"], "000001");
    let updated = fs::read_to_string(tickets_dir.join("000002-sample.md")).unwrap();
    assert!(updated.contains("depends_on = \"000001\""));

    let response = app_built
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/tickets/000001")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"depends_on":["000002"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_response(response).await;
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("acyclic"));
}

#[tokio::test]
async fn rest_patch_ticket_rejects_raw_markdown_field() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/tickets/000001")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"raw_markdown":"nope"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ─── Lane catalog tests ──────────────────────────────────────────────

#[tokio::test]
async fn rest_list_lanes_returns_project_catalog() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::get("/api/projects/demo/lanes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let ids: Vec<String> = body["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|lane| lane["id"].as_str().unwrap().to_string())
        .collect();
    // The fixture seeds the four historical lane ids; the response carries
    // them verbatim so the front-end can render the strip without further
    // round-trips.
    assert_eq!(ids, vec!["bbt", "bbd", "bbp", "bbq"]);
}

#[tokio::test]
async fn rest_post_lane_creates_new_entry_and_returns_201() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
            .oneshot(
                Request::post("/api/projects/demo/lanes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{"id":"ops","label":"运营","color":"#0ea5e9","description":"","status":"active"}"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_response(response).await;
    assert_eq!(body["id"], "ops");
    assert_eq!(body["status"], "active");
}

#[tokio::test]
async fn rest_patch_lane_updates_partial_fields() {
    let (_temp, workspace) = fixture();
    let app_built = app(workspace);

    // Patch existing `bbp` lane: rename label, leave color/status alone.
    let response = app_built
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/lanes/bbp")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"label":"产品-2026"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["id"], "bbp");
    assert_eq!(body["label"], "产品-2026");
    // Color must be preserved across the partial update.
    assert_eq!(body["color"], "#7c3aed");
}

#[tokio::test]
async fn rest_archive_lane_marks_status_and_reports_count() {
    let (_temp, workspace) = fixture();
    let response = app(workspace)
        .oneshot(
            Request::post("/api/projects/demo/lanes/bbq/archive")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["lane"]["id"], "bbq");
    assert_eq!(body["lane"]["status"], "archived");
    // Fixture has no tickets in `bbq`, so the affected count should be 0.
    assert_eq!(body["affected_ticket_count"], 0);
}

#[tokio::test]
async fn rest_agent_registry_write_roundtrips_project_agents() {
    let (_temp, workspace) = fixture();
    let app_built = app(workspace);

    let response = app_built
        .clone()
        .oneshot(
            Request::post("/api/agents")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"id":"codex","display_name":"Codex","kind":"platform_agent","runtime":"codex"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["id"], "codex");

    let response = app_built
        .clone()
        .oneshot(
            Request::post("/api/projects/demo/agents")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"agent":"codex","role":"owner","lanes":["bbt"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["agent"], "codex");

    let response = app_built
        .clone()
        .oneshot(
            Request::get("/api/projects/demo/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_response(response).await;
    assert_eq!(body["agents"][0]["id"], "codex");
    assert_eq!(body["agents"][0]["project_role"], "owner");

    let response = app_built
        .oneshot(
            Request::delete("/api/projects/demo/agents/codex")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_response(response).await;
    assert_eq!(body["removed"], true);
}

// ─── Wiki: tree / file / asset / upload ─────────────────────────────────────

#[tokio::test]
async fn wiki_tree_returns_empty_when_dir_missing() {
    let (_temp, workspace) = fixture();
    let app = app(workspace);
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/tree")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert!(body["tree"].as_array().unwrap().is_empty());
    assert!(!body["generated_at"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn wiki_tree_lists_nested_structure() {
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(wiki.join("modules/context-management")).unwrap();
    fs::write(wiki.join("index.md"), "# Root").unwrap();
    fs::write(wiki.join("modules/session.md"), "# Session").unwrap();
    fs::write(wiki.join("modules/context-management/index.md"), "# CM").unwrap();

    let app = app(workspace);
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/tree")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let tree = body["tree"].as_array().unwrap();
    // Dir first, then file.
    assert_eq!(tree[0]["name"], "modules");
    assert_eq!(tree[0]["path"], "modules");
    assert_eq!(tree[0]["kind"], "dir");
    assert_eq!(tree[1]["name"], "index.md");
    assert_eq!(tree[1]["path"], "index.md");
    assert_eq!(tree[1]["kind"], "file");

    let modules_children = tree[0]["children"].as_array().unwrap();
    assert_eq!(modules_children[0]["path"], "modules/context-management");
    assert_eq!(modules_children[1]["path"], "modules/session.md");
}

#[tokio::test]
async fn wiki_file_reads_nested_markdown_via_query() {
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(wiki.join("modules")).unwrap();
    fs::write(wiki.join("modules/session.md"), "# Session\nbody").unwrap();

    let app = app(workspace);
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/file?path=modules%2Fsession.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await["content"], "# Session\nbody");
}

#[tokio::test]
async fn wiki_file_rejects_escape() {
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(&wiki).unwrap();

    let app = app(workspace);
    // `..` traversal → 400.
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/file?path=..%2Ftickets%2Fsentinel.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn wiki_file_rejects_binary_only_formats() {
    // Binary-only formats (png, pdf, ...) must still 400 from the
    // document endpoint so clients know to use /wiki/asset instead.
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(&wiki).unwrap();
    fs::write(wiki.join("logo.png"), b"\x89PNG").unwrap();
    fs::write(wiki.join("paper.pdf"), b"%PDF").unwrap();

    let app = app(workspace);
    for path in ["logo.png", "paper.pdf"] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/api/projects/demo/wiki/file?path={}", path))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "expected 400 for binary format {path}"
        );
    }
}

#[tokio::test]
async fn wiki_file_serves_text_like_formats_with_content_type() {
    // SVG / JSON / YAML / source code are all valid document reads; the
    // frontend picks a renderer off the `content_type` extension hint.
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(&wiki).unwrap();
    fs::write(wiki.join("diagram.svg"), "<svg/>").unwrap();
    fs::write(wiki.join("data.json"), r#"{"a":1}"#).unwrap();

    let app = app(workspace);
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/projects/demo/wiki/file?path=diagram.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["content"], "<svg/>");
    assert_eq!(body["content_type"], "svg");

    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/file?path=data.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["content"], r#"{"a":1}"#);
    assert_eq!(body["content_type"], "json");
}

#[tokio::test]
async fn wiki_asset_serves_image_with_mime() {
    let (_temp, workspace) = fixture();
    let wiki = workspace.projects_root().join("demo/wiki");
    fs::create_dir_all(&wiki).unwrap();
    fs::write(wiki.join("diagram.svg"), "<svg/>").unwrap();

    let app = app(workspace);
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/wiki/asset?path=diagram.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/svg+xml"
    );
    assert_eq!(body_text(response).await, "<svg/>");
}

#[tokio::test]
async fn wiki_upload_creates_wiki_dir_on_first_upload_and_rebuilds_subdirs() {
    let (_temp, workspace) = fixture();
    // No wiki/ directory exists yet; upload must still succeed.
    let app = app(workspace.clone());

    // Build a multipart body with two files, one nested under modules/.
    let boundary = "----bbwikitest";
    let body = format!(
        concat!(
            "--{b}\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"index.md\"\r\n",
            "Content-Type: text/markdown\r\n\r\n",
            "# Hello\r\n",
            "--{b}\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"modules/session.md\"\r\n",
            "Content-Type: text/markdown\r\n\r\n",
            "# Session\r\n",
            "--{b}--\r\n",
        ),
        b = boundary
    );
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/projects/demo/wiki/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_response(response).await;
    assert!(result["errors"].as_array().unwrap().is_empty());
    let uploaded: Vec<&str> = result["uploaded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(uploaded.contains(&"index.md"));
    assert!(uploaded.contains(&"modules/session.md"));

    // Files land on disk at the expected locations.
    let wiki = workspace.projects_root().join("demo/wiki");
    assert_eq!(
        fs::read_to_string(wiki.join("index.md")).unwrap(),
        "# Hello"
    );
    assert_eq!(
        fs::read_to_string(wiki.join("modules/session.md")).unwrap(),
        "# Session"
    );
}

#[tokio::test]
async fn wiki_upload_blocks_path_traversal() {
    let (_temp, workspace) = fixture();
    let app = app(workspace.clone());
    let boundary = "----bbwikievil";
    let body = format!(
        concat!(
            "--{b}\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"../tickets/hack.md\"\r\n",
            "Content-Type: text/markdown\r\n\r\n",
            "pwned\r\n",
            "--{b}--\r\n",
        ),
        b = boundary
    );
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/projects/demo/wiki/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_response(response).await;
    assert!(result["uploaded"].as_array().unwrap().is_empty());
    assert!(!result["errors"].as_array().unwrap().is_empty());
    // Nothing should have leaked outside wiki/.
    let ticket = workspace.projects_root().join("demo/tickets/hack.md");
    assert!(!ticket.exists());
}

// ─── Task Graph HTTP Tests ───────────────────────────────────────────────────

fn tg_fixture() -> (TempDir, Workspace) {
    let (temp, workspace) = fixture();
    let root = workspace.root().to_path_buf();

    // Create system graph directory with fixture
    let sys_dir = root.join("task_graphs/system");
    fs::create_dir_all(&sys_dir).unwrap();
    let fixture_json =
        include_str!("../../../../../../.bb_template/task_graphs/system/frontend-smoke.json")
            .replace(
                r#""id": "frontend-smoke""#,
                r#""id": "frontend-smoke-loop""#,
            );
    fs::write(sys_dir.join("frontend-smoke-loop.json"), fixture_json).unwrap();

    // Create project task_graphs directory
    fs::create_dir_all(root.join("projects/demo/task_graphs")).unwrap();

    // Ensure agents directory exists (tasks.toml removed; prompts may live here)
    let agents_dir = root.join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    (temp, workspace)
}

fn minimal_project_graph_json() -> Value {
    json!({
        "schema_version": 1,
        "id": "my-test-graph",
        "scope": "project",
        "title": "My Test Graph",
        "version": 1,
        "readonly": false,
        "nodes": [
            { "id": "start", "type": "start", "label": "Start", "config": {} },
            { "id": "end-success", "type": "end", "label": "End", "config": { "result": "succeeded" } }
        ],
        "edges": [
            { "id": "start__end-success", "from": "start", "to": "end-success", "kind": "control" }
        ]
    })
}

#[tokio::test]
async fn tg_list_catalog_returns_system_graphs() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    let response = app
        .oneshot(
            Request::get("/api/projects/demo/task-graphs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let graphs = body["graphs"].as_array().unwrap();
    assert!(graphs
        .iter()
        .any(|g| g["id"] == "frontend-smoke-loop" && g["scope"] == "system"));
}

#[tokio::test]
async fn tg_read_system_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    let response = app
        .oneshot(
            Request::get("/api/projects/demo/task-graphs/system/frontend-smoke-loop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["graph"]["id"], "frontend-smoke-loop");
    assert_eq!(body["graph"]["scope"], "system");
}

#[tokio::test]
async fn tg_read_graph_not_found() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    let response = app
        .oneshot(
            Request::get("/api/projects/demo/task-graphs/project/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = json_response(response).await;
    assert_eq!(body["error"]["code"], "graph_not_found");
}

#[tokio::test]
async fn tg_create_project_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    let body = json!({ "graph": minimal_project_graph_json() });
    let response = app
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_response(response).await;
    assert_eq!(result["graph"]["id"], "my-test-graph");
    assert_eq!(result["graph"]["version"], 1);
    assert_eq!(result["validation"]["status"], "passed");
}

#[tokio::test]
async fn tg_create_duplicate_returns_conflict() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace.clone());

    let body = json!({ "graph": minimal_project_graph_json() });
    let req_body = serde_json::to_vec(&body).unwrap();

    // First create
    app.clone()
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(req_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Second create — duplicate
    let response = app
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let result = json_response(response).await;
    assert_eq!(result["error"]["code"], "duplicate_graph_id");
}

#[tokio::test]
async fn tg_patch_project_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace.clone());

    // Create first
    let create_body = json!({ "graph": minimal_project_graph_json() });
    app.clone()
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Patch
    let mut graph = minimal_project_graph_json();
    graph["title"] = json!("Updated Title");
    let patch_body = json!({ "graph": graph, "expected_version": 1 });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/task-graphs/project/my-test-graph")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_response(response).await;
    assert_eq!(result["graph"]["version"], 2);
    assert_eq!(result["graph"]["title"], "Updated Title");
}

#[tokio::test]
async fn tg_patch_stale_version() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace.clone());

    // Create
    let create_body = json!({ "graph": minimal_project_graph_json() });
    app.clone()
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Patch with wrong version
    let patch_body = json!({ "graph": minimal_project_graph_json(), "expected_version": 99 });
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/projects/demo/task-graphs/project/my-test-graph")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let result = json_response(response).await;
    assert_eq!(result["error"]["code"], "stale_version");
}

#[tokio::test]
async fn tg_delete_project_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace.clone());

    // Create
    let create_body = json!({ "graph": minimal_project_graph_json() });
    app.clone()
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/projects/demo/task-graphs/project/my-test-graph")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/task-graphs/project/my-test-graph")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tg_fork_system_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace.clone());

    let fork_body = json!({
        "target_id": "my-fork",
        "title": "Forked Graph"
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/projects/demo/task-graphs/system/frontend-smoke-loop/fork")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&fork_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_response(response).await;
    assert_eq!(result["graph"]["id"], "my-fork");
    assert_eq!(result["graph"]["scope"], "project");
    assert_eq!(result["graph"]["title"], "Forked Graph");
    assert_eq!(result["graph"]["origin"]["scope"], "system");
    assert_eq!(result["graph"]["origin"]["id"], "frontend-smoke-loop");

    // Should now appear in catalog
    let response = app
        .oneshot(
            Request::get("/api/projects/demo/task-graphs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_response(response).await;
    let graphs = body["graphs"].as_array().unwrap();
    assert!(graphs
        .iter()
        .any(|g| g["id"] == "my-fork" && g["scope"] == "project"));
}

#[tokio::test]
async fn tg_create_invalid_graph_returns_validation_error() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    // Graph with no start node
    let invalid_graph = json!({
        "schema_version": 1,
        "id": "invalid-graph",
        "scope": "project",
        "title": "Invalid",
        "version": 1,
        "readonly": false,
        "nodes": [
            { "id": "end-success", "type": "end", "label": "End", "config": { "result": "succeeded" } }
        ],
        "edges": []
    });
    let body = json!({ "graph": invalid_graph });

    let response = app
        .oneshot(
            Request::post("/api/projects/demo/task-graphs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let result = json_response(response).await;
    assert_eq!(result["error"]["code"], "validation_failed");
    assert!(!result["error"]["details"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn tg_fork_nonexistent_system_graph() {
    let (_temp, workspace) = tg_fixture();
    let app = app(workspace);

    let fork_body = json!({ "target_id": "my-fork" });
    let response = app
        .oneshot(
            Request::post("/api/projects/demo/task-graphs/system/nonexistent/fork")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&fork_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let result = json_response(response).await;
    assert_eq!(result["error"]["code"], "graph_not_found");
}
