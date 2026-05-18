use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

use chrono::Utc;
use serde_json::Value;

use super::super::model::{AgentEvent, AgentEventType, TokenUsage};
use super::super::store;
use super::super::AgentSessionError;

const TOOL_OUTPUT_LIMIT: usize = 8192;

#[derive(Debug, Clone)]
pub struct PiObserver {
    pub workspace_root: PathBuf,
    pub project: String,
    pub session_id: String,
    pub model_key: String,
}

#[derive(Debug, Clone)]
pub struct PiCapture {
    pub stdout: Vec<u8>,
    pub text_output: String,
    current_turn_text: String,
    pub log: String,
    pub error_message: Option<String>,
    pub provider_session_id: Option<String>,
    pub usage: BTreeMap<String, TokenUsage>,
    pub event_count: u64,
    pub tool_count: u64,
    pub next_seq: u64,
}

impl Default for PiCapture {
    fn default() -> Self {
        Self {
            stdout: Vec::new(),
            text_output: String::new(),
            current_turn_text: String::new(),
            log: String::new(),
            error_message: None,
            provider_session_id: None,
            usage: BTreeMap::new(),
            event_count: 0,
            tool_count: 0,
            next_seq: 1,
        }
    }
}

pub fn read_pi_json_pipe_to_end(pipe: impl Read, observer: PiObserver) -> PiCapture {
    let mut reader = BufReader::new(pipe);
    let mut capture = PiCapture::default();
    let mut call_id_to_tool = HashMap::<String, String>::new();
    let mut log_lines = Vec::<String>::new();
    let mut line = String::new();

    loop {
        line.clear();
        let Ok(read) = reader.read_line(&mut line) else {
            break;
        };
        if read == 0 {
            break;
        }

        capture.stdout.extend_from_slice(line.as_bytes());
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(event) = serde_json::from_str::<Value>(line) {
            match handle_pi_event(&observer, &event, &mut capture, &mut call_id_to_tool) {
                Ok(lines) => log_lines.extend(lines),
                Err(err) => log_lines.push(format!("AgentSession persist error: {err}")),
            }
        } else {
            let stripped = strip_ansi_codes(line);
            log_lines.push(stripped.clone());
            let _ = append_event(
                &observer,
                &mut capture,
                AgentEventType::Log,
                Some(stripped),
                None,
                None,
                None,
                None,
                None,
                Some("runtime".to_string()),
                None,
                BTreeMap::new(),
            );
        }
    }

    capture.log = log_lines.join("\n");
    capture
}

fn handle_pi_event(
    observer: &PiObserver,
    event: &Value,
    capture: &mut PiCapture,
    call_id_to_tool: &mut HashMap<String, String>,
) -> Result<Vec<String>, AgentSessionError> {
    match string_field(event, "type").as_deref() {
        Some("session") => {
            let session_id = string_field(event, "id");
            if let Some(provider_session_id) = session_id.as_deref() {
                capture.provider_session_id = Some(provider_session_id.to_string());
                store::update_provider_session_id(
                    &observer.workspace_root,
                    &observer.project,
                    &observer.session_id,
                    provider_session_id,
                )?;
            }
            append_event(
                observer,
                capture,
                AgentEventType::Status,
                None,
                None,
                None,
                None,
                None,
                Some("started".to_string()),
                None,
                session_id.clone(),
                BTreeMap::new(),
            )?;
            Ok(vec![session_id.map_or_else(
                || "Pi session started".to_string(),
                |id| format!("Pi session started session={id}"),
            )])
        }
        Some("agent_start") => {
            append_status(observer, capture, "agent_started")?;
            Ok(vec!["Pi agent started".to_string()])
        }
        Some("turn_start") => {
            capture.current_turn_text.clear();
            append_status(observer, capture, "turn_started")?;
            Ok(vec!["Pi turn started".to_string()])
        }
        Some("message_update") => handle_message_update(observer, event, capture),
        Some("message_end") => {
            if capture.text_output.trim().is_empty() {
                if let Some(text) = event.get("message").and_then(assistant_message_text) {
                    append_text(observer, capture, &text)?;
                }
            }
            Ok(Vec::new())
        }
        Some("turn_end") => {
            if let Some(message) = event.get("message") {
                append_message_usage(observer, message, capture)?;
                if capture.current_turn_text.trim().is_empty() {
                    if let Some(text) = assistant_message_text(message) {
                        append_text(observer, capture, &text)?;
                    }
                }
            }
            promote_current_turn_text(capture);
            append_status(observer, capture, "turn_completed")?;
            Ok(vec!["Pi turn completed".to_string()])
        }
        Some("agent_end") => {
            if capture.current_turn_text.trim().is_empty() {
                if let Some(text) = event.get("messages").and_then(assistant_messages_text) {
                    append_text(observer, capture, &text)?;
                }
            }
            promote_current_turn_text(capture);
            append_status(observer, capture, "completed")?;
            Ok(vec!["Pi agent completed".to_string()])
        }
        Some("tool_execution_start") => {
            let tool = string_field(event, "toolName").unwrap_or_else(|| "unknown".to_string());
            let call_id = string_field(event, "toolCallId");
            if let Some(call_id) = call_id.as_deref().filter(|id| !id.is_empty()) {
                call_id_to_tool.insert(call_id.to_string(), tool.clone());
            }
            capture.tool_count += 1;
            append_event(
                observer,
                capture,
                AgentEventType::ToolUse,
                None,
                Some(tool.clone()),
                call_id.clone(),
                event.get("args").cloned(),
                None,
                Some("started".to_string()),
                None,
                None,
                BTreeMap::new(),
            )?;
            Ok(vec![format!(
                "Tool {tool} call_id={} status=started",
                call_id.as_deref().unwrap_or("-")
            )])
        }
        Some("tool_execution_update") => {
            let tool = tool_for_event(event, call_id_to_tool);
            let call_id = string_field(event, "toolCallId");
            let output = event
                .get("partialResult")
                .map(tool_result_output)
                .filter(|output| !output.trim().is_empty())
                .unwrap_or_default();
            if !output.is_empty() {
                append_event(
                    observer,
                    capture,
                    AgentEventType::ToolResult,
                    None,
                    Some(tool.clone()),
                    call_id.clone(),
                    None,
                    Some(truncate_chars(&output, TOOL_OUTPUT_LIMIT)),
                    Some("running".to_string()),
                    None,
                    None,
                    BTreeMap::new(),
                )?;
            }
            Ok(vec![format!(
                "Tool {tool} call_id={} status=running",
                call_id.as_deref().unwrap_or("-")
            )])
        }
        Some("tool_execution_end") => {
            let tool = tool_for_event(event, call_id_to_tool);
            let call_id = string_field(event, "toolCallId");
            let is_error = bool_field(event, "isError").unwrap_or(false);
            let status = if is_error { "error" } else { "completed" }.to_string();
            let output = event
                .get("result")
                .map(tool_result_output)
                .unwrap_or_default();
            append_event(
                observer,
                capture,
                AgentEventType::ToolResult,
                None,
                Some(tool.clone()),
                call_id.clone(),
                None,
                Some(truncate_chars(&output, TOOL_OUTPUT_LIMIT)),
                Some(status.clone()),
                None,
                None,
                BTreeMap::new(),
            )?;
            Ok(vec![format!(
                "Tool result {tool} call_id={} status={status}",
                call_id.as_deref().unwrap_or("-")
            )])
        }
        Some("compaction_start") => {
            let reason = string_field(event, "reason").unwrap_or_else(|| "unknown".to_string());
            append_status(observer, capture, &format!("compaction_started:{reason}"))?;
            Ok(vec![format!("Pi compaction started reason={reason}")])
        }
        Some("compaction_end") => {
            let reason = string_field(event, "reason").unwrap_or_else(|| "unknown".to_string());
            if let Some(message) = string_field(event, "errorMessage") {
                capture.error_message = Some(message.clone());
                append_error(observer, capture, message.clone())?;
                return Ok(vec![format!(
                    "Pi compaction failed reason={reason}: {message}"
                )]);
            }
            append_status(observer, capture, &format!("compaction_completed:{reason}"))?;
            Ok(vec![format!("Pi compaction completed reason={reason}")])
        }
        Some("auto_retry_start") => {
            let attempt = i64_field(event, "attempt");
            let max_attempts = i64_field(event, "maxAttempts");
            let message = string_field(event, "errorMessage").unwrap_or_default();
            append_event(
                observer,
                capture,
                AgentEventType::Log,
                Some(format!(
                    "Pi auto retry started attempt={attempt}/{max_attempts}: {message}"
                )),
                None,
                None,
                None,
                None,
                None,
                Some("runtime".to_string()),
                None,
                BTreeMap::new(),
            )?;
            Ok(vec![format!(
                "Pi auto retry started attempt={attempt}/{max_attempts}: {message}"
            )])
        }
        Some("auto_retry_end") => {
            if bool_field(event, "success") == Some(false) {
                let message = string_field(event, "finalError")
                    .unwrap_or_else(|| "Pi auto retry failed".to_string());
                capture.error_message = Some(message.clone());
                append_error(observer, capture, message.clone())?;
                Ok(vec![message])
            } else {
                Ok(vec!["Pi auto retry completed".to_string()])
            }
        }
        Some("extension_error") => {
            let message = string_field(event, "error").unwrap_or_else(|| display_json_value(event));
            capture.error_message = Some(message.clone());
            append_error(observer, capture, message.clone())?;
            Ok(vec![format!("Pi extension error: {message}")])
        }
        Some("queue_update") => Ok(vec!["Pi queue updated".to_string()]),
        Some(other) => Ok(vec![format!("Pi event: {other}")]),
        None => Ok(Vec::new()),
    }
}

fn handle_message_update(
    observer: &PiObserver,
    event: &Value,
    capture: &mut PiCapture,
) -> Result<Vec<String>, AgentSessionError> {
    let delta = event.get("assistantMessageEvent").unwrap_or(event);
    match string_field(delta, "type").as_deref() {
        Some("text_delta") => {
            let text = string_field(delta, "delta").unwrap_or_default();
            if text.is_empty() {
                Ok(Vec::new())
            } else {
                append_text(observer, capture, &text)?;
                Ok(vec![format!("AI output:\n{}", text.trim_end())])
            }
        }
        Some("thinking_delta") => {
            let text = string_field(delta, "delta").unwrap_or_default();
            if text.trim().is_empty() {
                Ok(Vec::new())
            } else {
                append_event(
                    observer,
                    capture,
                    AgentEventType::Thinking,
                    Some(text.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    BTreeMap::new(),
                )?;
                Ok(vec![format!("Reasoning:\n{}", text.trim_end())])
            }
        }
        Some("error") => {
            let message = string_field(delta, "message")
                .or_else(|| string_field(delta, "error"))
                .unwrap_or_else(|| "unknown pi message error".to_string());
            capture.error_message = Some(message.clone());
            append_error(observer, capture, message.clone())?;
            Ok(vec![format!("Pi message error: {message}")])
        }
        _ => Ok(Vec::new()),
    }
}

fn append_message_usage(
    observer: &PiObserver,
    message: &Value,
    capture: &mut PiCapture,
) -> Result<(), AgentSessionError> {
    let Some(usage) = message.get("usage").and_then(token_usage_from_value) else {
        return Ok(());
    };
    let model_key = string_field(message, "model")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| observer.model_key.clone());
    capture
        .usage
        .entry(model_key.clone())
        .or_default()
        .add_assign(&usage);
    let mut delta = BTreeMap::new();
    delta.insert(model_key, usage);
    append_event(
        observer,
        capture,
        AgentEventType::UsageUpdate,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        delta,
    )
}

fn append_status(
    observer: &PiObserver,
    capture: &mut PiCapture,
    status: &str,
) -> Result<(), AgentSessionError> {
    append_event(
        observer,
        capture,
        AgentEventType::Status,
        None,
        None,
        None,
        None,
        None,
        Some(status.to_string()),
        None,
        None,
        BTreeMap::new(),
    )
}

fn append_text(
    observer: &PiObserver,
    capture: &mut PiCapture,
    text: &str,
) -> Result<(), AgentSessionError> {
    if text.is_empty() {
        return Ok(());
    }
    capture.current_turn_text.push_str(text);
    capture.text_output.push_str(text);
    append_event(
        observer,
        capture,
        AgentEventType::Text,
        Some(text.to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        BTreeMap::new(),
    )
}

fn promote_current_turn_text(capture: &mut PiCapture) {
    if capture.current_turn_text.trim().is_empty() {
        return;
    }
    capture.text_output = capture.current_turn_text.clone();
    capture.current_turn_text.clear();
}

fn append_error(
    observer: &PiObserver,
    capture: &mut PiCapture,
    message: String,
) -> Result<(), AgentSessionError> {
    append_event(
        observer,
        capture,
        AgentEventType::Error,
        Some(message),
        None,
        None,
        None,
        None,
        None,
        Some("error".to_string()),
        None,
        BTreeMap::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn append_event(
    observer: &PiObserver,
    capture: &mut PiCapture,
    event_type: AgentEventType,
    content: Option<String>,
    tool: Option<String>,
    call_id: Option<String>,
    input: Option<Value>,
    output: Option<String>,
    status: Option<String>,
    level: Option<String>,
    session_id: Option<String>,
    usage: BTreeMap<String, TokenUsage>,
) -> Result<(), AgentSessionError> {
    let event = AgentEvent {
        seq: capture.next_seq,
        timestamp: Utc::now().to_rfc3339(),
        event_type,
        content,
        tool,
        call_id,
        input,
        output,
        status,
        level,
        session_id,
        usage,
    };
    store::append_event(
        &observer.workspace_root,
        &observer.project,
        &observer.session_id,
        &event,
    )?;
    capture.next_seq += 1;
    capture.event_count += 1;
    Ok(())
}

fn tool_for_event(event: &Value, call_id_to_tool: &HashMap<String, String>) -> String {
    let call_id = string_field(event, "toolCallId");
    call_id
        .as_ref()
        .and_then(|id| call_id_to_tool.get(id))
        .cloned()
        .or_else(|| string_field(event, "toolName"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn token_usage_from_value(value: &Value) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: i64_field(value, "input") + i64_field(value, "input_tokens"),
        output_tokens: i64_field(value, "output") + i64_field(value, "output_tokens"),
        cache_read_tokens: i64_field(value, "cacheRead")
            + i64_field(value, "cache_read")
            + i64_field(value, "cache_read_tokens")
            + i64_field(value, "cached_input_tokens"),
        cache_write_tokens: i64_field(value, "cacheWrite")
            + i64_field(value, "cache_write")
            + i64_field(value, "cache_write_tokens"),
    };
    (!usage.is_empty()).then_some(usage)
}

fn assistant_messages_text(value: &Value) -> Option<String> {
    let items = value.as_array()?;
    let parts: Vec<String> = items.iter().filter_map(assistant_message_text).collect();
    (!parts.is_empty()).then(|| parts.join(""))
}

fn assistant_message_text(value: &Value) -> Option<String> {
    if string_field(value, "role").as_deref() != Some("assistant") {
        return None;
    }
    value.get("content").and_then(extract_text_content)
}

fn extract_text_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| match string_field(item, "type").as_deref() {
                    Some("text") => string_field(item, "text"),
                    _ => None,
                })
                .collect();
            (!parts.is_empty()).then(|| parts.join(""))
        }
        Value::Object(object) => object.get("text").and_then(extract_text_content),
        _ => None,
    }
}

fn tool_result_output(value: &Value) -> String {
    if let Some(content) = value.get("content") {
        if let Some(text) = extract_mixed_text(content) {
            return text;
        }
    }
    if let Some(details) = value.get("details") {
        let text = display_json_value(details);
        if !text.trim().is_empty() && text != "null" {
            return text;
        }
    }
    display_json_value(value)
}

fn extract_mixed_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(ToString::to_string)
                        .or_else(|| string_field(item, "text"))
                        .or_else(|| string_field(item, "content"))
                })
                .collect();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        Value::Object(_) => string_field(value, "text").or_else(|| string_field(value, "content")),
        _ => None,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn i64_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn display_json_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value.chars().take(max).collect()
    }
}

fn strip_ansi_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_session::{create_session, read_events, CreateAgentSession};

    #[test]
    fn pi_events_persist_status_text_tools_usage_and_error() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "pi".to_string(),
                agent: "pi".to_string(),
                model: Some("openai/gpt-5.4".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = PiObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "openai/gpt-5.4".to_string(),
        };
        let stdout = r#"{"type":"session","version":3,"id":"pi-sess-1","timestamp":"2026-05-21T00:00:00Z","cwd":"/repo"}
{"type":"agent_start"}
{"type":"turn_start"}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"hello"}}
{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","delta":"think"}}
{"type":"tool_execution_start","toolCallId":"call-1","toolName":"bash","args":{"command":"pwd"}}
{"type":"tool_execution_end","toolCallId":"call-1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}]},"isError":false}
{"type":"turn_end","message":{"role":"assistant","model":"fake-model","content":[{"type":"text","text":"hello"}],"usage":{"input":10,"output":5,"cacheRead":3,"cacheWrite":2}},"toolResults":[]}
{"type":"extension_error","error":"boom"}
"#;

        let capture = read_pi_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.provider_session_id.as_deref(), Some("pi-sess-1"));
        assert_eq!(capture.text_output, "hello");
        assert_eq!(capture.error_message.as_deref(), Some("boom"));
        assert_eq!(capture.tool_count, 1);
        assert_eq!(capture.usage["fake-model"].input_tokens, 10);
        assert_eq!(capture.usage["fake-model"].cache_read_tokens, 3);
        assert_eq!(capture.usage["fake-model"].cache_write_tokens, 2);

        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Status));
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Text && event.content.as_deref() == Some("hello")
        }));
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Thinking
                && event.content.as_deref() == Some("think")
        }));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::ToolUse));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::ToolResult));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::UsageUpdate));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Error));
    }

    #[test]
    fn pi_tool_errors_do_not_mark_successful_turn_failed() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "pi".to_string(),
                agent: "pi".to_string(),
                model: Some("minimax-cn/MiniMax-M2.7-highspeed".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = PiObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "minimax-cn/MiniMax-M2.7-highspeed".to_string(),
        };
        let stdout = r#"{"type":"session","id":"pi-sess-2"}
{"type":"turn_start"}
{"type":"tool_execution_start","toolCallId":"call-1","toolName":"read","args":{"path":"missing.md"}}
{"type":"tool_execution_end","toolCallId":"call-1","toolName":"read","result":{"content":[{"type":"text","text":"ENOENT: no such file or directory"}]},"isError":true}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"{\"findings\":[],\"risks\":[],\"coverage_notes\":[\"recovered\"]}"}}
{"type":"turn_end","message":{"role":"assistant","model":"minimax-cn/MiniMax-M2.7-highspeed","content":[{"type":"text","text":"{\"findings\":[],\"risks\":[],\"coverage_notes\":[\"recovered\"]}"}]}}
"#;

        let capture = read_pi_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.error_message, None);
        assert!(capture
            .text_output
            .contains("\"coverage_notes\":[\"recovered\"]"));

        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::ToolResult
                && event.status.as_deref() == Some("error")
        }));
        assert!(!events
            .iter()
            .any(|event| event.event_type == AgentEventType::Error));
    }

    #[test]
    fn pi_text_output_uses_last_completed_turn() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "pi".to_string(),
                agent: "pi".to_string(),
                model: Some("minimax-cn/MiniMax-M2.7-highspeed".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = PiObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "minimax-cn/MiniMax-M2.7-highspeed".to_string(),
        };
        let stdout = r##"{"type":"session","id":"pi-sess-3"}
{"type":"turn_start"}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Now let me inspect the files.\n"}}
{"type":"turn_end","message":{"role":"assistant","model":"minimax-cn/MiniMax-M2.7-highspeed","content":[{"type":"text","text":"Now let me inspect the files.\n"}]}}
{"type":"turn_start"}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"# Final Report\n\nClean output."}}
{"type":"turn_end","message":{"role":"assistant","model":"minimax-cn/MiniMax-M2.7-highspeed","content":[{"type":"text","text":"# Final Report\n\nClean output."}]}}
"##;

        let capture = read_pi_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.text_output, "# Final Report\n\nClean output.");

        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Text
                && event.content.as_deref() == Some("Now let me inspect the files.\n")
        }));
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Text
                && event.content.as_deref() == Some("# Final Report\n\nClean output.")
        }));
    }
}
