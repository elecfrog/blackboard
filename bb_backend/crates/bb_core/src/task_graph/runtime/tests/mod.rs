use super::*;

#[test]
fn opencode_json_capture_extracts_text_and_tool_log() {
    let stdout = r#"{"type":"step_start","sessionID":"sess-1","part":{}}
{"type":"tool_use","part":{"tool":"bb_read_inbox_note","callID":"call-1","state":{"status":"completed","input":{"name":"note.md"},"output":"read ok"}}}
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
