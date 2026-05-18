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
pub struct CodeBuddyObserver {
    pub workspace_root: PathBuf,
    pub project: String,
    pub session_id: String,
    pub model_key: String,
}

#[derive(Debug, Clone)]
pub struct CodeBuddyCapture {
    pub stdout: Vec<u8>,
    pub text_output: String,
    pub result_output: Option<String>,
    pub log: String,
    pub error_message: Option<String>,
    pub provider_session_id: Option<String>,
    pub usage: BTreeMap<String, TokenUsage>,
    pub event_count: u64,
    pub tool_count: u64,
    pub next_seq: u64,
}

impl Default for CodeBuddyCapture {
    fn default() -> Self {
        Self {
            stdout: Vec::new(),
            text_output: String::new(),
            result_output: None,
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

pub fn read_codebuddy_json_pipe_to_end(
    pipe: impl Read,
    observer: &CodeBuddyObserver,
) -> CodeBuddyCapture {
    let mut reader = BufReader::new(pipe);
    let mut capture = CodeBuddyCapture::default();
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
            if let Err(err) =
                handle_codebuddy_event(observer, &event, &mut capture, &mut call_id_to_tool)
            {
                log_lines.push(format!("AgentSession persist error: {err}"));
            }
        } else {
            let stripped = strip_ansi_codes(line);
            log_lines.push(stripped.clone());
            let _ = append_event(
                observer,
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

    if !log_lines.is_empty() {
        capture.log = log_lines.join("\n");
    }
    capture
}

fn handle_codebuddy_event(
    observer: &CodeBuddyObserver,
    event: &Value,
    capture: &mut CodeBuddyCapture,
    call_id_to_tool: &mut HashMap<String, String>,
) -> Result<(), AgentSessionError> {
    match string_field(event, "type").as_deref() {
        Some("system") => handle_system_event(observer, event, capture),
        Some("assistant" | "user") => {
            handle_message_event(observer, event, capture, call_id_to_tool)
        }
        Some("result") => handle_result_event(observer, event, capture),
        Some("error") => {
            let message = event_error_message(event).unwrap_or_else(|| display_json_value(event));
            capture.error_message = Some(message.clone());
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
        _ => {
            if let Some(message) = event_log_message(event) {
                append_event(
                    observer,
                    capture,
                    AgentEventType::Log,
                    Some(message),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("runtime".to_string()),
                    None,
                    BTreeMap::new(),
                )?;
            }
            Ok(())
        }
    }
}

fn handle_system_event(
    observer: &CodeBuddyObserver,
    event: &Value,
    capture: &mut CodeBuddyCapture,
) -> Result<(), AgentSessionError> {
    if string_field(event, "subtype").as_deref() != Some("init") {
        return Ok(());
    }
    if let Some(provider_session_id) = string_field(event, "session_id") {
        capture.provider_session_id = Some(provider_session_id.clone());
        store::update_provider_session_id(
            &observer.workspace_root,
            &observer.project,
            &observer.session_id,
            &provider_session_id,
        )?;
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
            Some(provider_session_id),
            BTreeMap::new(),
        )?;
    }
    Ok(())
}

fn handle_message_event(
    observer: &CodeBuddyObserver,
    event: &Value,
    capture: &mut CodeBuddyCapture,
    call_id_to_tool: &mut HashMap<String, String>,
) -> Result<(), AgentSessionError> {
    let message = event.get("message").unwrap_or(event);
    let role = string_field(message, "role").or_else(|| string_field(event, "type"));

    if role.as_deref() == Some("assistant") {
        if let Some(usage) = message.get("usage").and_then(token_usage_from_value) {
            let model_key = string_field(message, "model")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| observer.model_key.clone());
            let mut usage_map = BTreeMap::new();
            usage_map.insert(model_key.clone(), usage.clone());
            capture
                .usage
                .entry(model_key)
                .or_default()
                .add_assign(&usage);
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
                usage_map,
            )?;
        }
    }

    let Some(content) = message.get("content") else {
        if role.as_deref() == Some("assistant") {
            if let Some(text) = string_field(message, "text").filter(|text| !text.is_empty()) {
                append_text_event(observer, capture, text)?;
            }
        }
        return Ok(());
    };

    if let Some(text) = content.as_str() {
        if role.as_deref() == Some("assistant") {
            append_text_event(observer, capture, text.to_string())?;
        }
        return Ok(());
    }

    let Some(items) = content.as_array() else {
        return Ok(());
    };
    for item in items {
        match string_field(item, "type").as_deref() {
            Some("text") => {
                if role.as_deref() == Some("assistant") {
                    if let Some(text) = string_field(item, "text").filter(|text| !text.is_empty()) {
                        append_text_event(observer, capture, text)?;
                    }
                }
            }
            Some("thinking" | "reasoning" | "redacted_thinking") => {
                let text = string_field(item, "thinking")
                    .or_else(|| string_field(item, "text"))
                    .or_else(|| string_field(item, "content"))
                    .filter(|text| !text.is_empty());
                if let Some(text) = text {
                    append_event(
                        observer,
                        capture,
                        AgentEventType::Thinking,
                        Some(text),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        BTreeMap::new(),
                    )?;
                }
            }
            Some("tool_use" | "server_tool_use") => {
                let raw_tool = string_field(item, "name")
                    .or_else(|| string_field(item, "tool"))
                    .unwrap_or_else(|| "unknown".to_string());
                let raw_input = item.get("input").or_else(|| item.get("arguments"));
                let tool = raw_input
                    .filter(|_| raw_tool == "DeferExecuteTool")
                    .and_then(|input| string_field(input, "toolName"))
                    .unwrap_or_else(|| raw_tool.clone());
                let call_id = string_field(item, "id").or_else(|| string_field(item, "call_id"));
                if let Some(call_id) = call_id.as_deref().filter(|id| !id.is_empty()) {
                    call_id_to_tool.insert(call_id.to_string(), tool.clone());
                }
                capture.tool_count += 1;
                let input = if raw_tool == "DeferExecuteTool" {
                    raw_input
                        .and_then(|input| input.get("params"))
                        .map(normalize_json_payload)
                        .or_else(|| raw_input.map(normalize_json_payload))
                } else {
                    raw_input.map(normalize_json_payload)
                };
                append_event(
                    observer,
                    capture,
                    AgentEventType::ToolUse,
                    None,
                    Some(tool),
                    call_id,
                    input,
                    None,
                    Some("started".to_string()),
                    None,
                    None,
                    BTreeMap::new(),
                )?;
            }
            Some("tool_result") => {
                let call_id =
                    string_field(item, "tool_use_id").or_else(|| string_field(item, "call_id"));
                let tool = call_id
                    .as_ref()
                    .and_then(|id| call_id_to_tool.get(id))
                    .cloned()
                    .or_else(|| string_field(item, "name"))
                    .or_else(|| string_field(item, "tool"))
                    .unwrap_or_else(|| "unknown".to_string());
                let is_error = item
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                append_event(
                    observer,
                    capture,
                    AgentEventType::ToolResult,
                    None,
                    Some(tool),
                    call_id,
                    None,
                    Some(truncate_chars(&tool_result_output(item), TOOL_OUTPUT_LIMIT)),
                    Some(if is_error { "error" } else { "completed" }.to_string()),
                    None,
                    None,
                    BTreeMap::new(),
                )?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn handle_result_event(
    observer: &CodeBuddyObserver,
    event: &Value,
    capture: &mut CodeBuddyCapture,
) -> Result<(), AgentSessionError> {
    if let Some(provider_session_id) = string_field(event, "session_id") {
        capture.provider_session_id = Some(provider_session_id.clone());
        store::update_provider_session_id(
            &observer.workspace_root,
            &observer.project,
            &observer.session_id,
            &provider_session_id,
        )?;
    }

    if let Some(result) = string_field(event, "result") {
        capture.result_output = Some(result);
    }

    let is_error = event
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || string_field(event, "subtype")
            .as_deref()
            .is_some_and(|subtype| subtype == "error" || subtype == "failed");
    if is_error {
        let message = event_error_message(event)
            .or_else(|| string_field(event, "result"))
            .unwrap_or_else(|| "CodeBuddy result reported an error".to_string());
        capture.error_message = Some(message.clone());
        append_event(
            observer,
            capture,
            AgentEventType::Error,
            Some(message),
            None,
            None,
            None,
            None,
            Some("failed".to_string()),
            Some("error".to_string()),
            capture.provider_session_id.clone(),
            BTreeMap::new(),
        )?;
    }
    Ok(())
}

fn append_text_event(
    observer: &CodeBuddyObserver,
    capture: &mut CodeBuddyCapture,
    text: String,
) -> Result<(), AgentSessionError> {
    capture.text_output.push_str(&text);
    append_event(
        observer,
        capture,
        AgentEventType::Text,
        Some(text),
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

#[allow(clippy::too_many_arguments)]
pub fn append_event(
    observer: &CodeBuddyObserver,
    capture: &mut CodeBuddyCapture,
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

fn token_usage_from_value(usage: &Value) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: i64_field(usage, "input_tokens"),
        output_tokens: i64_field(usage, "output_tokens"),
        cache_read_tokens: i64_field(usage, "cache_read_input_tokens")
            + i64_field(usage, "cache_read_tokens")
            + i64_field(usage, "cached_input_tokens"),
        cache_write_tokens: i64_field(usage, "cache_creation_input_tokens")
            + i64_field(usage, "cache_write_tokens"),
    };
    (!usage.is_empty()).then_some(usage)
}

fn i64_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToString::to_string)
}

fn event_error_message(value: &Value) -> Option<String> {
    string_field(value, "message")
        .or_else(|| string_field(value, "error"))
        .or_else(|| value.get("error").map(display_json_value))
        .filter(|message| !message.trim().is_empty())
}

fn event_log_message(value: &Value) -> Option<String> {
    string_field(value, "message")
        .or_else(|| string_field(value, "text"))
        .or_else(|| string_field(value, "result"))
        .filter(|message| !message.trim().is_empty())
}

fn normalize_json_payload(value: &Value) -> Value {
    value.as_str().map_or_else(
        || value.clone(),
        |text| serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string())),
    )
}

fn tool_result_output(value: &Value) -> String {
    let Some(content) = value.get("content") else {
        return display_json_value(value);
    };
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    if let Some(items) = content.as_array() {
        let lines: Vec<String> = items
            .iter()
            .filter_map(|item| {
                string_field(item, "text")
                    .or_else(|| string_field(item, "content"))
                    .or_else(|| item.as_str().map(ToString::to_string))
            })
            .collect();
        if !lines.is_empty() {
            return lines.join("\n");
        }
    }
    display_json_value(content)
}

fn display_json_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}")),
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max {
        value.to_string()
    } else {
        value.chars().take(max).collect()
    }
}

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_session::{create_session, read_events, CreateAgentSession};
    use crate::agent_session::{AgentEventType, AgentSessionParent};

    #[test]
    fn codebuddy_events_persist_status_text_tools_usage_and_error() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "codebuddy".to_string(),
                agent: "native".to_string(),
                model: Some("gpt-5".to_string()),
                variant: None,
                parent: Some(AgentSessionParent::TaskGraphNode {
                    run_id: "run-1".to_string(),
                    node_id: "node-1".to_string(),
                }),
            },
        )
        .unwrap();
        let observer = CodeBuddyObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "gpt-5".to_string(),
        };
        let long_output = "x".repeat(9000);
        let stdout = format!(
            r#"{{"type":"system","subtype":"init","session_id":"cb-1","tools":["Bash","Read"]}}
{{"type":"assistant","message":{{"role":"assistant","model":"gpt-5","content":[{{"type":"text","text":"hello"}},{{"type":"thinking","thinking":"think"}},{{"type":"tool_use","id":"call-1","name":"Bash","input":{{"command":"pwd"}}}}],"usage":{{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":3,"cache_creation_input_tokens":2}}}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"call-1","content":"{}"}}]}}}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"call-2","name":"DeferExecuteTool","input":{{"toolName":"mcp__bb__list_projects","params":{{}}}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"call-2","content":"projects"}}]}}}}
plain runtime line
{{"type":"result","subtype":"error","is_error":true,"result":"boom","session_id":"cb-1"}}
"#,
            long_output
        );

        let capture = read_codebuddy_json_pipe_to_end(stdout.as_bytes(), &observer);

        assert_eq!(capture.provider_session_id.as_deref(), Some("cb-1"));
        assert_eq!(capture.text_output, "hello");
        assert_eq!(capture.result_output.as_deref(), Some("boom"));
        assert_eq!(capture.error_message.as_deref(), Some("boom"));
        assert_eq!(capture.tool_count, 2);
        assert_eq!(capture.usage["gpt-5"].input_tokens, 10);
        assert_eq!(capture.usage["gpt-5"].cache_read_tokens, 3);
        assert_eq!(capture.usage["gpt-5"].cache_write_tokens, 2);

        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Status));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Text));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Thinking));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::ToolUse));
        let tool_result = events
            .iter()
            .find(|event| event.event_type == AgentEventType::ToolResult)
            .expect("tool result event");
        assert_eq!(tool_result.tool.as_deref(), Some("Bash"));
        assert_eq!(tool_result.output.as_ref().unwrap().chars().count(), 8192);
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::ToolUse
                && event.tool.as_deref() == Some("mcp__bb__list_projects")
        }));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::UsageUpdate));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Log));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Error));
    }
}
