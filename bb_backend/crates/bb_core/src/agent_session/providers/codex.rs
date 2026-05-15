use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

use chrono::Utc;
use serde_json::Value;

use super::super::model::{AgentEvent, AgentEventType, TokenUsage};
use super::super::store;
use super::super::AgentSessionError;

const TOOL_OUTPUT_LIMIT: usize = 8192;

#[derive(Debug, Clone)]
pub(crate) struct CodexObserver {
    pub workspace_root: PathBuf,
    pub project: String,
    pub session_id: String,
    pub model_key: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexCapture {
    pub stdout: Vec<u8>,
    pub text_output: String,
    pub log: String,
    pub error_message: Option<String>,
    pub provider_session_id: Option<String>,
    pub usage: BTreeMap<String, TokenUsage>,
    pub event_count: u64,
    pub tool_count: u64,
    pub next_seq: u64,
}

impl Default for CodexCapture {
    fn default() -> Self {
        Self {
            stdout: Vec::new(),
            text_output: String::new(),
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

pub(crate) fn read_codex_json_pipe_to_end(
    pipe: impl Read,
    observer: CodexObserver,
) -> CodexCapture {
    let mut reader = BufReader::new(pipe);
    let mut capture = CodexCapture::default();
    let mut call_id_to_tool = HashMap::<String, String>::new();
    let mut completed_call_ids = HashSet::<String>::new();
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

        match serde_json::from_str::<Value>(line) {
            Ok(event) => {
                match handle_codex_event(
                    &observer,
                    &event,
                    &mut capture,
                    &mut call_id_to_tool,
                    &mut completed_call_ids,
                ) {
                    Ok(lines) => log_lines.extend(lines),
                    Err(err) => log_lines.push(format!("AgentSession persist error: {err}")),
                }
            }
            Err(_) => {
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
                    Some("info".to_string()),
                    None,
                    BTreeMap::new(),
                );
            }
        }
    }

    capture.log = log_lines.join("\n");
    capture
}

fn handle_codex_event(
    observer: &CodexObserver,
    event: &Value,
    capture: &mut CodexCapture,
    call_id_to_tool: &mut HashMap<String, String>,
    completed_call_ids: &mut HashSet<String>,
) -> Result<Vec<String>, AgentSessionError> {
    let top_type = string_field(event, "type").unwrap_or_default();
    let payload = event.get("payload").unwrap_or(event);
    let payload_type = string_field(payload, "type").unwrap_or_default();

    if top_type == "session_meta" {
        if let Some(provider_session_id) = string_field(payload, "id") {
            record_status(
                observer,
                capture,
                "started",
                Some(provider_session_id.clone()),
                true,
            )?;
            return Ok(vec![format!(
                "Codex session started session={provider_session_id}"
            )]);
        }
    }

    match top_type.as_str() {
        "thread.started" => {
            let thread_id = string_field(event, "thread_id");
            record_status(observer, capture, "started", thread_id.clone(), true)?;
            return Ok(vec![thread_id
                .map(|id| format!("Codex thread started thread={id}"))
                .unwrap_or_else(|| "Codex thread started".to_string())]);
        }
        "turn.started" => {
            append_event(
                observer,
                capture,
                AgentEventType::Status,
                None,
                None,
                None,
                None,
                None,
                Some("turn_started".to_string()),
                None,
                None,
                BTreeMap::new(),
            )?;
            return Ok(vec!["Codex turn started".to_string()]);
        }
        "turn.completed" => {
            let mut lines = Vec::new();
            if let Some(usage) = event.get("usage").and_then(token_usage_from_value) {
                capture
                    .usage
                    .entry(observer.model_key.clone())
                    .or_default()
                    .add_assign(&usage);
                let mut delta = BTreeMap::new();
                delta.insert(observer.model_key.clone(), usage.clone());
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
                )?;
                lines.push(format!(
                    "Token usage input={} output={} cache_read={}",
                    usage.input_tokens, usage.output_tokens, usage.cache_read_tokens
                ));
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
                Some("completed".to_string()),
                None,
                None,
                BTreeMap::new(),
            )?;
            lines.push("Codex turn completed".to_string());
            return Ok(lines);
        }
        "turn.failed" | "error" => {
            let message =
                event_error_message(event).unwrap_or_else(|| "unknown codex error".to_string());
            capture.error_message = Some(message.clone());
            append_event(
                observer,
                capture,
                AgentEventType::Error,
                Some(message.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BTreeMap::new(),
            )?;
            return Ok(vec![format!("Codex error: {message}")]);
        }
        "item.started" | "item.completed" => {
            if let Some(item) = event.get("item") {
                return handle_codex_item(
                    observer,
                    item,
                    capture,
                    call_id_to_tool,
                    completed_call_ids,
                );
            }
        }
        _ => {}
    }

    match (top_type.as_str(), payload_type.as_str()) {
        ("event_msg", "task_started") => {
            let turn_id = string_field(payload, "turn_id");
            let status = if capture.provider_session_id.is_some() {
                "turn_started"
            } else {
                "started"
            };
            record_status(observer, capture, status, turn_id.clone(), true)?;
            Ok(vec![turn_id
                .map(|id| format!("Codex turn started turn={id}"))
                .unwrap_or_else(|| "Codex turn started".to_string())])
        }
        ("event_msg", "task_complete") => {
            if capture.text_output.trim().is_empty() {
                if let Some(message) = string_field(payload, "last_agent_message") {
                    append_text(observer, capture, &message)?;
                }
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
                Some("completed".to_string()),
                None,
                string_field(payload, "turn_id"),
                BTreeMap::new(),
            )?;
            Ok(vec!["Codex turn completed".to_string()])
        }
        ("event_msg", "agent_message") => {
            if let Some(message) = string_field(payload, "message") {
                append_text(observer, capture, &message)?;
                Ok(vec![format!("AI output:\n{}", message.trim_end())])
            } else {
                Ok(Vec::new())
            }
        }
        ("event_msg", "agent_reasoning") => {
            if let Some(text) = string_field(payload, "text") {
                append_thinking(observer, capture, &text)?;
                Ok(vec![format!("Reasoning:\n{}", text.trim_end())])
            } else {
                Ok(Vec::new())
            }
        }
        ("response_item", "reasoning") => {
            let text = payload
                .get("content")
                .and_then(extract_text)
                .or_else(|| payload.get("summary").and_then(extract_text));
            if let Some(text) = text {
                append_thinking(observer, capture, &text)?;
                Ok(vec![format!("Reasoning:\n{}", text.trim_end())])
            } else {
                Ok(Vec::new())
            }
        }
        ("response_item", "function_call") => {
            let call_id = string_field(payload, "call_id");
            let tool = string_field(payload, "name").unwrap_or_else(|| "function_call".to_string());
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
                payload.get("arguments").map(parse_jsonish_value),
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
        ("event_msg", "dynamic_tool_call_request") => {
            let call_id =
                string_field(payload, "callId").or_else(|| string_field(payload, "call_id"));
            let tool = dynamic_tool_name(payload);
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
                payload.get("arguments").cloned(),
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
        ("event_msg", "exec_command_end") => {
            let call_id = string_field(payload, "call_id");
            let tool = call_id
                .as_ref()
                .and_then(|id| call_id_to_tool.get(id))
                .cloned()
                .unwrap_or_else(|| "exec_command".to_string());
            if let Some(call_id) = call_id.as_ref() {
                completed_call_ids.insert(call_id.clone());
            }
            let status = command_status(payload);
            let output = command_output(payload);
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
        ("event_msg", "dynamic_tool_call_response") => {
            let call_id = string_field(payload, "call_id");
            let tool = call_id
                .as_ref()
                .and_then(|id| call_id_to_tool.get(id))
                .cloned()
                .unwrap_or_else(|| dynamic_tool_name(payload));
            if let Some(call_id) = call_id.as_ref() {
                completed_call_ids.insert(call_id.clone());
            }
            let success = bool_field(payload, "success").unwrap_or(true);
            let status = if success { "completed" } else { "failed" }.to_string();
            let output = payload
                .get("content_items")
                .map(display_json_value)
                .or_else(|| string_field(payload, "error"))
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
        ("response_item", "function_call_output") => {
            let call_id = string_field(payload, "call_id");
            if call_id
                .as_ref()
                .map(|id| completed_call_ids.contains(id))
                .unwrap_or(false)
            {
                return Ok(Vec::new());
            }
            let tool = call_id
                .as_ref()
                .and_then(|id| call_id_to_tool.get(id))
                .cloned()
                .unwrap_or_else(|| "function_call".to_string());
            if let Some(call_id) = call_id.as_ref() {
                completed_call_ids.insert(call_id.clone());
            }
            let output = payload
                .get("output")
                .map(display_json_value)
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
                Some("completed".to_string()),
                None,
                None,
                BTreeMap::new(),
            )?;
            Ok(vec![format!(
                "Tool result {tool} call_id={} status=completed",
                call_id.as_deref().unwrap_or("-")
            )])
        }
        ("event_msg", "token_count") => {
            if let Some(usage) = token_usage_delta(payload) {
                capture
                    .usage
                    .entry(observer.model_key.clone())
                    .or_default()
                    .add_assign(&usage);
                let mut delta = BTreeMap::new();
                delta.insert(observer.model_key.clone(), usage.clone());
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
                )?;
                Ok(vec![format!(
                    "Token usage input={} output={} cache_read={}",
                    usage.input_tokens, usage.output_tokens, usage.cache_read_tokens
                )])
            } else {
                Ok(Vec::new())
            }
        }
        ("event_msg", "error") | ("event_msg", "task_failed") | ("error", _) => {
            let message =
                event_error_message(payload).unwrap_or_else(|| "unknown codex error".to_string());
            capture.error_message = Some(message.clone());
            append_event(
                observer,
                capture,
                AgentEventType::Error,
                Some(message.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                BTreeMap::new(),
            )?;
            Ok(vec![format!("Codex error: {message}")])
        }
        _ => Ok(Vec::new()),
    }
}

fn handle_codex_item(
    observer: &CodexObserver,
    item: &Value,
    capture: &mut CodexCapture,
    call_id_to_tool: &mut HashMap<String, String>,
    completed_call_ids: &mut HashSet<String>,
) -> Result<Vec<String>, AgentSessionError> {
    match string_field(item, "type").as_deref() {
        Some("agent_message") | Some("message") => {
            if let Some(text) = string_field(item, "text")
                .or_else(|| item.get("content").and_then(extract_text))
                .filter(|text| !text.trim().is_empty())
            {
                append_text(observer, capture, &text)?;
                Ok(vec![format!("AI output:\n{}", text.trim_end())])
            } else {
                Ok(Vec::new())
            }
        }
        Some("reasoning") => {
            let text = string_field(item, "text")
                .or_else(|| item.get("content").and_then(extract_text))
                .or_else(|| item.get("summary").and_then(extract_text));
            if let Some(text) = text {
                append_thinking(observer, capture, &text)?;
                Ok(vec![format!("Reasoning:\n{}", text.trim_end())])
            } else {
                Ok(Vec::new())
            }
        }
        Some("function_call") => {
            append_function_call_item(observer, item, capture, call_id_to_tool)
        }
        Some("function_call_output") => append_function_call_output_item(
            observer,
            item,
            capture,
            call_id_to_tool,
            completed_call_ids,
        ),
        _ => Ok(Vec::new()),
    }
}

fn append_function_call_item(
    observer: &CodexObserver,
    item: &Value,
    capture: &mut CodexCapture,
    call_id_to_tool: &mut HashMap<String, String>,
) -> Result<Vec<String>, AgentSessionError> {
    let call_id = string_field(item, "call_id");
    let tool = string_field(item, "name").unwrap_or_else(|| "function_call".to_string());
    if let Some(call_id) = call_id.as_deref().filter(|id| !id.is_empty()) {
        if call_id_to_tool.contains_key(call_id) {
            return Ok(Vec::new());
        }
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
        item.get("arguments").map(parse_jsonish_value),
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

fn append_function_call_output_item(
    observer: &CodexObserver,
    item: &Value,
    capture: &mut CodexCapture,
    call_id_to_tool: &HashMap<String, String>,
    completed_call_ids: &mut HashSet<String>,
) -> Result<Vec<String>, AgentSessionError> {
    let call_id = string_field(item, "call_id");
    if call_id
        .as_ref()
        .map(|id| completed_call_ids.contains(id))
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let tool = call_id
        .as_ref()
        .and_then(|id| call_id_to_tool.get(id))
        .cloned()
        .unwrap_or_else(|| "function_call".to_string());
    if let Some(call_id) = call_id.as_ref() {
        completed_call_ids.insert(call_id.clone());
    }
    let output = item
        .get("output")
        .map(display_json_value)
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
        Some("completed".to_string()),
        None,
        None,
        BTreeMap::new(),
    )?;
    Ok(vec![format!(
        "Tool result {tool} call_id={} status=completed",
        call_id.as_deref().unwrap_or("-")
    )])
}

fn record_status(
    observer: &CodexObserver,
    capture: &mut CodexCapture,
    status: &str,
    session_id: Option<String>,
    update_provider_id: bool,
) -> Result<(), AgentSessionError> {
    if update_provider_id && capture.provider_session_id.is_none() {
        if let Some(provider_session_id) = session_id.as_deref().filter(|id| !id.is_empty()) {
            capture.provider_session_id = Some(provider_session_id.to_string());
            store::update_provider_session_id(
                &observer.workspace_root,
                &observer.project,
                &observer.session_id,
                provider_session_id,
            )?;
        }
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
        Some(status.to_string()),
        None,
        session_id,
        BTreeMap::new(),
    )
}

fn append_text(
    observer: &CodexObserver,
    capture: &mut CodexCapture,
    text: &str,
) -> Result<(), AgentSessionError> {
    if text.is_empty() {
        return Ok(());
    }
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

fn append_thinking(
    observer: &CodexObserver,
    capture: &mut CodexCapture,
    text: &str,
) -> Result<(), AgentSessionError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    append_event(
        observer,
        capture,
        AgentEventType::Thinking,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_event(
    observer: &CodexObserver,
    capture: &mut CodexCapture,
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

fn dynamic_tool_name(payload: &Value) -> String {
    let tool = string_field(payload, "tool").unwrap_or_else(|| "dynamic_tool".to_string());
    string_field(payload, "namespace")
        .map(|namespace| format!("{namespace}.{tool}"))
        .unwrap_or(tool)
}

fn command_status(payload: &Value) -> String {
    match payload.get("exit_code").and_then(Value::as_i64) {
        Some(0) => "completed".to_string(),
        Some(_) => "failed".to_string(),
        None => "completed".to_string(),
    }
}

fn command_output(payload: &Value) -> String {
    if let Some(output) = string_field(payload, "aggregated_output") {
        return output;
    }
    let stdout = string_field(payload, "stdout").unwrap_or_default();
    let stderr = string_field(payload, "stderr").unwrap_or_default();
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn token_usage_delta(payload: &Value) -> Option<TokenUsage> {
    let usage = payload
        .get("info")
        .and_then(|info| info.get("last_token_usage"))
        .or_else(|| {
            payload
                .get("info")
                .and_then(|info| info.get("total_token_usage"))
        })?;
    token_usage_from_value(usage)
}

fn token_usage_from_value(usage: &Value) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: i64_field(usage, "input_tokens"),
        output_tokens: i64_field(usage, "output_tokens"),
        cache_read_tokens: i64_field(usage, "cached_input_tokens"),
        cache_write_tokens: 0,
    };
    (!usage.is_empty()).then_some(usage)
}

fn event_error_message(payload: &Value) -> Option<String> {
    string_field(payload, "message")
        .or_else(|| string_field(payload, "error"))
        .or_else(|| payload.get("error").map(display_json_value))
}

fn parse_jsonish_value(value: &Value) -> Value {
    match value {
        Value::String(text) => serde_json::from_str(text).unwrap_or_else(|_| value.clone()),
        _ => value.clone(),
    }
}

fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(extract_text).collect();
            (!parts.is_empty()).then(|| parts.join(""))
        }
        Value::Object(object) => object
            .get("text")
            .and_then(extract_text)
            .or_else(|| object.get("message").and_then(extract_text))
            .or_else(|| object.get("content").and_then(extract_text))
            .or_else(|| object.get("summary").and_then(extract_text)),
        _ => None,
    }
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
    fn codex_events_persist_status_text_tools_usage_and_error() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "codex".to_string(),
                agent: "codex".to_string(),
                model: Some("model-a".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = CodexObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "model-a".to_string(),
        };
        let stdout = r#"{"type":"session_meta","payload":{"id":"sess-1"}}
{"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}
{"type":"event_msg","payload":{"type":"agent_message","message":"hello"}}
{"type":"event_msg","payload":{"type":"agent_reasoning","text":"thinking"}}
{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"cmd\":\"pwd\"}","call_id":"call-1"}}
{"type":"event_msg","payload":{"type":"exec_command_end","call_id":"call-1","aggregated_output":"ok","exit_code":0}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"duplicate"}}
{"type":"event_msg","payload":{"type":"dynamic_tool_call_request","callId":"call-2","namespace":"mcp","tool":"search","arguments":{"q":"x"}}}
{"type":"event_msg","payload":{"type":"dynamic_tool_call_response","call_id":"call-2","namespace":"mcp","tool":"search","success":true,"content_items":[{"text":"done"}]}}
{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"cached_input_tokens":3,"output_tokens":5,"reasoning_output_tokens":2,"total_tokens":15}}}}
{"type":"event_msg","payload":{"type":"error","message":"boom"}}
"#;

        let capture = read_codex_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.provider_session_id.as_deref(), Some("sess-1"));
        assert_eq!(capture.text_output, "hello");
        assert_eq!(capture.error_message.as_deref(), Some("boom"));
        assert_eq!(capture.tool_count, 2);
        assert_eq!(capture.usage["model-a"].input_tokens, 10);
        assert_eq!(capture.usage["model-a"].cache_read_tokens, 3);
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
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == AgentEventType::ToolResult)
                .count(),
            2
        );
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::UsageUpdate));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::Error));
    }

    #[test]
    fn codex_current_cli_schema_persists_text_and_usage() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "codex".to_string(),
                agent: "codex".to_string(),
                model: Some("gpt-5.2".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = CodexObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "gpt-5.2".to_string(),
        };
        let stdout = r#"{"type":"thread.started","thread_id":"thread-1"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"hello current schema"}}
{"type":"turn.completed","usage":{"input_tokens":15230,"cached_input_tokens":2432,"output_tokens":23,"reasoning_output_tokens":12}}
"#;

        let capture = read_codex_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.provider_session_id.as_deref(), Some("thread-1"));
        assert_eq!(capture.text_output, "hello current schema");
        assert_eq!(capture.usage["gpt-5.2"].input_tokens, 15230);
        assert_eq!(capture.usage["gpt-5.2"].output_tokens, 23);
        assert_eq!(capture.usage["gpt-5.2"].cache_read_tokens, 2432);

        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Status
                && event.session_id.as_deref() == Some("thread-1")
        }));
        assert!(events.iter().any(|event| {
            event.event_type == AgentEventType::Text
                && event.content.as_deref() == Some("hello current schema")
        }));
        assert!(events
            .iter()
            .any(|event| event.event_type == AgentEventType::UsageUpdate));
    }

    #[test]
    fn codex_current_cli_function_call_started_completed_is_single_tool_use() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "codex".to_string(),
                agent: "codex".to_string(),
                model: Some("gpt-5.2".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = CodexObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "gpt-5.2".to_string(),
        };
        let stdout = r#"{"type":"thread.started","thread_id":"thread-1"}
{"type":"item.started","item":{"id":"item_1","type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"cmd\":\"pwd\"}"}}
{"type":"item.completed","item":{"id":"item_1","type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"cmd\":\"pwd\"}"}}
{"type":"item.completed","item":{"id":"item_2","type":"function_call_output","call_id":"call-1","output":"ok"}}
"#;

        let capture = read_codex_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.tool_count, 1);
        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == AgentEventType::ToolUse)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == AgentEventType::ToolResult)
                .count(),
            1
        );
    }
}
