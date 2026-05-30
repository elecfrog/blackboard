//! Runtime command builder and `OpenCode` JSON output parser.
//!
//! Handles legacy external LLM runtimes, timeout management, live log streaming,
//! and structured output capture.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::llm::ResolvedLlmInvocation;
use super::run_state::{self, ArtifactContentType, RunStatus};
use super::types::{LlmConfig, TaskGraphError};
use super::RunnerOptions;
use super::RunnerStepResult;

mod opencode;

use opencode::{read_opencode_json_pipe_to_end, runtime_emits_opencode_json};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct RuntimeCommandCapture {
    pub log: String,
    pub artifact: String,
    pub parse_source: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeCommandObserver {
    pub workspace_root: PathBuf,
    pub project: String,
    pub run_id: String,
    pub node_id: String,
    pub runtime: String,
}

// ─── Command builder ─────────────────────────────────────────────────────────

pub(super) fn run_runtime_command(
    opts: &RunnerOptions,
    invocation: &ResolvedLlmInvocation,
    project: &str,
    node_id: &str,
) -> Result<Output, std::io::Error> {
    let mut cmd = match invocation.runtime.as_str() {
        "codex" | "codebuddy" | "pi" => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "AgentSession runtimes must execute through AgentSession",
            ));
        }
        _ => {
            let opencode_program = crate::platform::resolve_spawn_program(&opts.opencode_path);
            let mut c = Command::new(opencode_program);
            c.arg("run");
            c.arg("--format").arg("json");
            if invocation.agent != "native" {
                c.arg("--agent").arg(&invocation.agent);
            }
            if let Some(ref model) = invocation.model {
                c.arg("--model").arg(model);
            }
            if let Some(ref variant) = invocation.variant {
                c.arg("--variant").arg(variant);
            }
            c.current_dir(&opts.workspace_root);
            c.arg("--dir").arg(&opts.workspace_root);
            c.arg("--dangerously-skip-permissions");
            c.arg("--title").arg(format!(
                "tg-{}-{}",
                project,
                &opts.run_id[..std::cmp::min(opts.run_id.len(), 16)]
            ));
            c.args(&invocation.custom_args);
            c.arg(&invocation.prompt);
            c
        }
    };

    for (key, value) in &invocation.custom_env {
        cmd.env(key, value);
    }
    cmd.env("BB_DAEMON", "1");
    cmd.env("BB_DAEMON_PROJECT", project);
    cmd.env("BB_DAEMON_AGENT", &invocation.agent);
    cmd.env("BB_TASK_GRAPH_RUN", &opts.run_id);
    cmd.env("BB_WORKSPACE_ROOT", &opts.workspace_root);
    cmd.env("BB_PROJECT_ROOT", &opts.workspace_root);
    cmd.env("BB_SCRIPTS_DIR", &opts.scripts_dir);
    if let Some(ref config) = invocation.opencode_config_content {
        cmd.env("OPENCODE_CONFIG_CONTENT", config);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn()?;
    let observer = RuntimeCommandObserver {
        workspace_root: opts.workspace_root.clone(),
        project: project.to_string(),
        run_id: opts.run_id.clone(),
        node_id: node_id.to_string(),
        runtime: invocation.runtime.clone(),
    };
    wait_with_timeout(child, opts.node_timeout, observer)
}

// ─── Timeout and process management ─────────────────────────────────────────

fn wait_with_timeout(
    mut child: Child,
    timeout: Duration,
    observer: RuntimeCommandObserver,
) -> Result<Output, std::io::Error> {
    let stdout_observer = observer.clone();
    let stdout_reader = child.stdout.take().map(|stdout| {
        thread::spawn(move || {
            if runtime_emits_opencode_json(&stdout_observer.runtime) {
                read_opencode_json_pipe_to_end(stdout, stdout_observer)
            } else {
                read_pipe_to_end(stdout)
            }
        })
    });
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));
    let deadline = Instant::now() + timeout;
    loop {
        if observer_run_cancelled(&observer) {
            let _ = crate::platform::terminate_child_process_tree(&mut child);
            let status = child.wait()?;
            let stdout = join_pipe_reader(stdout_reader)?;
            let stderr = join_pipe_reader(stderr_reader)?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }

        if let Some(status) = child.try_wait()? {
            let stdout = join_pipe_reader(stdout_reader)?;
            let stderr = join_pipe_reader(stderr_reader)?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if Instant::now() >= deadline {
            let _ = crate::platform::terminate_child_process_tree(&mut child);
            let status = child.wait()?;
            let stdout = join_pipe_reader(stdout_reader)?;
            let stderr = join_pipe_reader(stderr_reader)?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn observer_run_cancelled(observer: &RuntimeCommandObserver) -> bool {
    matches!(
        run_state::read_run(
            &observer.workspace_root,
            &observer.project,
            &observer.run_id
        )
        .map(|run| run.status),
        Ok(RunStatus::Cancelled)
    )
}

fn read_pipe_to_end(mut pipe: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = pipe.read_to_end(&mut buffer);
    buffer
}

fn join_pipe_reader(
    reader: Option<thread::JoinHandle<Vec<u8>>>,
) -> Result<Vec<u8>, std::io::Error> {
    Ok(reader
        .map(|handle| handle.join().unwrap_or_else(|_| Vec::new()))
        .unwrap_or_default())
}

pub(super) fn capture_runtime_output(
    runtime: &str,
    stdout: &str,
    stderr: &str,
) -> RuntimeCommandCapture {
    opencode::capture_runtime_output(runtime, stdout, stderr)
}

// ─── Shared utility functions ────────────────────────────────────────────────

pub(super) fn runtime_failure_message(
    capture: &RuntimeCommandCapture,
    exit_code: Option<i32>,
) -> String {
    if let Some(message) = capture
        .error_message
        .as_ref()
        .filter(|message| !message.trim().is_empty())
    {
        return message.clone();
    }

    let tail = tail_str(&capture.log, 2048);
    if tail.trim().is_empty() {
        format!("runtime exited with code {exit_code:?}")
    } else {
        tail
    }
}

pub(super) fn run_is_cancelled(ws: &Path, project: &str, run_id: &str) -> bool {
    matches!(
        run_state::read_run(ws, project, run_id).map(|run| run.status),
        Ok(RunStatus::Cancelled)
    )
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(super) fn mark_node_cancelled(
    ws: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    start_time: String,
    end_time: String,
    duration_ms: u64,
    exit_code: Option<i32>,
    capture: &RuntimeCommandCapture,
) -> Result<RunnerStepResult, TaskGraphError> {
    use super::run_state::{NodeRunStatus, TaskGraphRunNode};

    let tail = tail_str(&capture.log, 4096);
    let log_tail = if tail.trim().is_empty() {
        "cancelled".to_string()
    } else {
        format!("cancelled\n{tail}")
    };
    let node_state = TaskGraphRunNode {
        node_id: node_id.to_string(),
        status: NodeRunStatus::Skipped,
        started_at: Some(start_time),
        completed_at: Some(end_time),
        duration_ms: Some(duration_ms),
        iteration: None,
        exit_code,
        error: None,
        output_artifact: None,
        log_tail: Some(log_tail),
        child_run_id: None,
        runtime: None,
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    };
    run_state::update_node_state(ws, project, run_id, &node_state)?;
    Ok(RunnerStepResult::Completed("cancelled".to_string()))
}

pub(super) fn try_parse_json_or_text(s: &str) -> serde_json::Value {
    let trimmed = s.trim();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return val;
    }
    let mut graph_candidate = None;
    let mut fallback = None;
    for (start, _) in trimmed.match_indices('{') {
        let mut deserializer = serde_json::Deserializer::from_str(&trimmed[start..]);
        if let Ok(val) = serde_json::Value::deserialize(&mut deserializer) {
            if val.get("processed").is_some() || val.get("continue").is_some() {
                return val;
            }
            if graph_candidate.is_none() && is_task_graph_like_json(&val) {
                graph_candidate = Some(val.clone());
            }
            fallback = Some(val);
        }
    }
    graph_candidate
        .or(fallback)
        .unwrap_or_else(|| serde_json::Value::String(s.to_string()))
}

pub(super) fn output_value_for_llm_config(
    config: &LlmConfig,
    parse_source: &str,
    artifact: &str,
) -> serde_json::Value {
    let artifact_type = effective_llm_output_contract(config)
        .and_then(|o| o.get("artifact_type"))
        .and_then(|v| v.as_str());
    match artifact_type {
        Some("markdown" | "text") => serde_json::Value::String(
            if artifact.trim().is_empty() {
                parse_source
            } else {
                artifact
            }
            .to_string(),
        ),
        Some("json") => parse_json_artifact_value(parse_source),
        _ => try_parse_json_or_text(parse_source),
    }
}

fn parse_json_artifact_value(parse_source: &str) -> serde_json::Value {
    let trimmed = parse_source.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return value;
    }

    if matches!(trimmed.chars().next(), Some('{') | Some('[')) {
        return serde_json::Value::String(parse_source.to_string());
    }

    try_parse_json_or_text(parse_source)
}

fn is_task_graph_like_json(value: &serde_json::Value) -> bool {
    let graph = value
        .get("subgraph")
        .or_else(|| value.get("graph"))
        .unwrap_or(value);
    graph.get("schema_version").is_some()
        && graph.get("nodes").is_some()
        && graph.get("edges").is_some()
}

pub(super) fn combine_command_output(stdout: &str, stderr: &str) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

pub(super) fn strip_ansi_codes(input: &str) -> String {
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

pub(super) fn tail_str(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        s.chars().skip(char_count - max).collect()
    }
}

pub(super) fn write_node_artifact(
    ws: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    content: &str,
    config: &LlmConfig,
) -> Result<Option<super::run_state::OutputArtifact>, TaskGraphError> {
    let artifact_type = effective_llm_output_contract(config)
        .and_then(|o| o.get("artifact_type"))
        .and_then(|v| v.as_str());

    let Some(art_type) = artifact_type else {
        return Ok(None);
    };

    let content_type = match art_type {
        "json" => ArtifactContentType::Json,
        "markdown" => ArtifactContentType::Markdown,
        _ => ArtifactContentType::Text,
    };

    let artifact_id = format!("{node_id}-output");
    let artifact =
        run_state::write_artifact(ws, project, run_id, &artifact_id, content, content_type)?;
    Ok(Some(artifact))
}

pub(super) fn artifact_content_for_llm_config(
    config: &LlmConfig,
    fallback: &str,
    output_value: &serde_json::Value,
) -> String {
    let artifact_type = effective_llm_output_contract(config)
        .and_then(|o| o.get("artifact_type"))
        .and_then(|v| v.as_str());
    match artifact_type {
        Some("json") => {
            artifact_content_for_type(ArtifactContentType::Json, fallback, output_value)
        }
        _ => fallback.to_string(),
    }
}

fn effective_llm_output_contract(config: &LlmConfig) -> Option<&serde_json::Value> {
    config.output_contract.as_ref().or(config.output.as_ref())
}

pub(super) fn artifact_content_for_type(
    content_type: ArtifactContentType,
    fallback: &str,
    output_value: &serde_json::Value,
) -> String {
    if content_type == ArtifactContentType::Json && !output_value.is_string() {
        serde_json::to_string_pretty(output_value).unwrap_or_else(|_| fallback.to_string())
    } else {
        fallback.to_string()
    }
}

#[cfg(test)]
mod tests;
