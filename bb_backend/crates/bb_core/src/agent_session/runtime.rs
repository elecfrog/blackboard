use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::model::{
    AgentEventType, AgentResult, AgentResultStatus, AgentSessionStatus, AgentSessionSummary,
    AgentToolPolicy, TokenUsage,
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
use super::providers::pi::{
    append_event as append_pi_event, read_pi_json_pipe_to_end, PiCapture, PiObserver,
};
use super::store::{self, FinalizeStats};
use super::AgentSessionError;
use crate::fs_util::{resolve_slash, write_file_atomic};

const PI_PROMPT_FILE_ARG_THRESHOLD: usize = 8_000;
const DEFAULT_AGENT_STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_AGENT_COMPLETED_EXIT_GRACE: Duration = Duration::from_secs(5);
const DEFAULT_AGENT_STDOUT_JOIN_GRACE: Duration = Duration::from_secs(20);
const AGENT_STARTUP_TIMEOUT_ENV: &str = "BB_AGENT_SESSION_STARTUP_TIMEOUT_SECONDS";
const AGENT_COMPLETED_EXIT_GRACE_ENV: &str = "BB_AGENT_SESSION_COMPLETED_EXIT_GRACE_SECONDS";
const AGENT_STDOUT_JOIN_GRACE_ENV: &str = "BB_AGENT_SESSION_STDOUT_JOIN_GRACE_SECONDS";
const PI_DEFAULT_BASH_DENY: &[&str] = &[
    r"\bgit\s+(checkout|switch|reset|clean|restore|revert)\b",
    r"\bcargo\s+clean\b",
    r"\brm\s+-rf\b",
    r"\bRemove-Item\b(?=.*\b-Recurse\b)(?=.*\b-Force\b)",
    r"\brd\s+/s\b",
    r"\brmdir\s+/s\b",
    r"\bdel\s+/s\b",
];

const PI_TOOL_POLICY_EXTENSION: &str = r#"
import fs from "node:fs";
import path from "node:path";

function loadPolicy() {
  const policyPath = process.env.BB_PI_TOOL_POLICY_PATH;
  if (!policyPath) return null;
  try {
    return JSON.parse(fs.readFileSync(policyPath, "utf8"));
  } catch (error) {
    return {
      mode: "block",
      allowed_tools: [],
      bash: { deny: [".*"] },
      _load_error: String(error && error.message ? error.message : error),
      _policy_path: policyPath,
    };
  }
}

function regexMatch(patterns, value) {
  if (!Array.isArray(patterns) || !value) return null;
  for (const pattern of patterns) {
    try {
      if (new RegExp(pattern, "i").test(value)) return pattern;
    } catch {
      if (value.includes(String(pattern))) return pattern;
    }
  }
  return null;
}

function wildcardToRegex(pattern) {
  const normalized = String(pattern).trim().replace(/\s+/g, " ");
  const escaped = normalized
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${escaped}$`, "i");
}

function wildcardMatch(patterns, value) {
  if (!Array.isArray(patterns) || !value) return null;
  const normalizedValue = String(value).trim().replace(/\s+/g, " ");
  for (const pattern of patterns) {
    try {
      if (wildcardToRegex(pattern).test(normalizedValue)) return pattern;
    } catch {
      if (normalizedValue.includes(String(pattern))) return pattern;
    }
  }
  return null;
}

function hasRules(patterns) {
  return Array.isArray(patterns) && patterns.length > 0;
}

function pathInput(input) {
  if (!input || typeof input !== "object") return null;
  const value = input.path ?? input.file ?? input.filename ?? input.target;
  return typeof value === "string" && value.trim() ? value : null;
}

function normalizeForCompare(value) {
  return process.platform === "win32" ? value.toLowerCase() : value;
}

function isInside(child, root) {
  const relative = path.relative(root, child);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function allowedByRoots(rawPath, roots) {
  if (!Array.isArray(roots) || roots.length === 0) return true;
  if (typeof rawPath !== "string" || !rawPath.trim()) return false;
  const cwd = process.cwd();
  const target = normalizeForCompare(path.resolve(cwd, rawPath));
  return roots.some((root) => {
    if (typeof root !== "string" || !root.trim()) return false;
    const resolvedRoot = normalizeForCompare(path.resolve(cwd, root));
    return isInside(target, resolvedRoot);
  });
}

function deniedByRoots(rawPath, roots) {
  if (!Array.isArray(roots) || roots.length === 0) return false;
  if (typeof rawPath !== "string" || !rawPath.trim()) return false;
  const cwd = process.cwd();
  const target = normalizeForCompare(path.resolve(cwd, rawPath));
  return roots.some((root) => {
    if (typeof root !== "string" || !root.trim()) return false;
    const resolvedRoot = normalizeForCompare(path.resolve(cwd, root));
    return isInside(target, resolvedRoot);
  });
}

function mcpTarget(input) {
  if (!input || typeof input !== "object") return "";
  const direct = input.tool ?? input.name ?? input.target;
  if (typeof direct === "string") return direct;
  return "";
}

function block(reason, policyLoadError) {
  const result = { block: true, reason: `Blocked by Blackboard Pi tool policy: ${reason}` };
  if (policyLoadError) result.policy_load_error = policyLoadError;
  return result;
}

export default function (pi) {
  const policy = loadPolicy();
  if (!policy || policy.mode === "off") return;

  pi.on("tool_call", async (event) => {
    if (policy._load_error) {
      return block(`failed to load policy: ${policy._load_error}`, policy._policy_path);
    }

    const toolName = event.toolName || event.name || "";
    const input = event.input || {};
    const allowedTools = policy.allowed_tools;
    if (Array.isArray(allowedTools) && !allowedTools.includes(toolName)) {
      return block(`tool '${toolName}' is not in allowed_tools`);
    }

    if (toolName === "bash") {
      const command = typeof input.command === "string" ? input.command : "";
      const denied = regexMatch(policy.bash && policy.bash.deny, command);
      if (denied) return block(`bash command matched deny rule '${denied}'`);
      const blacklisted = wildcardMatch(policy.bash && policy.bash.blacklist, command);
      if (blacklisted) return block(`bash command matched blacklist rule '${blacklisted}'`);

      const allowRules = policy.bash && policy.bash.allow;
      const whitelistRules = policy.bash && policy.bash.whitelist;
      const hasAllowRules = hasRules(allowRules) || hasRules(whitelistRules);
      if (
        hasAllowRules &&
        !regexMatch(allowRules, command) &&
        !wildcardMatch(whitelistRules, command)
      ) {
        return block("bash command did not match any allow/whitelist rule");
      }
    }

    if (toolName === "write" || toolName === "edit") {
      const targetPath = pathInput(input);
      if (deniedByRoots(targetPath, policy.write_deny_roots)) {
        return block(`${toolName} path is inside write_deny_roots`);
      }
      if (!allowedByRoots(targetPath, policy.write_roots)) {
        return block(`${toolName} path is outside write_roots`);
      }
    }

    if (toolName === "mcp") {
      const target = mcpTarget(input);
      const denied = regexMatch(policy.mcp && policy.mcp.deny, target);
      if (denied) return block(`mcp target matched deny rule '${denied}'`);
      const allowRules = policy.mcp && policy.mcp.allow;
      if (Array.isArray(allowRules) && !regexMatch(allowRules, target)) {
        return block(`mcp target '${target}' is not allowed`);
      }
    }
  });
}
"#;

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
    pub pi_path: String,
    pub pi_mcp_config_content: Option<String>,
    pub tool_policy: Option<AgentToolPolicy>,
    pub custom_env: BTreeMap<String, String>,
    pub custom_args: Vec<String>,
    pub timeout: Duration,
    pub startup_timeout: Option<Duration>,
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
        "pi" => run_pi_turn(request, cancel_check),
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

    let progress_check = agent_session_progress_check(&request);
    let (status, terminal_reason) = wait_with_timeout(
        &mut child,
        request.timeout,
        request.startup_timeout,
        &opencode_program,
        cancel_check,
        progress_check,
    )?;
    let mut capture = join_stdout_reader(stdout_reader, &request)?;
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
        TerminalReason::Timeout | TerminalReason::StartupTimeout => AgentResultStatus::Timeout,
        TerminalReason::CompletedEvent => {
            if capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
        TerminalReason::Exited => {
            if status.is_some_and(|status| status.success()) && capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let terminal_error = terminal_reason.error_message(request.startup_timeout);
    let error = result_error(
        result_status,
        status,
        capture
            .error_message
            .as_deref()
            .or(terminal_error.as_deref()),
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
    let (codex_program, codex_prefix_args) =
        crate::platform::resolve_codex_spawn_command(&request.codex_path);
    let mut cmd = Command::new(&codex_program);
    cmd.args(&codex_prefix_args);
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
        .map(|stdout| thread::spawn(move || read_codex_json_pipe_to_end(stdout, &observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let progress_check = agent_session_progress_check(&request);
    let (status, terminal_reason) = wait_with_timeout(
        &mut child,
        request.timeout,
        request.startup_timeout,
        &codex_program,
        cancel_check,
        progress_check,
    )?;
    let mut capture = join_codex_stdout_reader(stdout_reader, &request)?;
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
        TerminalReason::Timeout | TerminalReason::StartupTimeout => AgentResultStatus::Timeout,
        TerminalReason::CompletedEvent => {
            if capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
        TerminalReason::Exited => {
            if status.is_some_and(|status| status.success()) && capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let terminal_error = terminal_reason.error_message(request.startup_timeout);
    let error = result_error(
        result_status,
        status,
        capture
            .error_message
            .as_deref()
            .or(terminal_error.as_deref()),
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
        .map(|stdout| thread::spawn(move || read_codebuddy_json_pipe_to_end(stdout, &observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let progress_check = agent_session_progress_check(&request);
    let (status, terminal_reason) = wait_with_timeout(
        &mut child,
        request.timeout,
        request.startup_timeout,
        &codebuddy_program,
        cancel_check,
        progress_check,
    )?;
    let mut capture = join_codebuddy_stdout_reader(stdout_reader, &request)?;
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
        TerminalReason::Timeout | TerminalReason::StartupTimeout => AgentResultStatus::Timeout,
        TerminalReason::CompletedEvent => {
            if capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
        TerminalReason::Exited => {
            if status.is_some_and(|status| status.success()) && capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let terminal_error = terminal_reason.error_message(request.startup_timeout);
    let error = result_error(
        result_status,
        status,
        capture
            .error_message
            .as_deref()
            .or(terminal_error.as_deref()),
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

fn run_pi_turn<F>(
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

    let mcp_config_path = write_pi_mcp_config(&request)?;
    let tool_policy =
        effective_pi_tool_policy(&request.workspace_root, request.tool_policy.clone());
    let tool_policy_files = write_pi_tool_policy_files(&request, tool_policy.as_ref())?;

    let start = Instant::now();
    let (pi_program, pi_prefix_args) = crate::platform::resolve_pi_spawn_command(&request.pi_path);
    let mut cmd = Command::new(&pi_program);
    cmd.args(&pi_prefix_args);
    cmd.arg("--mode").arg("json").arg("--print");
    if let Some(session_id) = request
        .continue_provider_session_id
        .as_ref()
        .filter(|session_id| !session_id.is_empty())
    {
        cmd.arg("--session").arg(session_id);
    }
    if let Some(model) = request.model.as_ref().filter(|model| !model.is_empty()) {
        cmd.arg("--model").arg(model);
    }
    if let Some(thinking) = request
        .variant
        .as_deref()
        .filter(|value| pi_thinking_level(value).is_some())
    {
        cmd.arg("--thinking").arg(thinking);
    }
    let prompt_arg = pi_prompt_arg(&request)?;

    cmd.current_dir(&request.execution_root);
    append_pi_tool_policy_args(
        &mut cmd,
        tool_policy.as_ref(),
        tool_policy_files.as_ref(),
        &request.custom_args,
    );
    if let Some(path) = mcp_config_path.as_ref() {
        cmd.arg("--mcp-config").arg(resolve_slash(path));
    }
    cmd.args(&request.custom_args);
    cmd.arg(prompt_arg);

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
    if let Some(files) = tool_policy_files.as_ref() {
        cmd.env("BB_PI_TOOL_POLICY_PATH", &files.policy_path);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| AgentSessionError::Spawn {
        program: pi_program.clone(),
        source,
    })?;

    let observer = PiObserver {
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
        .map(|stdout| thread::spawn(move || read_pi_json_pipe_to_end(stdout, observer)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_pipe_to_end(stderr)));

    let progress_check = agent_session_progress_check(&request);
    let (status, terminal_reason) = wait_with_timeout(
        &mut child,
        request.timeout,
        request.startup_timeout,
        &pi_program,
        cancel_check,
        progress_check,
    )?;
    let mut capture = join_pi_stdout_reader(stdout_reader, &request)?;
    let stderr = join_pipe_reader(stderr_reader)?;
    let stderr_text = String::from_utf8_lossy(&stderr).to_string();
    if !stderr_text.trim().is_empty() {
        let content = format!("stderr:\n{}", tail_str(stderr_text.trim(), 2048));
        let observer = PiObserver {
            workspace_root: request.workspace_root.clone(),
            project: request.project.clone(),
            session_id: request.session_id.clone(),
            model_key: request
                .model
                .clone()
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
        };
        append_pi_event(
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
        TerminalReason::Timeout | TerminalReason::StartupTimeout => AgentResultStatus::Timeout,
        TerminalReason::CompletedEvent => {
            if capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
        TerminalReason::Exited => {
            if status.is_some_and(|status| status.success()) && capture.error_message.is_none() {
                AgentResultStatus::Completed
            } else {
                AgentResultStatus::Failed
            }
        }
    };
    let terminal_error = terminal_reason.error_message(request.startup_timeout);
    let error = result_error(
        result_status,
        status,
        capture
            .error_message
            .as_deref()
            .or(terminal_error.as_deref()),
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

#[derive(Debug, Clone)]
struct PiToolPolicyFiles {
    policy_path: PathBuf,
    extension_path: PathBuf,
}

fn effective_pi_tool_policy(
    workspace_root: &Path,
    policy: Option<AgentToolPolicy>,
) -> Option<AgentToolPolicy> {
    let mut policy = policy.unwrap_or_default();
    if policy.is_off() {
        return None;
    }

    let managed_projects_root = resolve_slash(workspace_root.join("projects"));
    if !policy
        .write_deny_roots
        .iter()
        .any(|root| root == &managed_projects_root)
    {
        policy.write_deny_roots.push(managed_projects_root);
    }

    for pattern in PI_DEFAULT_BASH_DENY {
        if !policy.bash.deny.iter().any(|item| item == pattern) {
            policy.bash.deny.push((*pattern).to_string());
        }
    }
    Some(policy)
}

fn append_pi_tool_policy_args(
    cmd: &mut Command,
    policy: Option<&AgentToolPolicy>,
    files: Option<&PiToolPolicyFiles>,
    custom_args: &[String],
) {
    let Some(policy) = policy else {
        return;
    };
    if let Some(allowed_tools) = policy.allowed_tools.as_ref() {
        if !pi_custom_args_define_tool_set(custom_args) {
            if allowed_tools.is_empty() {
                cmd.arg("--no-tools");
            } else {
                cmd.arg("--tools").arg(allowed_tools.join(","));
            }
        }
    }
    if let Some(files) = files {
        cmd.arg("--extension")
            .arg(crate::fs_util::resolve_slash(&files.extension_path));
    }
}

fn pi_custom_args_define_tool_set(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(arg.as_str(), "--tools" | "-t" | "--no-tools" | "-nt")
            || arg.starts_with("--tools=")
    })
}

fn write_pi_tool_policy_files(
    request: &AgentTurnRequest,
    policy: Option<&AgentToolPolicy>,
) -> Result<Option<PiToolPolicyFiles>, AgentSessionError> {
    let Some(policy) = policy else {
        return Ok(None);
    };
    let artifacts_dir = store::session_dir(
        &request.workspace_root,
        &request.project,
        &request.session_id,
    )
    .join("artifacts")
    .join("pi-tool-policy");
    fs::create_dir_all(&artifacts_dir).map_err(|source| AgentSessionError::Io {
        path: artifacts_dir.clone(),
        source,
    })?;

    let policy_path = artifacts_dir.join("policy.json");
    let extension_path = artifacts_dir.join("bb-pi-tool-policy.ts");
    let policy_json =
        serde_json::to_string_pretty(policy).map_err(|err| AgentSessionError::Parse {
            path: policy_path.clone(),
            message: err.to_string(),
        })?;
    write_file_atomic(&policy_path, &policy_json).map_err(|err| match err {
        crate::InboxError::Io { path, source } => AgentSessionError::Io { path, source },
        other => AgentSessionError::IoMessage {
            path: policy_path.clone(),
            message: other.to_string(),
        },
    })?;
    write_file_atomic(&extension_path, PI_TOOL_POLICY_EXTENSION).map_err(|err| match err {
        crate::InboxError::Io { path, source } => AgentSessionError::Io { path, source },
        other => AgentSessionError::IoMessage {
            path: extension_path.clone(),
            message: other.to_string(),
        },
    })?;

    Ok(Some(PiToolPolicyFiles {
        policy_path,
        extension_path,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalReason {
    Exited,
    CompletedEvent,
    Timeout,
    StartupTimeout,
    Cancelled,
}

impl TerminalReason {
    fn error_message(self, startup_timeout: Option<Duration>) -> Option<String> {
        match self {
            Self::StartupTimeout => Some(format!(
                "agent session produced no events within {} ms during startup",
                startup_timeout.unwrap_or_default().as_millis()
            )),
            Self::Timeout => Some("agent session timed out".to_string()),
            Self::Exited | Self::CompletedEvent | Self::Cancelled => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AgentSessionProgress {
    has_activity: bool,
    completed: bool,
}

fn wait_with_timeout<F, P>(
    child: &mut Child,
    timeout: Duration,
    startup_timeout: Option<Duration>,
    program: &str,
    cancel_check: F,
    mut progress_check: P,
) -> Result<(Option<ExitStatus>, TerminalReason), AgentSessionError>
where
    F: Fn() -> bool,
    P: FnMut() -> AgentSessionProgress,
{
    let started_at = Instant::now();
    let deadline = started_at + timeout;
    let startup_deadline = startup_timeout.map(|startup| started_at + startup.min(timeout));
    let completed_exit_grace = effective_agent_completed_exit_grace();
    let activity_poll_interval = Duration::from_millis(500);
    let mut next_activity_check = started_at;
    let mut has_activity = startup_timeout.is_none_or(|startup| startup.is_zero());
    let mut completed_at: Option<Instant> = None;
    loop {
        if cancel_check() {
            let _ = crate::platform::terminate_child_process_tree(child);
            let status = child.wait().ok();
            return Ok((status, TerminalReason::Cancelled));
        }
        if let Some(status) = child.try_wait().map_err(|source| AgentSessionError::Io {
            path: PathBuf::from(program),
            source,
        })? {
            return Ok((Some(status), TerminalReason::Exited));
        }
        let now = Instant::now();
        if completed_at.is_none() && now >= next_activity_check {
            let progress = progress_check();
            has_activity = has_activity || progress.has_activity;
            if progress.completed {
                completed_at = Some(now);
            }
            next_activity_check = now + activity_poll_interval;
        }
        if completed_at
            .is_some_and(|completed_at| now.duration_since(completed_at) >= completed_exit_grace)
        {
            let _ = crate::platform::terminate_child_process_tree(child);
            let status = child.wait().ok();
            return Ok((status, TerminalReason::CompletedEvent));
        }
        if !has_activity && startup_deadline.is_some_and(|deadline| now >= deadline) {
            let _ = crate::platform::terminate_child_process_tree(child);
            let status = child.wait().ok();
            return Ok((status, TerminalReason::StartupTimeout));
        }
        if now >= deadline {
            let _ = crate::platform::terminate_child_process_tree(child);
            let status = child.wait().ok();
            return Ok((status, TerminalReason::Timeout));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn agent_session_progress_check(
    request: &AgentTurnRequest,
) -> impl FnMut() -> AgentSessionProgress {
    let workspace_root = request.workspace_root.clone();
    let project = request.project.clone();
    let session_id = request.session_id.clone();
    move || {
        store::read_events(&workspace_root, &project, &session_id, None)
            .map(|events| AgentSessionProgress {
                has_activity: !events.is_empty(),
                completed: events.iter().any(|event| {
                    event.event_type == AgentEventType::Status
                        && event.status.as_deref() == Some("completed")
                }),
            })
            .unwrap_or_default()
    }
}

pub fn effective_agent_startup_timeout(node_timeout: Duration) -> Option<Duration> {
    let startup_timeout = std::env::var(AGENT_STARTUP_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_AGENT_STARTUP_TIMEOUT);
    if startup_timeout.is_zero() {
        None
    } else {
        Some(startup_timeout.min(node_timeout))
    }
}

fn effective_agent_completed_exit_grace() -> Duration {
    std::env::var(AGENT_COMPLETED_EXIT_GRACE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_AGENT_COMPLETED_EXIT_GRACE)
}

fn effective_agent_stdout_join_grace() -> Duration {
    std::env::var(AGENT_STDOUT_JOIN_GRACE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_AGENT_STDOUT_JOIN_GRACE)
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
    request: &AgentTurnRequest,
) -> Result<OpenCodeCapture, AgentSessionError> {
    Ok(join_capture_reader(reader).unwrap_or_else(|| opencode_capture_from_events(request)))
}

fn join_codex_stdout_reader(
    reader: Option<thread::JoinHandle<CodexCapture>>,
    request: &AgentTurnRequest,
) -> Result<CodexCapture, AgentSessionError> {
    Ok(join_capture_reader(reader).unwrap_or_else(|| codex_capture_from_events(request)))
}

fn join_codebuddy_stdout_reader(
    reader: Option<thread::JoinHandle<CodeBuddyCapture>>,
    request: &AgentTurnRequest,
) -> Result<CodeBuddyCapture, AgentSessionError> {
    Ok(join_capture_reader(reader).unwrap_or_else(|| codebuddy_capture_from_events(request)))
}

fn join_pi_stdout_reader(
    reader: Option<thread::JoinHandle<PiCapture>>,
    request: &AgentTurnRequest,
) -> Result<PiCapture, AgentSessionError> {
    Ok(join_capture_reader(reader).unwrap_or_else(|| pi_capture_from_events(request)))
}

fn join_pipe_reader(
    reader: Option<thread::JoinHandle<Vec<u8>>>,
) -> Result<Vec<u8>, AgentSessionError> {
    Ok(join_capture_reader(reader).unwrap_or_default())
}

fn join_capture_reader<T: Default>(reader: Option<thread::JoinHandle<T>>) -> Option<T> {
    let Some(handle) = reader else {
        return Some(T::default());
    };
    let join_grace = effective_agent_stdout_join_grace();
    let deadline = Instant::now() + join_grace;
    loop {
        if handle.is_finished() {
            return Some(handle.join().unwrap_or_default());
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Debug, Clone, Default)]
struct EventCaptureParts {
    text_output: String,
    log: String,
    error_message: Option<String>,
    provider_session_id: Option<String>,
    usage: BTreeMap<String, TokenUsage>,
    event_count: u64,
    tool_count: u64,
    next_seq: u64,
}

fn event_capture_parts_from_store(request: &AgentTurnRequest) -> EventCaptureParts {
    let Ok(events) = store::read_events(
        &request.workspace_root,
        &request.project,
        &request.session_id,
        None,
    ) else {
        return EventCaptureParts::default();
    };

    let mut text = Vec::new();
    let mut log = Vec::new();
    let mut error_message = None;
    let mut provider_session_id = None;
    let mut usage = BTreeMap::<String, TokenUsage>::new();
    let mut tool_count = 0;
    let mut max_seq = 0;

    for event in &events {
        max_seq = max_seq.max(event.seq);
        if provider_session_id.is_none() {
            provider_session_id = event.session_id.clone();
        }
        for (model, item) in &event.usage {
            usage.entry(model.clone()).or_default().add_assign(item);
        }
        match event.event_type {
            AgentEventType::Text => {
                if let Some(content) = event
                    .content
                    .as_deref()
                    .map(str::trim)
                    .filter(|content| !content.is_empty())
                {
                    text.push(content.to_string());
                }
            }
            AgentEventType::ToolUse => {
                tool_count += 1;
            }
            AgentEventType::Error => {
                if let Some(content) = event
                    .content
                    .as_deref()
                    .map(str::trim)
                    .filter(|content| !content.is_empty())
                {
                    if error_message.is_none() {
                        error_message = Some(content.to_string());
                    }
                    log.push(content.to_string());
                }
            }
            AgentEventType::Log => {
                if let Some(content) = event
                    .content
                    .as_deref()
                    .map(str::trim)
                    .filter(|content| !content.is_empty())
                {
                    log.push(content.to_string());
                }
            }
            AgentEventType::Status
            | AgentEventType::Thinking
            | AgentEventType::ToolResult
            | AgentEventType::UsageUpdate => {}
        }
    }

    EventCaptureParts {
        text_output: text.join("\n\n"),
        log: log.join("\n"),
        error_message,
        provider_session_id,
        usage,
        event_count: events.len() as u64,
        tool_count,
        next_seq: max_seq.saturating_add(1).max(1),
    }
}

fn opencode_capture_from_events(request: &AgentTurnRequest) -> OpenCodeCapture {
    let parts = event_capture_parts_from_store(request);
    OpenCodeCapture {
        text_output: parts.text_output,
        log: parts.log,
        error_message: parts.error_message,
        provider_session_id: parts.provider_session_id,
        usage: parts.usage,
        event_count: parts.event_count,
        tool_count: parts.tool_count,
        next_seq: parts.next_seq,
        ..Default::default()
    }
}

fn codex_capture_from_events(request: &AgentTurnRequest) -> CodexCapture {
    let parts = event_capture_parts_from_store(request);
    CodexCapture {
        text_output: parts.text_output,
        log: parts.log,
        error_message: parts.error_message,
        provider_session_id: parts.provider_session_id,
        usage: parts.usage,
        event_count: parts.event_count,
        tool_count: parts.tool_count,
        next_seq: parts.next_seq,
        ..Default::default()
    }
}

fn codebuddy_capture_from_events(request: &AgentTurnRequest) -> CodeBuddyCapture {
    let parts = event_capture_parts_from_store(request);
    CodeBuddyCapture {
        text_output: parts.text_output,
        log: parts.log,
        error_message: parts.error_message,
        provider_session_id: parts.provider_session_id,
        usage: parts.usage,
        event_count: parts.event_count,
        tool_count: parts.tool_count,
        next_seq: parts.next_seq,
        ..Default::default()
    }
}

fn pi_capture_from_events(request: &AgentTurnRequest) -> PiCapture {
    let parts = event_capture_parts_from_store(request);
    let mut capture = PiCapture::default();
    capture.text_output = parts.text_output;
    capture.log = parts.log;
    capture.error_message = parts.error_message;
    capture.provider_session_id = parts.provider_session_id;
    capture.usage = parts.usage;
    capture.event_count = parts.event_count;
    capture.tool_count = parts.tool_count;
    capture.next_seq = parts.next_seq;
    capture
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

fn pi_thinking_level(value: &str) -> Option<&str> {
    match value {
        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" => Some(value),
        _ => None,
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

fn write_pi_mcp_config(request: &AgentTurnRequest) -> Result<Option<PathBuf>, AgentSessionError> {
    let Some(content) = request
        .pi_mcp_config_content
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
    .join("artifacts")
    .join("pi-mcp");
    fs::create_dir_all(&artifacts_dir).map_err(|source| AgentSessionError::Io {
        path: artifacts_dir.clone(),
        source,
    })?;
    let path = artifacts_dir.join("mcp.json");
    if fs::read_to_string(&path).is_ok_and(|existing| existing == *content) {
        return Ok(Some(path));
    }

    if let Err(err) = write_file_atomic(&path, content) {
        if matches!(
            &err,
            crate::InboxError::Io { source, .. } if source.kind() == ErrorKind::PermissionDenied
        ) && fs::read_to_string(&path).is_ok_and(|existing| existing == *content)
        {
            return Ok(Some(path));
        }

        return Err(match err {
            crate::InboxError::Io { path, source } => AgentSessionError::Io { path, source },
            other => AgentSessionError::IoMessage {
                path: path.clone(),
                message: other.to_string(),
            },
        });
    }
    Ok(Some(path))
}

fn pi_prompt_arg(request: &AgentTurnRequest) -> Result<String, AgentSessionError> {
    if request.prompt.chars().count() <= PI_PROMPT_FILE_ARG_THRESHOLD {
        return Ok(request.prompt.clone());
    }

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
    let path = artifacts_dir.join("prompt.md");
    write_file_atomic(&path, &request.prompt).map_err(|err| match err {
        crate::InboxError::Io { path, source } => AgentSessionError::Io { path, source },
        other => AgentSessionError::IoMessage {
            path: path.clone(),
            message: other.to_string(),
        },
    })?;
    Ok(format!("@{}", resolve_slash(&path)))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::TempDir;

    use crate::agent_session::model::AgentBashToolPolicy;

    use super::*;

    fn request_with_prompt(root: &std::path::Path, prompt: String) -> AgentTurnRequest {
        AgentTurnRequest {
            workspace_root: root.to_path_buf(),
            scripts_dir: root.join("scripts"),
            execution_root: root.to_path_buf(),
            project: "demo".to_string(),
            session_id: "as-test".to_string(),
            runtime: "pi".to_string(),
            agent: "native".to_string(),
            model: None,
            variant: None,
            continue_provider_session_id: None,
            prompt,
            codex_path: "codex".to_string(),
            codex_config_args: vec![],
            codebuddy_path: "codebuddy".to_string(),
            codebuddy_mcp_config_content: None,
            codebuddy_settings_json: None,
            opencode_path: "opencode".to_string(),
            opencode_config_content: None,
            pi_path: "pi".to_string(),
            pi_mcp_config_content: None,
            tool_policy: None,
            custom_env: BTreeMap::new(),
            custom_args: vec![],
            timeout: Duration::from_secs(1),
            startup_timeout: Some(Duration::from_secs(1)),
        }
    }

    #[test]
    fn pi_prompt_arg_keeps_short_prompt_inline() {
        let tmp = TempDir::new().unwrap();
        let request = request_with_prompt(tmp.path(), "short prompt".to_string());

        let arg = pi_prompt_arg(&request).unwrap();

        assert_eq!(arg, "short prompt");
    }

    #[test]
    fn pi_prompt_arg_writes_long_prompt_to_session_artifact() {
        let tmp = TempDir::new().unwrap();
        let prompt = "x".repeat(PI_PROMPT_FILE_ARG_THRESHOLD + 1);
        let request = request_with_prompt(tmp.path(), prompt.clone());

        let arg = pi_prompt_arg(&request).unwrap();

        assert!(arg.starts_with('@'));
        let path = std::path::PathBuf::from(arg.trim_start_matches('@'));
        assert!(path.ends_with("runtime/agent_sessions/demo/as-test/artifacts/prompt.md"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), prompt);
    }

    #[test]
    fn pi_mcp_config_writes_session_scoped_artifact() {
        let tmp = TempDir::new().unwrap();
        let mut request = request_with_prompt(tmp.path(), "prompt".to_string());
        request.pi_mcp_config_content = Some(r#"{"mcpServers":{"bb":{}}}"#.to_string());

        let path = write_pi_mcp_config(&request).unwrap().unwrap();

        assert!(path.ends_with("runtime/agent_sessions/demo/as-test/artifacts/pi-mcp/mcp.json"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"mcpServers":{"bb":{}}}"#
        );
        assert!(!tmp.path().join(".pi/mcp.json").exists());
    }

    #[test]
    fn pi_mcp_config_skips_rewriting_identical_session_artifact() {
        let tmp = TempDir::new().unwrap();
        let mut request = request_with_prompt(tmp.path(), "prompt".to_string());
        let content = r#"{"mcpServers":{"bb":{}}}"#.to_string();
        request.pi_mcp_config_content = Some(content.clone());

        let path = store::session_dir(
            &request.workspace_root,
            &request.project,
            &request.session_id,
        )
        .join("artifacts")
        .join("pi-mcp")
        .join("mcp.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &content).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();

        let actual = write_pi_mcp_config(&request).unwrap().unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        #[cfg(windows)]
        {
            #[allow(clippy::permissions_set_readonly_false)]
            {
                let mut perms = std::fs::metadata(&path).unwrap().permissions();
                perms.set_readonly(false);
                std::fs::set_permissions(&path, perms).unwrap();
            }
        }

        assert_eq!(actual, path);
    }

    #[test]
    fn effective_pi_tool_policy_injects_default_destructive_bash_denies() {
        let tmp = TempDir::new().unwrap();
        let policy = effective_pi_tool_policy(tmp.path(), None).unwrap();

        assert!(policy.allowed_tools.is_none());
        assert!(policy
            .write_deny_roots
            .iter()
            .any(|root| root.ends_with("/projects")));
        assert!(policy
            .bash
            .deny
            .iter()
            .any(|pattern| pattern.contains("git") && pattern.contains("checkout")));
        assert!(policy
            .bash
            .deny
            .iter()
            .any(|pattern| pattern.contains("cargo") && pattern.contains("clean")));
    }

    #[test]
    fn pi_tool_policy_writes_policy_and_extension_files() {
        let tmp = TempDir::new().unwrap();
        let mut request = request_with_prompt(tmp.path(), "prompt".to_string());
        request.tool_policy = Some(AgentToolPolicy {
            allowed_tools: Some(vec!["read".to_string(), "grep".to_string()]),
            write_roots: vec![tmp.path().join("src").display().to_string()],
            write_deny_roots: Vec::new(),
            bash: AgentBashToolPolicy {
                whitelist: vec!["cargo clippy*".to_string()],
                blacklist: vec!["*cargo clean*".to_string()],
                ..Default::default()
            },
            ..Default::default()
        });
        let policy = effective_pi_tool_policy(&request.workspace_root, request.tool_policy.clone());

        let files = write_pi_tool_policy_files(&request, policy.as_ref())
            .unwrap()
            .unwrap();

        let policy_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(files.policy_path).unwrap()).unwrap();
        assert_eq!(policy_json["allowed_tools"][0], "read");
        assert!(policy_json["write_deny_roots"][0]
            .as_str()
            .unwrap_or_default()
            .ends_with("/projects"));
        assert!(policy_json["bash"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap_or_default().contains("git")));
        assert_eq!(policy_json["bash"]["whitelist"][0], "cargo clippy*");
        assert_eq!(policy_json["bash"]["blacklist"][0], "*cargo clean*");
        let extension = std::fs::read_to_string(files.extension_path).unwrap();
        assert!(extension.contains("tool_call"));
        assert!(extension.contains("wildcardMatch"));
        assert!(extension.contains("Blocked by Blackboard Pi tool policy"));
    }

    #[test]
    fn pi_custom_args_define_tool_set_detects_allowlist_and_no_tools() {
        assert!(pi_custom_args_define_tool_set(&["--tools".to_string()]));
        assert!(pi_custom_args_define_tool_set(
            &["--tools=read".to_string()]
        ));
        assert!(pi_custom_args_define_tool_set(&["-nt".to_string()]));
        assert!(!pi_custom_args_define_tool_set(&["--model".to_string()]));
    }

    #[test]
    fn bash_policy_accepts_wildcard_list_aliases() {
        let policy: AgentToolPolicy = serde_json::from_value(serde_json::json!({
            "bash": {
                "white_list": ["cargo clippy*"],
                "black_list": ["*cargo clean*"]
            }
        }))
        .unwrap();

        assert_eq!(policy.bash.whitelist, vec!["cargo clippy*"]);
        assert_eq!(policy.bash.blacklist, vec!["*cargo clean*"]);
    }
}
