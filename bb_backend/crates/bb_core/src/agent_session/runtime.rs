use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::model::{
    AgentEventType, AgentResult, AgentResultStatus, AgentSessionStatus, AgentSessionSummary,
};
use super::providers::codebuddy::{
    append_event as append_codebuddy_event, read_codebuddy_json_pipe_to_end, CodeBuddyCapture,
    CodeBuddyObserver,
};
use super::providers::codex::{
    append_event as append_codex_event, read_codex_json_pipe_to_end, CodexCapture, CodexObserver,
};
use super::providers::opencode::{
    append_event as append_opencode_event, read_opencode_json_pipe_to_end, OpenCodeCapture,
    OpenCodeObserver,
};
use super::store::{self, FinalizeStats};
use super::AgentSessionError;

#[derive(Debug, Clone)]
pub struct AgentTurnRequest {
    pub workspace_root: PathBuf,
    pub scripts_dir: PathBuf,
    pub execution_root: PathBuf,
    pub project: String,
    pub session_id: String,
    pub runtime: String,
    pub agent: String,
    pub model: Option<String>,
    pub variant: Option<String>,
    pub continue_provider_session_id: Option<String>,
    pub prompt: String,
    pub codex_path: String,
    pub codex_config_args: Vec<String>,
    pub codebuddy_path: String,
    pub codebuddy_mcp_config_content: Option<String>,
    pub codebuddy_settings_json: Option<String>,
    pub opencode_path: String,
    pub opencode_config_content: Option<String>,
    pub custom_env: BTreeMap<String, String>,
    pub custom_args: Vec<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct AgentTurnOutcome {
    pub session: AgentSessionSummary,
    pub result: AgentResult,
    pub exit_code: Option<i32>,
    pub artifact: String,
    pub parse_source: String,
    pub log: String,
}

pub fn run_turn<F>(
    request: AgentTurnRequest,
    cancel_check: F,
) -> Result<AgentTurnOutcome, AgentSessionError>
where
    F: Fn() -> bool,
{
    match request.runtime.as_str() {
        "opencode" => run_opencode_turn(request, cancel_check),
        "codex" => run_codex_turn(request, cancel_check),
        "codebuddy" => run_codebuddy_turn(request, cancel_check),
        runtime => Err(AgentSessionError::InvalidInput(format!(
            "AgentSession does not support runtime {runtime}"
        ))),
    }
}

fn run_opencode_turn<F>(
    request: AgentTurnRequest,
    cancel_check: F,
) -> Result<AgentTurnOutcome, AgentSessionError>
where
    F: Fn() -> bool,
{
    store::update_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        |session| {
            session.status = AgentSessionStatus::Running;
        },
    )?;

    let start = Instant::now();
    let opencode_program = crate::platform::resolve_spawn_program(&request.opencode_path);
    let mut cmd = Command::new(&opencode_program);
    cmd.arg("run").arg("--format").arg("json");
    cmd.arg("--thinking");
    if let Some(session_id) = request
        .continue_provider_session_id
        .as_ref()
        .filter(|session_id| !session_id.is_empty())
    {
        cmd.arg("--session").arg(session_id);
    }
    if request.agent != "native" && request.agent != "opencode" {
        cmd.arg("--agent").arg(&request.agent);
    }
    if let Some(model) = request.model.as_ref().filter(|model| !model.is_empty()) {
        cmd.arg("--model").arg(model);
    }
    if let Some(variant) = request
        .variant
        .as_ref()
        .filter(|variant| !variant.is_empty())
    {
        cmd.arg("--variant").arg(variant);
    }
    cmd.current_dir(&request.execution_root);
    cmd.arg("--dir").arg(&request.execution_root);
    cmd.arg("--dangerously-skip-permissions");
    cmd.arg("--title").arg(format!(
        "as-{}",
        &request.session_id[..request.session_id.len().min(24)]
    ));
    cmd.args(&request.custom_args);
    cmd.arg(&request.prompt);

    for (key, value) in &request.custom_env {
        cmd.env(key, value);
    }
    cmd.env("BB_DAEMON", "1");
    cmd.env("BB_DAEMON_PROJECT", &request.project);
    cmd.env("BB_DAEMON_AGENT", &request.agent);
    cmd.env("BB_AGENT_SESSION", &request.session_id);
    cmd.env("BB_WORKSPACE_ROOT", &request.workspace_root);
    cmd.env("BB_PROJECT_ROOT", &request.execution_root);
    cmd.env("BB_SCRIPTS_DIR", &request.scripts_dir);
    if let Some(config) = request.opencode_config_content.as_ref() {
        cmd.env("OPENCODE_CONFIG_CONTENT", config);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| AgentSessionError::Spawn {
        program: opencode_program.clone(),
        source,
    })?;

    let observer = OpenCodeObserver {
        workspace_root: request.workspace_root.clone(),
        project: request.project.clone(),
        session_id: request.session_id.clone(),
        model_key: request
            .model
            .clone()
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_opencode_json_pipe_to_end(stdout, observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let (status, terminal_reason) =
        wait_with_timeout(&mut child, request.timeout, &opencode_program, cancel_check)?;
    let mut capture = join_stdout_reader(stdout_reader)?;
    let stderr = join_pipe_reader(stderr_reader)?;
    let stderr_text = String::from_utf8_lossy(&stderr).to_string();
    if !stderr_text.trim().is_empty() {
        let content = format!("stderr:\n{}", tail_str(stderr_text.trim(), 2048));
        let observer = OpenCodeObserver {
            workspace_root: request.workspace_root.clone(),
            project: request.project.clone(),
            session_id: request.session_id.clone(),
            model_key: request
                .model
                .clone()
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
        };
        append_opencode_event(
            &observer,
            &mut capture,
            AgentEventType::Log,
            Some(content.clone()),
            None,
            None,
            None,
            None,
            None,
            Some("stderr".to_string()),
            None,
            Default::default(),
        )?;
        if capture.log.trim().is_empty() {
            capture.log = content;
        } else {
            capture.log.push('\n');
            capture.log.push_str(&content);
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let result_status = match terminal_reason {
        TerminalReason::Cancelled => AgentResultStatus::Cancelled,
        TerminalReason::Timeout => AgentResultStatus::Timeout,
        TerminalReason::Exited => {
            if status.map(|status| status.success()).unwrap_or(false)
                && capture.error_message.is_none()
            {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let error = result_error(
        result_status,
        status,
        capture.error_message.as_deref(),
        &capture.log,
    );
    let session_status = match result_status {
        AgentResultStatus::Completed => AgentSessionStatus::Idle,
        AgentResultStatus::Cancelled => AgentSessionStatus::Cancelled,
        AgentResultStatus::Failed | AgentResultStatus::Timeout => AgentSessionStatus::Failed,
    };
    let session = store::finalize_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        session_status,
        capture.provider_session_id.clone(),
        FinalizeStats {
            event_count: capture.event_count,
            tool_count: capture.tool_count,
            usage: capture.usage.clone(),
        },
    )?;

    let stdout = String::from_utf8_lossy(&capture.stdout).to_string();
    let combined = combine_command_output(&stdout, &stderr_text);
    let parse_source = if capture.text_output.trim().is_empty() {
        combined.clone()
    } else {
        capture.text_output.clone()
    };
    let artifact = if capture.text_output.trim().is_empty() {
        if capture.log.trim().is_empty() {
            combined
        } else {
            capture.log.clone()
        }
    } else {
        capture.text_output.clone()
    };

    Ok(AgentTurnOutcome {
        session: store::session_summary(&session),
        result: AgentResult {
            status: result_status,
            output: capture.text_output,
            error,
            duration_ms,
            provider_session_id: capture.provider_session_id,
            usage: capture.usage,
        },
        exit_code: status.and_then(|status| status.code()),
        artifact,
        parse_source,
        log: capture.log,
    })
}

fn run_codex_turn<F>(
    request: AgentTurnRequest,
    cancel_check: F,
) -> Result<AgentTurnOutcome, AgentSessionError>
where
    F: Fn() -> bool,
{
    store::update_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        |session| {
            session.status = AgentSessionStatus::Running;
        },
    )?;

    let start = Instant::now();
    let codex_program = crate::platform::resolve_spawn_program(&request.codex_path);
    let mut cmd = Command::new(&codex_program);
    cmd.arg("exec");
    if let Some(model) = request.model.as_ref().filter(|model| !model.is_empty()) {
        cmd.arg("--model").arg(model);
    }
    for config_arg in &request.codex_config_args {
        cmd.arg("--config").arg(config_arg);
    }
    cmd.current_dir(&request.execution_root);
    cmd.arg("-C").arg(&request.execution_root);
    cmd.arg("--json");
    cmd.arg("--dangerously-bypass-approvals-and-sandbox");
    cmd.arg("--ephemeral");
    cmd.arg("--ignore-user-config");
    cmd.args(&request.custom_args);
    cmd.arg(&request.prompt);

    for (key, value) in &request.custom_env {
        cmd.env(key, value);
    }
    cmd.env("BB_DAEMON", "1");
    cmd.env("BB_DAEMON_PROJECT", &request.project);
    cmd.env("BB_DAEMON_AGENT", &request.agent);
    cmd.env("BB_AGENT_SESSION", &request.session_id);
    cmd.env("BB_WORKSPACE_ROOT", &request.workspace_root);
    cmd.env("BB_PROJECT_ROOT", &request.execution_root);
    cmd.env("BB_SCRIPTS_DIR", &request.scripts_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| AgentSessionError::Spawn {
        program: codex_program.clone(),
        source,
    })?;

    let observer = CodexObserver {
        workspace_root: request.workspace_root.clone(),
        project: request.project.clone(),
        session_id: request.session_id.clone(),
        model_key: request
            .model
            .clone()
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_codex_json_pipe_to_end(stdout, observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let (status, terminal_reason) =
        wait_with_timeout(&mut child, request.timeout, &codex_program, cancel_check)?;
    let mut capture = join_codex_stdout_reader(stdout_reader)?;
    let stderr = join_pipe_reader(stderr_reader)?;
    let stderr_text = String::from_utf8_lossy(&stderr).to_string();
    if !stderr_text.trim().is_empty() {
        let content = format!("stderr:\n{}", tail_str(stderr_text.trim(), 2048));
        let observer = CodexObserver {
            workspace_root: request.workspace_root.clone(),
            project: request.project.clone(),
            session_id: request.session_id.clone(),
            model_key: request
                .model
                .clone()
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
        };
        append_codex_event(
            &observer,
            &mut capture,
            AgentEventType::Log,
            Some(content.clone()),
            None,
            None,
            None,
            None,
            None,
            Some("stderr".to_string()),
            None,
            Default::default(),
        )?;
        if capture.log.trim().is_empty() {
            capture.log = content;
        } else {
            capture.log.push('\n');
            capture.log.push_str(&content);
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let result_status = match terminal_reason {
        TerminalReason::Cancelled => AgentResultStatus::Cancelled,
        TerminalReason::Timeout => AgentResultStatus::Timeout,
        TerminalReason::Exited => {
            if status.map(|status| status.success()).unwrap_or(false)
                && capture.error_message.is_none()
            {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let error = result_error(
        result_status,
        status,
        capture.error_message.as_deref(),
        &capture.log,
    );
    let session_status = match result_status {
        AgentResultStatus::Completed => AgentSessionStatus::Idle,
        AgentResultStatus::Cancelled => AgentSessionStatus::Cancelled,
        AgentResultStatus::Failed | AgentResultStatus::Timeout => AgentSessionStatus::Failed,
    };
    let session = store::finalize_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        session_status,
        capture.provider_session_id.clone(),
        FinalizeStats {
            event_count: capture.event_count,
            tool_count: capture.tool_count,
            usage: capture.usage.clone(),
        },
    )?;

    let stdout = String::from_utf8_lossy(&capture.stdout).to_string();
    let combined = combine_command_output(&stdout, &stderr_text);
    let parse_source = if capture.text_output.trim().is_empty() {
        combined.clone()
    } else {
        capture.text_output.clone()
    };
    let artifact = if capture.text_output.trim().is_empty() {
        if capture.log.trim().is_empty() {
            combined
        } else {
            capture.log.clone()
        }
    } else {
        capture.text_output.clone()
    };

    Ok(AgentTurnOutcome {
        session: store::session_summary(&session),
        result: AgentResult {
            status: result_status,
            output: capture.text_output,
            error,
            duration_ms,
            provider_session_id: capture.provider_session_id,
            usage: capture.usage,
        },
        exit_code: status.and_then(|status| status.code()),
        artifact,
        parse_source,
        log: capture.log,
    })
}

fn run_codebuddy_turn<F>(
    request: AgentTurnRequest,
    cancel_check: F,
) -> Result<AgentTurnOutcome, AgentSessionError>
where
    F: Fn() -> bool,
{
    store::update_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        |session| {
            session.status = AgentSessionStatus::Running;
        },
    )?;

    let start = Instant::now();
    let mcp_config_path = write_codebuddy_mcp_config(&request)?;
    let codebuddy_program = crate::platform::resolve_spawn_program(&request.codebuddy_path);
    let mut cmd = Command::new(&codebuddy_program);
    cmd.current_dir(&request.execution_root);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("-y");
    if let Some(path) = mcp_config_path.as_ref() {
        cmd.arg("--strict-mcp-config").arg("--mcp-config").arg(path);
    }
    if let Some(settings) = request
        .codebuddy_settings_json
        .as_ref()
        .filter(|settings| !settings.trim().is_empty())
    {
        cmd.arg("--settings").arg(settings);
    }
    if let Some(model) = request.model.as_ref().filter(|model| !model.is_empty()) {
        cmd.arg("--model").arg(model);
    }
    if !request.agent.trim().is_empty() && request.agent != "native" {
        cmd.arg("--agent").arg(&request.agent);
    }
    cmd.args(&request.custom_args);
    cmd.arg(&request.prompt);

    for (key, value) in &request.custom_env {
        cmd.env(key, value);
    }
    cmd.env("BB_DAEMON", "1");
    cmd.env("BB_DAEMON_PROJECT", &request.project);
    cmd.env("BB_DAEMON_AGENT", &request.agent);
    cmd.env("BB_AGENT_SESSION", &request.session_id);
    cmd.env("BB_WORKSPACE_ROOT", &request.workspace_root);
    cmd.env("BB_PROJECT_ROOT", &request.execution_root);
    cmd.env("BB_SCRIPTS_DIR", &request.scripts_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| AgentSessionError::Spawn {
        program: codebuddy_program.clone(),
        source,
    })?;

    let observer = CodeBuddyObserver {
        workspace_root: request.workspace_root.clone(),
        project: request.project.clone(),
        session_id: request.session_id.clone(),
        model_key: request
            .model
            .clone()
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_codebuddy_json_pipe_to_end(stdout, observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let (status, terminal_reason) = wait_with_timeout(
        &mut child,
        request.timeout,
        &codebuddy_program,
        cancel_check,
    )?;
    let mut capture = join_codebuddy_stdout_reader(stdout_reader)?;
    let stderr = join_pipe_reader(stderr_reader)?;
    let stderr_text = String::from_utf8_lossy(&stderr).to_string();
    if !stderr_text.trim().is_empty() {
        let content = format!("stderr:\n{}", tail_str(stderr_text.trim(), 2048));
        let observer = CodeBuddyObserver {
            workspace_root: request.workspace_root.clone(),
            project: request.project.clone(),
            session_id: request.session_id.clone(),
            model_key: request
                .model
                .clone()
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
        };
        append_codebuddy_event(
            &observer,
            &mut capture,
            AgentEventType::Log,
            Some(content.clone()),
            None,
            None,
            None,
            None,
            None,
            Some("stderr".to_string()),
            None,
            Default::default(),
        )?;
        if capture.log.trim().is_empty() {
            capture.log = content;
        } else {
            capture.log.push('\n');
            capture.log.push_str(&content);
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let result_status = match terminal_reason {
        TerminalReason::Cancelled => AgentResultStatus::Cancelled,
        TerminalReason::Timeout => AgentResultStatus::Timeout,
        TerminalReason::Exited => {
            if status.map(|status| status.success()).unwrap_or(false)
                && capture.error_message.is_none()
            {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let error = result_error(
        result_status,
        status,
        capture.error_message.as_deref(),
        &capture.log,
    );
    let session_status = match result_status {
        AgentResultStatus::Completed => AgentSessionStatus::Idle,
        AgentResultStatus::Cancelled => AgentSessionStatus::Cancelled,
        AgentResultStatus::Failed | AgentResultStatus::Timeout => AgentSessionStatus::Failed,
    };
    let session = store::finalize_session(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        session_status,
        capture.provider_session_id.clone(),
        FinalizeStats {
            event_count: capture.event_count,
            tool_count: capture.tool_count,
            usage: capture.usage.clone(),
        },
    )?;

    let stdout = String::from_utf8_lossy(&capture.stdout).to_string();
    let combined = combine_command_output(&stdout, &stderr_text);
    let output_text = capture
        .result_output
        .clone()
        .filter(|output| !output.trim().is_empty())
        .unwrap_or_else(|| capture.text_output.clone());
    let parse_source = if output_text.trim().is_empty() {
        combined.clone()
    } else {
        output_text.clone()
    };
    let artifact = if output_text.trim().is_empty() {
        if capture.log.trim().is_empty() {
            combined
        } else {
            capture.log.clone()
        }
    } else {
        output_text.clone()
    };

    Ok(AgentTurnOutcome {
        session: store::session_summary(&session),
        result: AgentResult {
            status: result_status,
            output: output_text,
            error,
            duration_ms,
            provider_session_id: capture.provider_session_id,
            usage: capture.usage,
        },
        exit_code: status.and_then(|status| status.code()),
        artifact,
        parse_source,
        log: capture.log,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalReason {
    Exited,
    Timeout,
    Cancelled,
}

fn wait_with_timeout<F>(
    child: &mut Child,
    timeout: Duration,
    program: &str,
    cancel_check: F,
) -> Result<(Option<ExitStatus>, TerminalReason), AgentSessionError>
where
    F: Fn() -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        if cancel_check() {
            let _ = child.kill();
            let status = child.wait().ok();
            return Ok((status, TerminalReason::Cancelled));
        }
        if let Some(status) = child.try_wait().map_err(|source| AgentSessionError::Io {
            path: PathBuf::from(program),
            source,
        })? {
            return Ok((Some(status), TerminalReason::Exited));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().ok();
            return Ok((status, TerminalReason::Timeout));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn result_error(
    status: AgentResultStatus,
    exit_status: Option<ExitStatus>,
    error_message: Option<&str>,
    log: &str,
) -> Option<String> {
    match status {
        AgentResultStatus::Completed => None,
        AgentResultStatus::Cancelled => Some("agent session was cancelled".to_string()),
        AgentResultStatus::Timeout => Some("agent session timed out".to_string()),
        AgentResultStatus::Failed => error_message.map(str::to_string).or_else(|| {
            if log.trim().is_empty() {
                Some(format!(
                    "runtime exited with code {:?}",
                    exit_status.and_then(|status| status.code())
                ))
            } else {
                Some(tail_str(log, 2048))
            }
        }),
    }
}

fn read_pipe_to_end(mut pipe: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = pipe.read_to_end(&mut buffer);
    buffer
}

fn join_stdout_reader(
    reader: Option<thread::JoinHandle<OpenCodeCapture>>,
) -> Result<OpenCodeCapture, AgentSessionError> {
    Ok(reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default())
}

fn join_codex_stdout_reader(
    reader: Option<thread::JoinHandle<CodexCapture>>,
) -> Result<CodexCapture, AgentSessionError> {
    Ok(reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default())
}

fn join_codebuddy_stdout_reader(
    reader: Option<thread::JoinHandle<CodeBuddyCapture>>,
) -> Result<CodeBuddyCapture, AgentSessionError> {
    Ok(reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default())
}

fn join_pipe_reader(
    reader: Option<thread::JoinHandle<Vec<u8>>>,
) -> Result<Vec<u8>, AgentSessionError> {
    Ok(reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default())
}

fn combine_command_output(stdout: &str, stderr: &str) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn tail_str(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        s.chars().skip(char_count - max).collect()
    }
}

fn write_codebuddy_mcp_config(
    request: &AgentTurnRequest,
) -> Result<Option<PathBuf>, AgentSessionError> {
    let Some(content) = request
        .codebuddy_mcp_config_content
        .as_ref()
        .filter(|content| !content.trim().is_empty())
    else {
        return Ok(None);
    };
    let artifacts_dir = store::session_dir(
        &request.workspace_root,
        &request.project,
        &request.session_id,
    )
    .join("artifacts");
    fs::create_dir_all(&artifacts_dir).map_err(|source| AgentSessionError::Io {
        path: artifacts_dir.clone(),
        source,
    })?;
    let path = artifacts_dir.join("codebuddy-mcp.json");
    fs::write(&path, content).map_err(|source| AgentSessionError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(Some(path))
}
