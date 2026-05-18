use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

use chrono::Utc;
use serde::Deserialize;

use super::super::model::{AgentEvent, AgentEventType, TokenUsage};
use super::super::store;
use super::super::AgentSessionError;

const TOOL_OUTPUT_LIMIT: usize = 8192;

#[derive(Debug, Clone)]
pub struct OpenCodeObserver {
    pub workspace_root: PathBuf,
    pub project: String,
    pub session_id: String,
    pub model_key: String,
}

#[derive(Debug, Clone)]
pub struct OpenCodeCapture {
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

impl Default for OpenCodeCapture {
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

#[derive(Debug, Deserialize)]
struct OpenCodeEvent {
    #[serde(rename = "type", default)]
    event_type: String,
    #[serde(rename = "sessionID", default)]
    session_id: Option<String>,
    #[serde(default)]
    part: OpenCodeEventPart,
    #[serde(default)]
    error: Option<OpenCodeError>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenCodeEventPart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(rename = "callID", default)]
    call_id: Option<String>,
    #[serde(default)]
    state: Option<OpenCodeToolState>,
    #[serde(default)]
    tokens: Option<OpenCodeTokens>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeToolState {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeTokens {
    #[serde(default)]
    input: i64,
    #[serde(default)]
    output: i64,
    #[serde(default)]
    cache: Option<OpenCodeCacheTokens>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeCacheTokens {
    #[serde(default)]
    read: i64,
    #[serde(default)]
    write: i64,
}

#[derive(Debug, Deserialize)]
struct OpenCodeError {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    data: Option<OpenCodeErrorData>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeErrorData {
    #[serde(default)]
    message: Option<String>,
}

pub fn read_opencode_json_pipe_to_end(
    pipe: impl Read,
    observer: OpenCodeObserver,
) -> OpenCodeCapture {
    let mut reader = BufReader::new(pipe);
    let mut capture = OpenCodeCapture::default();
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

        if let Ok(event) = serde_json::from_str::<OpenCodeEvent>(line) {
            log_lines.extend(opencode_event_log_lines(&event));
            if let Err(err) =
                handle_opencode_event(&observer, &event, &mut capture, &mut call_id_to_tool)
            {
                log_lines.push(format!("AgentSession persist error: {err}"));
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
                Some("info".to_string()),
                None,
                BTreeMap::new(),
            );
        }
    }

    capture.log = log_lines.join("\n");
    capture
}

fn handle_opencode_event(
    observer: &OpenCodeObserver,
    event: &OpenCodeEvent,
    capture: &mut OpenCodeCapture,
    call_id_to_tool: &mut HashMap<String, String>,
) -> Result<(), AgentSessionError> {
    match event.event_type.as_str() {
        "step_start" => {
            if let Some(provider_session_id) = event.session_id.as_deref() {
                capture.provider_session_id = Some(provider_session_id.to_string());
                store::update_provider_session_id(
                    &observer.workspace_root,
                    &observer.project,
                    &observer.session_id,
                    provider_session_id,
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
                    Some(provider_session_id.to_string()),
                    BTreeMap::new(),
                )?;
            }
        }
        "text" => {
            if let Some(text) = event.part.text.as_deref().filter(|text| !text.is_empty()) {
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
                )?;
            }
        }
        "reasoning" | "thinking" => {
            if let Some(text) = opencode_thinking_text(event) {
                append_thinking(observer, capture, &text)?;
            }
        }
        "tool_use" => {
            let tool = event
                .part
                .tool
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let call_id = event.part.call_id.clone();
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
                event
                    .part
                    .state
                    .as_ref()
                    .and_then(|state| state.input.clone()),
                None,
                event
                    .part
                    .state
                    .as_ref()
                    .and_then(|state| state.status.clone()),
                None,
                None,
                BTreeMap::new(),
            )?;

            if let Some(output) = event
                .part
                .state
                .as_ref()
                .and_then(|state| state.output.as_ref())
            {
                let resolved_tool = call_id
                    .as_ref()
                    .and_then(|id| call_id_to_tool.get(id))
                    .cloned()
                    .unwrap_or(tool);
                append_event(
                    observer,
                    capture,
                    AgentEventType::ToolResult,
                    None,
                    Some(resolved_tool),
                    call_id,
                    None,
                    Some(truncate_chars(
                        &display_json_value(output),
                        TOOL_OUTPUT_LIMIT,
                    )),
                    event
                        .part
                        .state
                        .as_ref()
                        .and_then(|state| state.status.clone()),
                    None,
                    None,
                    BTreeMap::new(),
                )?;
            }
        }
        "step_finish" => {
            if let Some(tokens) = event.part.tokens.as_ref() {
                let usage = TokenUsage {
                    input_tokens: tokens.input,
                    output_tokens: tokens.output,
                    cache_read_tokens: tokens.cache.as_ref().map_or(0, |cache| cache.read),
                    cache_write_tokens: tokens.cache.as_ref().map_or(0, |cache| cache.write),
                };
                if !usage.is_empty() {
                    capture
                        .usage
                        .entry(observer.model_key.clone())
                        .or_default()
                        .add_assign(&usage);
                    let mut delta = BTreeMap::new();
                    delta.insert(observer.model_key.clone(), usage);
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
                }
            }
        }
        "error" => {
            let message = opencode_event_error_message(event)
                .unwrap_or_else(|| "unknown opencode error".to_string());
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
                None,
                None,
                BTreeMap::new(),
            )?;
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn append_event(
    observer: &OpenCodeObserver,
    capture: &mut OpenCodeCapture,
    event_type: AgentEventType,
    content: Option<String>,
    tool: Option<String>,
    call_id: Option<String>,
    input: Option<serde_json::Value>,
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

fn opencode_event_log_lines(event: &OpenCodeEvent) -> Vec<String> {
    match event.event_type.as_str() {
        "step_start" => {
            let session = event
                .session_id
                .as_deref()
                .map(|id| format!(" session={id}"))
                .unwrap_or_default();
            vec![format!("Step started{session}")]
        }
        "text" => event
            .part
            .text
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(|text| vec![format!("AI output:\n{}", text.trim_end())])
            .unwrap_or_default(),
        "reasoning" | "thinking" => opencode_thinking_text(event)
            .map(|text| vec![format!("Reasoning:\n{}", text.trim_end())])
            .unwrap_or_default(),
        "tool_use" => {
            let tool = event.part.tool.as_deref().unwrap_or("unknown");
            let call_id = event.part.call_id.as_deref().unwrap_or("-");
            let status = event
                .part
                .state
                .as_ref()
                .and_then(|state| state.status.as_deref())
                .unwrap_or("unknown");
            let mut lines = vec![format!("Tool {tool} call_id={call_id} status={status}")];

            if let Some(input) = event
                .part
                .state
                .as_ref()
                .and_then(|state| state.input.as_ref())
            {
                lines.push(format!(
                    "  input: {}",
                    truncate_chars(&compact_json(input), 800)
                ));
            }

            if let Some(output) = event
                .part
                .state
                .as_ref()
                .and_then(|state| state.output.as_ref())
            {
                lines.push(format!(
                    "  output: {}",
                    truncate_chars(&display_json_value(output), 1200)
                ));
            }

            lines
        }
        "error" => {
            let message = opencode_event_error_message(event)
                .unwrap_or_else(|| "unknown opencode error".to_string());
            vec![format!("OpenCode error: {message}")]
        }
        "step_finish" => event.part.tokens.as_ref().map_or_else(
            || vec!["Step finished".to_string()],
            |tokens| {
                let cache = tokens
                    .cache
                    .as_ref()
                    .map(|cache| format!(" cache_read={} cache_write={}", cache.read, cache.write))
                    .unwrap_or_default();
                vec![format!(
                    "Step finished tokens input={} output={}{}",
                    tokens.input, tokens.output, cache
                )]
            },
        ),
        other => {
            if other.is_empty() {
                Vec::new()
            } else {
                vec![format!("OpenCode event: {other}")]
            }
        }
    }
}

fn append_thinking(
    observer: &OpenCodeObserver,
    capture: &mut OpenCodeCapture,
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

fn opencode_thinking_text(event: &OpenCodeEvent) -> Option<String> {
    [
        event.part.text.as_deref(),
        event.part.thinking.as_deref(),
        event.part.content.as_deref(),
        event.part.summary.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::to_string)
    .find(|text| !text.trim().is_empty())
}

fn opencode_event_error_message(event: &OpenCodeEvent) -> Option<String> {
    if event.event_type != "error" {
        return None;
    }
    event
        .error
        .as_ref()
        .and_then(opencode_error_message)
        .or_else(|| Some("unknown opencode error".to_string()))
}

fn opencode_error_message(error: &OpenCodeError) -> Option<String> {
    error
        .data
        .as_ref()
        .and_then(|data| data.message.clone())
        .filter(|message| !message.trim().is_empty())
        .or_else(|| error.name.clone().filter(|name| !name.trim().is_empty()))
}

fn compact_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn display_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => compact_json(value),
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
    fn opencode_events_persist_status_text_tool_usage_and_error() {
        let temp = tempfile::tempdir().unwrap();
        let session = create_session(
            temp.path(),
            CreateAgentSession {
                project: "demo".to_string(),
                title: None,
                runtime: "opencode".to_string(),
                agent: "native".to_string(),
                model: Some("model-a".to_string()),
                variant: None,
                parent: None,
            },
        )
        .unwrap();
        let observer = OpenCodeObserver {
            workspace_root: temp.path().to_path_buf(),
            project: "demo".to_string(),
            session_id: session.id.clone(),
            model_key: "model-a".to_string(),
        };
        let stdout = r#"{"type":"step_start","sessionID":"sess-1","part":{}}
{"type":"text","part":{"text":"hello"}}
{"type":"reasoning","part":{"text":"think"}}
{"type":"tool_use","part":{"tool":"bash","callID":"call-1","state":{"status":"completed","input":{"command":"pwd"},"output":"ok"}}}
{"type":"step_finish","part":{"tokens":{"input":10,"output":5,"cache":{"read":1,"write":2}}}}
{"type":"error","error":{"name":"RateLimitError","data":{"message":"boom"}}}
"#;

        let capture = read_opencode_json_pipe_to_end(stdout.as_bytes(), observer);

        assert_eq!(capture.provider_session_id.as_deref(), Some("sess-1"));
        assert_eq!(capture.text_output, "hello");
        assert_eq!(capture.error_message.as_deref(), Some("boom"));
        assert_eq!(capture.tool_count, 1);
        assert_eq!(capture.usage["model-a"].input_tokens, 10);
        let events = read_events(temp.path(), "demo", &session.id, None).unwrap();
        assert_eq!(events.len(), 7);
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
    }
}
