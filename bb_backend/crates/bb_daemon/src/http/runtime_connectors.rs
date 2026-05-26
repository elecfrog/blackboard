use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use bb_core::InboxError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::task_graph::runner_config::{
    patch_runner_config, read_runner_config_snapshot, RunnerConfigPatch, RunnerRuntimeCommand,
};
use super::{ApiError, AppState};

#[derive(Debug, Serialize)]
pub struct RuntimeConnectorList {
    config_path: String,
    node_timeout_secs: u64,
    run_timeout_secs: u64,
    runtimes: Vec<RuntimeConnector>,
    pi: PiRuntimeConnector,
}

#[derive(Debug, Serialize)]
pub struct RuntimeConnector {
    id: String,
    display_name: String,
    command: String,
    configured_command: Option<String>,
    fallback_command: String,
    resolved_program: String,
    resolved_args: Vec<String>,
    status: RuntimeConnectorStatus,
    version: Option<String>,
    error: Option<String>,
    max_concurrency: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConnectorStatus {
    Connected,
    Missing,
    CheckFailed,
}

#[derive(Debug, Serialize)]
pub struct PiRuntimeConnector {
    agent_dir: Option<String>,
    settings_path: Option<String>,
    settings_exists: bool,
    settings_error: Option<String>,
    configured_shell_path: Option<String>,
    effective_shell_path: Option<String>,
    shell_path_source: PiShellPathSource,
    shell_path_exists: bool,
    recommended_shell_path: Option<String>,
    default_provider: Option<String>,
    default_model: Option<String>,
    default_thinking_level: Option<String>,
    workaround_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PiShellPathSource {
    Settings,
    GitBashDefault,
    Path,
    Missing,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConnectorPatch {
    commands: BTreeMap<String, String>,
    runtime_max_concurrency: BTreeMap<String, u32>,
    node_timeout_secs: Option<u64>,
    run_timeout_secs: Option<u64>,
    pi: Option<PiRuntimeConnectorPatch>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PiRuntimeConnectorPatch {
    shell_path: Option<String>,
}

pub async fn list_runtime_connectors_handler(
    State(state): State<AppState>,
) -> Result<Json<RuntimeConnectorList>, ApiError> {
    let workspace_root = state.workspace_root()?;
    Ok(Json(runtime_connector_list(&workspace_root)))
}

pub async fn patch_runtime_connectors_handler(
    State(state): State<AppState>,
    input: Result<Json<RuntimeConnectorPatch>, JsonRejection>,
) -> Result<Json<RuntimeConnectorList>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid runtime connector body: {err}"
        )))
    })?;
    if let Some(pi) = &input.pi {
        if let Some(shell_path) = pi.shell_path.as_deref() {
            write_pi_shell_path(shell_path)?;
        }
    }
    let workspace_root = state.workspace_root()?;
    patch_runner_config(
        &workspace_root,
        RunnerConfigPatch {
            commands: input.commands,
            runtime_max_concurrency: input.runtime_max_concurrency,
            node_timeout_secs: input.node_timeout_secs,
            run_timeout_secs: input.run_timeout_secs,
        },
    )?;
    Ok(Json(runtime_connector_list(&workspace_root)))
}

fn runtime_connector_list(workspace_root: &std::path::Path) -> RuntimeConnectorList {
    let snapshot = read_runner_config_snapshot(workspace_root);
    RuntimeConnectorList {
        config_path: snapshot.config_path.display().to_string(),
        node_timeout_secs: snapshot.node_timeout_secs,
        run_timeout_secs: snapshot.run_timeout_secs,
        runtimes: snapshot
            .runtimes
            .iter()
            .map(inspect_runtime_connector)
            .collect(),
        pi: inspect_pi_runtime(),
    }
}

fn inspect_runtime_connector(runtime: &RunnerRuntimeCommand) -> RuntimeConnector {
    let (resolved_program, resolved_args) = resolve_runtime_command(&runtime.id, &runtime.command);
    let probe = probe_runtime(&runtime.id, &resolved_program, &resolved_args);
    RuntimeConnector {
        id: runtime.id.clone(),
        display_name: display_name(&runtime.id).to_string(),
        command: runtime.command.clone(),
        configured_command: runtime.configured_command.clone(),
        fallback_command: runtime.fallback_command.clone(),
        resolved_program,
        resolved_args,
        status: probe.status,
        version: probe.version,
        error: probe.error,
        max_concurrency: runtime.max_concurrency,
    }
}

fn resolve_runtime_command(id: &str, command: &str) -> (String, Vec<String>) {
    match id {
        "codex" => bb_core::platform::resolve_codex_spawn_command(command),
        "pi" => bb_core::platform::resolve_pi_spawn_command(command),
        _ => (
            bb_core::platform::resolve_spawn_program(command),
            Vec::new(),
        ),
    }
}

struct RuntimeProbe {
    status: RuntimeConnectorStatus,
    version: Option<String>,
    error: Option<String>,
}

fn probe_runtime(id: &str, program: &str, prefix_args: &[String]) -> RuntimeProbe {
    let version_arg = if id == "codebuddy" { "-v" } else { "--version" };
    let output = Command::new(program)
        .args(prefix_args)
        .arg(version_arg)
        .output();
    match output {
        Ok(output) if output.status.success() => RuntimeProbe {
            status: RuntimeConnectorStatus::Connected,
            version: first_non_empty_line(&String::from_utf8_lossy(&output.stdout))
                .or_else(|| first_non_empty_line(&String::from_utf8_lossy(&output.stderr))),
            error: None,
        },
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            RuntimeProbe {
                status: RuntimeConnectorStatus::CheckFailed,
                version: None,
                error: first_non_empty_line(&stderr)
                    .or_else(|| first_non_empty_line(&stdout))
                    .or_else(|| Some(format!("version probe exited with {}", output.status))),
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => RuntimeProbe {
            status: RuntimeConnectorStatus::Missing,
            version: None,
            error: Some(err.to_string()),
        },
        Err(err) => RuntimeProbe {
            status: RuntimeConnectorStatus::CheckFailed,
            version: None,
            error: Some(err.to_string()),
        },
    }
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn display_name(id: &str) -> &str {
    match id {
        "codex" => "Codex",
        "codebuddy" => "CodeBuddy",
        "opencode" => "OpenCode",
        "pi" => "Pi",
        _ => id,
    }
}

fn inspect_pi_runtime() -> PiRuntimeConnector {
    let agent_dir = bb_core::platform::user_home_dir().map(|home| home.join(".pi"));
    let settings_path = agent_dir.as_ref().map(|dir| dir.join("settings.json"));
    let recommended_shell_path = recommended_git_bash();
    let path_bash = path_bash();
    let (settings_exists, settings_error, settings) = read_pi_settings(settings_path.as_ref());
    let configured_shell_path = settings.as_ref().and_then(|value| {
        string_field(value, "shellPath").or_else(|| string_field(value, "shell_path"))
    });
    let default_provider = settings.as_ref().and_then(|value| {
        string_field(value, "defaultProvider").or_else(|| string_field(value, "default_provider"))
    });
    let default_model = settings.as_ref().and_then(|value| {
        string_field(value, "defaultModel").or_else(|| string_field(value, "default_model"))
    });
    let default_thinking_level = settings.as_ref().and_then(|value| {
        string_field(value, "defaultThinkingLevel")
            .or_else(|| string_field(value, "default_thinking_level"))
    });
    let (effective_shell_path, shell_path_source) =
        if configured_shell_path.as_deref().is_some_and(path_exists) {
            (configured_shell_path.clone(), PiShellPathSource::Settings)
        } else if recommended_shell_path.as_deref().is_some_and(path_exists) {
            (
                recommended_shell_path.clone(),
                PiShellPathSource::GitBashDefault,
            )
        } else if path_bash.as_deref().is_some_and(path_exists) {
            (path_bash.clone(), PiShellPathSource::Path)
        } else {
            (configured_shell_path.clone(), PiShellPathSource::Missing)
        };
    let shell_path_exists = effective_shell_path.as_deref().is_some_and(path_exists);
    let workaround_required = cfg!(windows)
        && !matches!(shell_path_source, PiShellPathSource::Settings)
        && recommended_shell_path.is_some();

    PiRuntimeConnector {
        agent_dir: agent_dir.map(|path| path.display().to_string()),
        settings_path: settings_path.map(|path| path.display().to_string()),
        settings_exists,
        settings_error,
        configured_shell_path,
        effective_shell_path,
        shell_path_source,
        shell_path_exists,
        recommended_shell_path,
        default_provider,
        default_model,
        default_thinking_level,
        workaround_required,
    }
}

fn read_pi_settings(path: Option<&PathBuf>) -> (bool, Option<String>, Option<Value>) {
    let Some(path) = path else {
        return (false, None, None);
    };
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(value) => (true, None, Some(value)),
            Err(err) => (true, Some(err.to_string()), None),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (false, None, None),
        Err(err) => (false, Some(err.to_string()), None),
    }
}

fn write_pi_shell_path(shell_path: &str) -> Result<(), ApiError> {
    let Some(home) = bb_core::platform::user_home_dir() else {
        return Ok(());
    };
    let dir = home.join(".pi");
    let path = dir.join("settings.json");
    std::fs::create_dir_all(&dir).map_err(|source| {
        ApiError(InboxError::Io {
            path: dir.clone(),
            source,
        })
    })?;
    let mut settings = read_pi_settings(Some(&path))
        .2
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    settings.insert(
        "shellPath".to_string(),
        Value::String(shell_path.trim().to_string()),
    );
    let content = serde_json::to_string_pretty(&Value::Object(settings)).map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "failed to serialize Pi settings: {err}"
        )))
    })?;
    std::fs::write(&path, format!("{content}\n")).map_err(|source| {
        ApiError(InboxError::Io {
            path: path.clone(),
            source,
        })
    })?;
    Ok(())
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn recommended_git_bash() -> Option<String> {
    let candidates = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];
    candidates
        .iter()
        .find(|candidate| path_exists(candidate))
        .map(|value| value.to_string())
}

fn path_bash() -> Option<String> {
    let output = if cfg!(windows) {
        Command::new("where").arg("bash.exe").output()
    } else {
        Command::new("which").arg("bash").output()
    }
    .ok()?;
    if !output.status.success() {
        return None;
    }
    first_non_empty_line(&String::from_utf8_lossy(&output.stdout))
}

fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}
