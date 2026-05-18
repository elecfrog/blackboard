//! Agent CLI installation and version checks.
//!
//! This module is intentionally separate from `agents_config`: config
//! connectors write Blackboard rules/MCP config, while agent tools inspect and
//! install the underlying CLI binaries.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::InboxError;

const OPENCODE_VERSION: &str = "1.15.0";
const PI_VERSION: &str = "0.75.4";

#[derive(Debug, Clone, Copy)]
struct AgentToolSpec {
    id: &'static str,
    display_name: &'static str,
    cli_name: &'static str,
    npm_package: &'static str,
    target_version: &'static str,
    install_arg: &'static str,
    install_flags: &'static [&'static str],
}

const AGENT_TOOL_SPECS: &[AgentToolSpec] = &[
    AgentToolSpec {
        id: "opencode",
        display_name: "OpenCode",
        cli_name: "opencode",
        npm_package: "opencode-ai",
        target_version: OPENCODE_VERSION,
        install_arg: "opencode-ai@1.15.0",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "codex",
        display_name: "Codex",
        cli_name: "codex",
        npm_package: "@openai/codex",
        target_version: "latest",
        install_arg: "@openai/codex@latest",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "codebuddy",
        display_name: "CodeBuddy",
        cli_name: "codebuddy",
        npm_package: "@tencent-ai/codebuddy-code",
        target_version: "latest",
        install_arg: "@tencent-ai/codebuddy-code@latest",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "pi",
        display_name: "Pi",
        cli_name: "pi",
        npm_package: "@earendil-works/pi-coding-agent",
        target_version: PI_VERSION,
        install_arg: "@earendil-works/pi-coding-agent@0.75.4",
        install_flags: &["--ignore-scripts"],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolStatus {
    Missing,
    Installed,
    VersionMismatch,
    NpmMissing,
    CheckFailed,
    ExternalInstall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTool {
    pub id: String,
    pub display_name: String,
    pub cli_name: String,
    pub npm_package: String,
    pub target_version: String,
    pub install_arg: String,
    pub install_command: String,
    pub status: AgentToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolList {
    pub tools: Vec<AgentTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolInstallResult {
    pub tool: AgentTool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
}

#[must_use]
pub fn list_agent_tools() -> AgentToolList {
    let npm_program = resolve_command_path("npm");
    let tools = AGENT_TOOL_SPECS
        .iter()
        .map(|spec| inspect_tool(spec, npm_program.as_deref()))
        .collect();
    AgentToolList { tools }
}

pub fn install_agent_tool(id: &str) -> Result<AgentToolInstallResult, InboxError> {
    let spec = find_tool_spec(id)?;
    let npm_program = resolve_command_path("npm").ok_or_else(|| {
        InboxError::InvalidInput("npm is not available; install Node.js/npm first".to_string())
    })?;
    let npm_spawn_program = crate::platform::resolve_spawn_program(&npm_program.to_string_lossy());
    let command = install_command_display(spec);
    let output = Command::new(&npm_spawn_program)
        .args(["install", "-g"])
        .args(spec.install_flags)
        .arg(spec.install_arg)
        .output()
        .map_err(|source| InboxError::Io {
            path: PathBuf::from(&npm_spawn_program),
            source,
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let tool = inspect_tool(spec, Some(&npm_program));
    Ok(AgentToolInstallResult {
        tool,
        command,
        exit_code: output.status.code(),
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    })
}

fn find_tool_spec(id: &str) -> Result<&'static AgentToolSpec, InboxError> {
    AGENT_TOOL_SPECS
        .iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| InboxError::InvalidInput(format!("unknown agent tool id: {id}")))
}

fn inspect_tool(spec: &AgentToolSpec, npm_program: Option<&Path>) -> AgentTool {
    let cli_path = resolve_command_path(spec.cli_name);
    let version_result = cli_path
        .as_ref()
        .map(|path| command_version(path))
        .transpose();
    let (current_version, version_error) = match version_result {
        Ok(version) => (version.flatten(), None),
        Err(err) => (None, Some(err)),
    };

    let npm_version = npm_program.and_then(|npm| npm_global_package_version(npm, spec.npm_package));
    let (status, last_error) = status_from_probe(
        spec,
        npm_program.is_some(),
        cli_path.is_some(),
        current_version.as_deref(),
        npm_version.as_deref(),
        version_error.as_deref(),
    );

    AgentTool {
        id: spec.id.to_string(),
        display_name: spec.display_name.to_string(),
        cli_name: spec.cli_name.to_string(),
        npm_package: spec.npm_package.to_string(),
        target_version: spec.target_version.to_string(),
        install_arg: spec.install_arg.to_string(),
        install_command: install_command_display(spec),
        status,
        cli_path: cli_path.map(|path| path.to_string_lossy().to_string()),
        current_version,
        npm_version,
        last_error,
    }
}

fn status_from_probe(
    spec: &AgentToolSpec,
    npm_available: bool,
    cli_present: bool,
    current_version: Option<&str>,
    npm_version: Option<&str>,
    check_error: Option<&str>,
) -> (AgentToolStatus, Option<String>) {
    if !npm_available {
        return (
            AgentToolStatus::NpmMissing,
            Some("npm is not available".to_string()),
        );
    }
    if !cli_present {
        if npm_version.is_some() {
            return (
                AgentToolStatus::CheckFailed,
                Some(format!(
                    "{} is installed globally but `{}` is not on PATH",
                    spec.npm_package, spec.cli_name
                )),
            );
        }
        return (AgentToolStatus::Missing, None);
    }
    if npm_version.is_none() {
        return (
            AgentToolStatus::ExternalInstall,
            check_error.map(str::to_string),
        );
    }
    if let Some(err) = check_error {
        return (AgentToolStatus::CheckFailed, Some(err.to_string()));
    }
    if current_version.is_none() {
        return (
            AgentToolStatus::CheckFailed,
            Some(format!("failed to parse `{}` version", spec.cli_name)),
        );
    }
    if spec.target_version != "latest" && current_version != Some(spec.target_version) {
        return (
            AgentToolStatus::VersionMismatch,
            Some(format!(
                "{} must be locked to {}, found {}",
                spec.display_name,
                spec.target_version,
                current_version.unwrap_or("unknown")
            )),
        );
    }
    (AgentToolStatus::Installed, None)
}

fn command_version(path: &Path) -> Result<Option<String>, String> {
    let program = crate::platform::resolve_spawn_program(&path.to_string_lossy());
    let output = Command::new(&program)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_version(&text))
}

fn npm_global_package_version(npm: &Path, package: &str) -> Option<String> {
    let program = crate::platform::resolve_spawn_program(&npm.to_string_lossy());
    let output = Command::new(program)
        .args(["ls", "-g", package, "--depth=0", "--json"])
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    let value: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    value
        .get("dependencies")
        .and_then(|deps| deps.get(package))
        .and_then(|pkg| pkg.get("version"))
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

fn install_command_display(spec: &AgentToolSpec) -> String {
    let mut parts = vec!["npm".to_string(), "install".to_string(), "-g".to_string()];
    parts.extend(
        spec.install_flags
            .iter()
            .map(std::string::ToString::to_string),
    );
    parts.push(spec.install_arg.to_string());
    parts.join(" ")
}

fn resolve_command_path(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 || command.contains('\\') || command.contains('/') {
        return path.is_file().then(|| path.to_path_buf());
    }

    let path_env = env::var_os("PATH")?;
    for dir in env::split_paths(&path_env) {
        for candidate in command_candidates(&dir, command) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn command_candidates(dir: &Path, command: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let path = Path::new(command);
        if path.extension().is_some() {
            return vec![dir.join(command)];
        }
        let mut exts = vec![".cmd".to_string(), ".exe".to_string(), ".com".to_string()];
        if let Ok(path_ext) = env::var("PATHEXT") {
            for ext in path_ext.split(';') {
                let normalized = normalize_windows_ext(ext);
                if !normalized.is_empty() && !exts.iter().any(|seen| seen == &normalized) {
                    exts.push(normalized);
                }
            }
        }
        exts.into_iter()
            .map(|ext| dir.join(format!("{command}{ext}")))
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![dir.join(command)]
    }
}

#[cfg(windows)]
fn normalize_windows_ext(ext: &str) -> String {
    let ext = ext.trim().trim_matches('"').to_ascii_lowercase();
    if ext.is_empty() {
        String::new()
    } else if ext.starts_with('.') {
        ext
    } else {
        format!(".{ext}")
    }
}

fn parse_version(text: &str) -> Option<String> {
    text.split_whitespace()
        .find_map(|token| {
            let token = token.trim_matches(|ch: char| {
                !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '+')
            });
            token
                .chars()
                .next()
                .filter(char::is_ascii_digit)
                .map(|_| token.to_string())
        })
        .filter(|token| !token.is_empty())
}

fn optional_tail(value: &str, max: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(tail_str(trimmed, max))
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

#[cfg(test)]
mod tests {
    use super::{find_tool_spec, status_from_probe, AgentToolStatus};

    #[test]
    fn codebuddy_uses_tencent_package_not_cli_name() {
        let spec = find_tool_spec("codebuddy").unwrap();
        assert_eq!(spec.cli_name, "codebuddy");
        assert_eq!(spec.npm_package, "@tencent-ai/codebuddy-code");
        assert_eq!(spec.install_arg, "@tencent-ai/codebuddy-code@latest");
    }

    #[test]
    fn pi_uses_earendil_package_and_ignores_scripts() {
        let spec = find_tool_spec("pi").unwrap();
        assert_eq!(spec.cli_name, "pi");
        assert_eq!(spec.npm_package, "@earendil-works/pi-coding-agent");
        assert_eq!(spec.install_arg, "@earendil-works/pi-coding-agent@0.75.4");
        assert_eq!(
            super::install_command_display(spec),
            "npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.75.4"
        );
    }

    #[test]
    fn unknown_tool_id_is_invalid() {
        let err = find_tool_spec("unknown").unwrap_err().to_string();
        assert!(err.contains("unknown agent tool id"));
    }

    #[test]
    fn opencode_mismatch_and_locked_version_statuses() {
        let spec = find_tool_spec("opencode").unwrap();
        let (status, _) = status_from_probe(spec, true, true, Some("1.15.1"), Some("1.15.1"), None);
        assert_eq!(status, AgentToolStatus::VersionMismatch);

        let (status, _) = status_from_probe(spec, true, true, Some("1.15.0"), Some("1.15.0"), None);
        assert_eq!(status, AgentToolStatus::Installed);
    }

    #[test]
    fn pi_mismatch_and_locked_version_statuses() {
        let spec = find_tool_spec("pi").unwrap();
        let (status, _) = status_from_probe(spec, true, true, Some("0.75.3"), Some("0.75.3"), None);
        assert_eq!(status, AgentToolStatus::VersionMismatch);

        let (status, _) = status_from_probe(spec, true, true, Some("0.75.4"), Some("0.75.4"), None);
        assert_eq!(status, AgentToolStatus::Installed);
    }

    #[test]
    fn external_install_when_cli_exists_without_global_package() {
        let spec = find_tool_spec("codex").unwrap();
        let (status, _) = status_from_probe(spec, true, true, Some("0.130.0"), None, None);
        assert_eq!(status, AgentToolStatus::ExternalInstall);
    }

    #[test]
    fn npm_missing_wins_over_other_probe_details() {
        let spec = find_tool_spec("codex").unwrap();
        let (status, _) = status_from_probe(spec, false, true, Some("0.130.0"), None, None);
        assert_eq!(status, AgentToolStatus::NpmMissing);
    }
}
