use super::*;

#[test]
fn opencode_json_capture_extracts_text_and_tool_log() {
    let stdout = r#"{"type":"step_start","sessionID":"sess-1","part":{}}
{"type":"tool_use","part":{"tool":"bb_read_inbox_note","callID":"call-1","state":{"status":"completed","input":{"name":"note.json"},"output":"read ok"}}}
{"type":"text","part":{"text":"{\"processed\":true,\"continue\":false}"}}
{"type":"step_finish","part":{"tokens":{"input":10,"output":5,"cache":{"read":1,"write":2}}}}
"#;

    let capture = capture_runtime_output("opencode", stdout, "");

    assert_eq!(
        capture.parse_source,
        r#"{"processed":true,"continue":false}"#
    );
    assert_eq!(capture.artifact, r#"{"processed":true,"continue":false}"#);
    assert!(capture.log.contains("Tool bb_read_inbox_note"));
    assert!(capture
        .log
        .contains("Step finished tokens input=10 output=5"));
    assert!(capture.error_message.is_none());
}

#[test]
fn opencode_json_capture_treats_error_event_as_failure() {
    let stdout = r#"{"type":"error","error":{"name":"RateLimitError","data":{"message":"boom"}}}"#;

    let capture = capture_runtime_output("opencode", stdout, "");

    assert_eq!(capture.error_message.as_deref(), Some("boom"));
    assert!(capture.log.contains("OpenCode error: boom"));
}

#[test]
fn json_text_parser_prefers_task_graph_over_later_nested_objects() {
    let text = r#"
Here is the generated graph:
{
  "schema_version": 1,
  "id": "research-subgraph",
  "scope": "project",
  "title": "Research",
  "version": 1,
  "readonly": false,
  "nodes": [],
  "edges": []
}
And an edge example:
{"id":"e1","from":"start","to":"end","kind":"exec"}
"#;

    let parsed = try_parse_json_or_text(text);

    assert_eq!(parsed["id"], "research-subgraph");
    assert!(parsed.get("schema_version").is_some());
}

#[test]
fn json_text_parser_accepts_wrapped_task_graph() {
    let text = r#"
{"subgraph":{"schema_version":1,"id":"wrapped","scope":"project","title":"Wrapped","version":1,"readonly":false,"nodes":[],"edges":[]}}
{"id":"e1","from":"start","to":"end","kind":"exec"}
"#;

    let parsed = try_parse_json_or_text(text);

    assert_eq!(parsed["subgraph"]["id"], "wrapped");
}

#[test]
fn json_artifact_rejects_invalid_document_instead_of_salvaging_inner_object() {
    let config: LlmConfig = serde_json::from_value(serde_json::json!({
        "runtime": "pi",
        "agent": "native",
        "prompt": {},
        "output": {
            "artifact_type": "json",
            "required": true
        }
    }))
    .unwrap();
    let text = r#"{"reviewer_id":"R2","findings":[{"id":"r2-001","suggested_change":"write "quoted" text"}],"risk_updates":[]}"#;

    let output = output_value_for_llm_config(&config, text, text);

    assert!(output.is_string());
}

#[test]
fn json_artifact_still_accepts_embedded_json_after_intro_text() {
    let config: LlmConfig = serde_json::from_value(serde_json::json!({
        "runtime": "pi",
        "agent": "native",
        "prompt": {},
        "output": {
            "artifact_type": "json",
            "required": true
        }
    }))
    .unwrap();
    let text = "Here is the JSON:\n{\"processed\":true,\"continue\":false}";

    let output = output_value_for_llm_config(&config, text, text);

    assert_eq!(
        output,
        serde_json::json!({"processed": true, "continue": false})
    );
}

#[test]
fn markdown_llm_output_uses_artifact_text_for_data_output() {
    let config: LlmConfig = serde_json::from_value(serde_json::json!({
        "runtime": "pi",
        "agent": "native",
        "prompt": {},
        "output": {
            "artifact_type": "markdown",
            "required": true
        }
    }))
    .unwrap();

    let output = output_value_for_llm_config(&config, "{}", "# Review\n\nFinding text");

    assert_eq!(output, serde_json::json!("# Review\n\nFinding text"));
}
