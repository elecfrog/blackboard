use std::io::{BufRead, BufReader, Read};

use serde::Deserialize;

use super::super::run_state;
use super::{
    combine_command_output, strip_ansi_codes, tail_str, RuntimeCommandCapture,
    RuntimeCommandObserver,
};

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

pub(super) fn read_opencode_json_pipe_to_end(
    pipe: impl Read,
    observer: RuntimeCommandObserver,
) -> Vec<u8> {
    let mut reader = BufReader::new(pipe);
    let mut output = Vec::new();
    let mut live_tail = String::new();
    let mut line = String::new();

    loop {
        line.clear();
        let Ok(read) = reader.read_line(&mut line) else {
            break;
        };
        if read == 0 {
            break;
        }

        output.extend_from_slice(line.as_bytes());
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let log_lines = serde_json::from_str::<OpenCodeEvent>(line)
            .map(|event| opencode_event_log_lines(&event))
            .unwrap_or_else(|_| vec![strip_ansi_codes(line)]);
        if log_lines.is_empty() {
            continue;
        }

        let block = log_lines.join("\n");
        if !live_tail.is_empty() {
            live_tail.push('\n');
        }
        live_tail.push_str(&block);
        live_tail = tail_str(&live_tail, 4096);

        let _ = run_state::append_node_log(
            &observer.workspace_root,
            &observer.project,
            &observer.run_id,
            &observer.node_id,
            &block,
        );
        let _ = run_state::update_node_log_tail(
            &observer.workspace_root,
            &observer.project,
            &observer.run_id,
            &observer.node_id,
            &live_tail,
        );
    }

    output
}

// ─── Output capture and parsing ──────────────────────────────────────────────

pub(super) fn capture_runtime_output(
    runtime: &str,
    stdout: &str,
    stderr: &str,
) -> RuntimeCommandCapture {
    if runtime_emits_opencode_json(runtime) {
        return capture_opencode_json_output(stdout, stderr);
    }

    let combined = strip_ansi_codes(&combine_command_output(stdout, stderr));
    RuntimeCommandCapture {
        log: combined.clone(),
        artifact: combined.clone(),
        parse_source: combined,
        error_message: None,
    }
}

pub(super) fn runtime_emits_opencode_json(runtime: &str) -> bool {
    runtime == "opencode"
}

pub(super) fn capture_opencode_json_output(stdout: &str, stderr: &str) -> RuntimeCommandCapture {
    let mut text_output = String::new();
    let mut log_lines = Vec::new();
    let mut raw_lines = Vec::new();
    let mut error_message = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(event) = serde_json::from_str::<OpenCodeEvent>(line) else {
            raw_lines.push(strip_ansi_codes(line));
            continue;
        };

        if event.event_type == "text" {
            if let Some(text) = event.part.text.as_deref() {
                text_output.push_str(text);
            }
        }
        if let Some(message) = opencode_event_error_message(&event) {
            error_message = Some(message);
        }
        log_lines.extend(opencode_event_log_lines(&event));
    }

    if !raw_lines.is_empty() {
        log_lines.push("Raw output:".to_string());
        log_lines.extend(raw_lines);
    }

    let stderr = strip_ansi_codes(stderr);
    if !stderr.trim().is_empty() {
        log_lines.push(format!("stderr:\n{}", tail_str(stderr.trim(), 2048)));
    }

    let log = log_lines.join("\n");
    let combined = strip_ansi_codes(&combine_command_output(stdout, stderr.as_str()));
    let parse_source = if text_output.trim().is_empty() {
        combined.clone()
    } else {
        text_output.clone()
    };
    let artifact = if text_output.trim().is_empty() {
        if log.trim().is_empty() {
            combined
        } else {
            log.clone()
        }
    } else {
        text_output
    };

    RuntimeCommandCapture {
        log,
        artifact,
        parse_source,
        error_message,
    }
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
                lines.push(format!("  input: {}", tail_str(&compact_json(input), 800)));
            }

            if let Some(output) = event
                .part
                .state
                .as_ref()
                .and_then(|state| state.output.as_ref())
            {
                let output = display_json_value(output);
                lines.push(format!("  output: {}", tail_str(&output, 1200)));
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
