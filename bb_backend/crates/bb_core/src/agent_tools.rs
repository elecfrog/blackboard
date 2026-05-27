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
const PI_MCP_ADAPTER_PACKAGE: &str = "npm:pi-mcp-adapter";

#[derive(Debug, Clone, Copy)]
struct AgentToolSpec {
    id: &'static str,
    display_name: &'static str,
    cli_name: &'static str,
    npm_package: &'static str,
    brew_formula: Option<&'static str>,
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
        brew_formula: Some("opencode"),
        target_version: OPENCODE_VERSION,
        install_arg: "opencode-ai@1.15.0",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "codex",
        display_name: "Codex",
        cli_name: "codex",
        npm_package: "@openai/codex",
        brew_formula: None,
        target_version: "latest",
        install_arg: "@openai/codex@latest",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "codebuddy",
        display_name: "CodeBuddy",
        cli_name: "codebuddy",
        npm_package: "@tencent-ai/codebuddy-code",
        brew_formula: None,
        target_version: "latest",
        install_arg: "@tencent-ai/codebuddy-code@latest",
        install_flags: &[],
    },
    AgentToolSpec {
        id: "pi",
        display_name: "Pi",
        cli_name: "pi",
        npm_package: "@earendil-works/pi-coding-agent",
        brew_formula: None,
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
    Incomplete,
    VersionMismatch,
    NpmMissing,
    CheckFailed,
    ExternalInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolInstallSource {
    Npm,
    Brew,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolComponent {
    pub id: String,
    pub display_name: String,
    pub status: AgentToolStatus,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
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
    pub install_source: Option<AgentToolInstallSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<AgentToolComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolList {
    pub tools: Vec<AgentTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolInstallResult {
    pub tool: AgentTool,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<AgentToolInstallStepResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolInstallStepResult {
    pub id: String,
    pub label: String,
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
    if let Some(cli_path) = resolve_command_path(spec.cli_name) {
        if detect_brew_install(&cli_path, spec).is_some() {
            let current_version = command_version(&cli_path).ok().flatten();
            if should_apply_locked_version_with_npm(spec, current_version.as_deref()) {
                return install_npm_tool(spec);
            }
            return install_brew_tool(spec);
        }
    }
    if spec.id == "pi" {
        return install_pi_tool(spec);
    }
    install_npm_tool(spec)
}

fn install_npm_tool(spec: &AgentToolSpec) -> Result<AgentToolInstallResult, InboxError> {
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
        steps: vec![],
        exit_code: output.status.code(),
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    })
}

fn should_apply_locked_version_with_npm(
    spec: &AgentToolSpec,
    current_version: Option<&str>,
) -> bool {
    spec.target_version != "latest" && current_version != Some(spec.target_version)
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
    let install_source = detect_install_source(cli_path.as_deref(), npm_version.as_deref(), spec);
    let (status, last_error) = status_from_probe(
        spec,
        npm_program.is_some(),
        cli_path.is_some(),
        current_version.as_deref(),
        npm_version.as_deref(),
        install_source,
        version_error.as_deref(),
    );
    let components = if spec.id == "pi" {
        pi_components(
            status,
            current_version.as_deref(),
            last_error.as_deref(),
            cli_path.as_deref(),
        )
    } else {
        Vec::new()
    };
    let (status, last_error) = if spec.id == "pi" {
        pi_aggregate_status(status, last_error, &components)
    } else {
        (status, last_error)
    };

    AgentTool {
        id: spec.id.to_string(),
        display_name: spec.display_name.to_string(),
        cli_name: spec.cli_name.to_string(),
        npm_package: spec.npm_package.to_string(),
        target_version: spec.target_version.to_string(),
        install_arg: spec.install_arg.to_string(),
        install_command: tool_install_command_display(spec, install_source),
        status,
        install_source,
        cli_path: cli_path.map(|path| path.to_string_lossy().to_string()),
        current_version,
        npm_version,
        last_error,
        components,
    }
}

fn install_brew_tool(spec: &AgentToolSpec) -> Result<AgentToolInstallResult, InboxError> {
    let formula = spec.brew_formula.ok_or_else(|| {
        InboxError::InvalidInput(format!(
            "{} does not support Homebrew updates",
            spec.display_name
        ))
    })?;
    let brew_program = resolve_command_path("brew").ok_or_else(|| {
        InboxError::InvalidInput(
            "brew is not available; update this tool with its original installer".to_string(),
        )
    })?;
    let brew_spawn_program =
        crate::platform::resolve_spawn_program(&brew_program.to_string_lossy());
    let command = brew_update_command_display(formula);
    let output = Command::new(&brew_spawn_program)
        .args(["upgrade", formula])
        .output()
        .map_err(|source| InboxError::Io {
            path: PathBuf::from(&brew_spawn_program),
            source,
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let npm_program = resolve_command_path("npm");
    let tool = inspect_tool(spec, npm_program.as_deref());
    Ok(AgentToolInstallResult {
        tool,
        command,
        steps: vec![],
        exit_code: output.status.code(),
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    })
}

fn install_pi_tool(spec: &AgentToolSpec) -> Result<AgentToolInstallResult, InboxError> {
    let npm_program = resolve_command_path("npm").ok_or_else(|| {
        InboxError::InvalidInput("npm is not available; install Node.js/npm first".to_string())
    })?;
    let mut steps = Vec::new();

    let npm_step = run_npm_install_step(spec, &npm_program)?;
    let npm_succeeded = npm_step.exit_code == Some(0);
    steps.push(npm_step);

    if npm_succeeded {
        steps.push(run_pi_adapter_install_step());
    }

    let command = steps
        .iter()
        .map(|step| step.command.as_str())
        .collect::<Vec<_>>()
        .join(" && ");
    let exit_code = steps
        .iter()
        .find_map(|step| (step.exit_code != Some(0)).then_some(step.exit_code))
        .flatten()
        .or_else(|| steps.last().and_then(|step| step.exit_code));
    let stdout = combined_step_tail(&steps, true);
    let stderr = combined_step_tail(&steps, false);
    let tool = inspect_tool(spec, Some(&npm_program));
    Ok(AgentToolInstallResult {
        tool,
        command,
        steps,
        exit_code,
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    })
}

fn run_npm_install_step(
    spec: &AgentToolSpec,
    npm_program: &Path,
) -> Result<AgentToolInstallStepResult, InboxError> {
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
    Ok(AgentToolInstallStepResult {
        id: "pi_cli".to_string(),
        label: "Pi CLI".to_string(),
        command,
        exit_code: output.status.code(),
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    })
}

fn run_pi_adapter_install_step() -> AgentToolInstallStepResult {
    let command = format!("pi install {PI_MCP_ADAPTER_PACKAGE}");
    let Some(pi_path) = resolve_command_path("pi") else {
        return AgentToolInstallStepResult {
            id: "pi_mcp_adapter".to_string(),
            label: "Pi MCP adapter".to_string(),
            command,
            exit_code: Some(1),
            stdout_tail: None,
            stderr_tail: Some(
                "Pi CLI was installed but `pi` is not available on PATH; restart the shell or fix PATH"
                    .to_string(),
            ),
        };
    };
    let (program, prefix_args) =
        crate::platform::resolve_pi_spawn_command(&pi_path.to_string_lossy());
    let output = match Command::new(&program)
        .args(&prefix_args)
        .args(["install", PI_MCP_ADAPTER_PACKAGE])
        .output()
    {
        Ok(output) => output,
        Err(source) => {
            return AgentToolInstallStepResult {
                id: "pi_mcp_adapter".to_string(),
                label: "Pi MCP adapter".to_string(),
                command,
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: Some(format!("failed to run `{program}`: {source}")),
            };
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    AgentToolInstallStepResult {
        id: "pi_mcp_adapter".to_string(),
        label: "Pi MCP adapter".to_string(),
        command,
        exit_code: output.status.code(),
        stdout_tail: optional_tail(&stdout, 4096),
        stderr_tail: optional_tail(&stderr, 4096),
    }
}

fn combined_step_tail(steps: &[AgentToolInstallStepResult], stdout: bool) -> String {
    steps
        .iter()
        .filter_map(|step| {
            let text = if stdout {
                step.stdout_tail.as_deref()
            } else {
                step.stderr_tail.as_deref()
            }?;
            Some(format!("{}:\n{}", step.label, text))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn status_from_probe(
    spec: &AgentToolSpec,
    npm_available: bool,
    cli_present: bool,
    current_version: Option<&str>,
    npm_version: Option<&str>,
    install_source: Option<AgentToolInstallSource>,
    check_error: Option<&str>,
) -> (AgentToolStatus, Option<String>) {
    if !cli_present {
        if !npm_available {
            return (
                AgentToolStatus::NpmMissing,
                Some("npm is not available".to_string()),
            );
        }
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
    if matches!(install_source, Some(AgentToolInstallSource::Brew)) {
        if let Some(err) = check_error {
            return (AgentToolStatus::CheckFailed, Some(err.to_string()));
        }
        if spec.target_version != "latest" {
            if current_version.is_none() {
                return (
                    AgentToolStatus::CheckFailed,
                    Some(format!("failed to parse `{}` version", spec.cli_name)),
                );
            }
            if current_version != Some(spec.target_version) {
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
        }
        return (AgentToolStatus::Installed, None);
    }
    if !npm_available {
        return (
            AgentToolStatus::ExternalInstall,
            Some("npm is not available; using CLI found on PATH".to_string()),
        );
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

fn pi_components(
    cli_status: AgentToolStatus,
    current_version: Option<&str>,
    cli_error: Option<&str>,
    cli_path: Option<&Path>,
) -> Vec<AgentToolComponent> {
    let mut components = vec![AgentToolComponent {
        id: "pi_cli".to_string(),
        display_name: "Pi CLI".to_string(),
        status: cli_status,
        target: PI_VERSION.to_string(),
        current: current_version.map(str::to_string),
        install_command: Some(pi_cli_install_command_display()),
        last_error: cli_error.map(str::to_string),
    }];

    components.push(inspect_pi_mcp_adapter(cli_status, cli_path));
    components
}

fn inspect_pi_mcp_adapter(
    cli_status: AgentToolStatus,
    cli_path: Option<&Path>,
) -> AgentToolComponent {
    let install_command = format!("pi install {PI_MCP_ADAPTER_PACKAGE}");
    if !matches!(
        cli_status,
        AgentToolStatus::Installed | AgentToolStatus::ExternalInstall
    ) {
        return AgentToolComponent {
            id: "pi_mcp_adapter".to_string(),
            display_name: "Pi MCP adapter".to_string(),
            status: AgentToolStatus::Missing,
            target: PI_MCP_ADAPTER_PACKAGE.to_string(),
            current: None,
            install_command: Some(install_command),
            last_error: Some(
                "Pi CLI must be installed at the required version before checking packages"
                    .to_string(),
            ),
        };
    }

    let Some(cli_path) = cli_path else {
        return AgentToolComponent {
            id: "pi_mcp_adapter".to_string(),
            display_name: "Pi MCP adapter".to_string(),
            status: AgentToolStatus::CheckFailed,
            target: PI_MCP_ADAPTER_PACKAGE.to_string(),
            current: None,
            install_command: Some(install_command),
            last_error: Some("`pi` is not on PATH".to_string()),
        };
    };

    match pi_list_packages(cli_path) {
        Ok(output) => {
            let installed = output
                .lines()
                .any(|line| line.contains("npm:pi-mcp-adapter") || line.contains("pi-mcp-adapter"));
            AgentToolComponent {
                id: "pi_mcp_adapter".to_string(),
                display_name: "Pi MCP adapter".to_string(),
                status: if installed {
                    AgentToolStatus::Installed
                } else {
                    AgentToolStatus::Missing
                },
                target: PI_MCP_ADAPTER_PACKAGE.to_string(),
                current: installed.then(|| PI_MCP_ADAPTER_PACKAGE.to_string()),
                install_command: Some(install_command),
                last_error: (!installed).then(|| {
                    "Pi MCP adapter is required for TaskGraph nodes that use the `mcp` tool"
                        .to_string()
                }),
            }
        }
        Err(err) => AgentToolComponent {
            id: "pi_mcp_adapter".to_string(),
            display_name: "Pi MCP adapter".to_string(),
            status: AgentToolStatus::CheckFailed,
            target: PI_MCP_ADAPTER_PACKAGE.to_string(),
            current: None,
            install_command: Some(install_command),
            last_error: Some(err),
        },
    }
}

fn pi_list_packages(pi_path: &Path) -> Result<String, String> {
    let (program, prefix_args) =
        crate::platform::resolve_pi_spawn_command(&pi_path.to_string_lossy());
    let output = Command::new(&program)
        .args(&prefix_args)
        .arg("list")
        .output()
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(format!("{stdout}\n{stderr}"))
    } else {
        Err(if stderr.trim().is_empty() {
            format!("`pi list` exited with {}", output.status)
        } else {
            stderr.trim().to_string()
        })
    }
}

fn pi_aggregate_status(
    cli_status: AgentToolStatus,
    cli_error: Option<String>,
    components: &[AgentToolComponent],
) -> (AgentToolStatus, Option<String>) {
    if cli_status != AgentToolStatus::Installed {
        return (cli_status, cli_error);
    }
    let Some(adapter) = components
        .iter()
        .find(|component| component.id == "pi_mcp_adapter")
    else {
        return (cli_status, cli_error);
    };
    match adapter.status {
        AgentToolStatus::Installed => (AgentToolStatus::Installed, None),
        AgentToolStatus::Missing => (
            AgentToolStatus::Incomplete,
            Some("Pi MCP adapter missing; run `pi install npm:pi-mcp-adapter`".to_string()),
        ),
        AgentToolStatus::CheckFailed => (AgentToolStatus::CheckFailed, adapter.last_error.clone()),
        other => (other, adapter.last_error.clone()),
    }
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(version) = parse_version(&stdout) {
        return Ok(Some(version));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_version(&stderr))
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

fn pi_cli_install_command_display() -> String {
    let spec = find_tool_spec("pi").expect("pi tool spec must be registered");
    install_command_display(spec)
}

fn tool_install_command_display(
    spec: &AgentToolSpec,
    install_source: Option<AgentToolInstallSource>,
) -> String {
    if matches!(install_source, Some(AgentToolInstallSource::Brew)) {
        if spec.target_version != "latest" {
            return install_command_display(spec);
        }
        if let Some(formula) = spec.brew_formula {
            return brew_update_command_display(formula);
        }
    }
    if spec.id == "pi" {
        return format!(
            "{} && pi install {}",
            install_command_display(spec),
            PI_MCP_ADAPTER_PACKAGE
        );
    }
    install_command_display(spec)
}

fn brew_update_command_display(formula: &str) -> String {
    format!("brew upgrade {formula}")
}

fn detect_install_source(
    cli_path: Option<&Path>,
    npm_version: Option<&str>,
    spec: &AgentToolSpec,
) -> Option<AgentToolInstallSource> {
    if npm_version.is_some() {
        return Some(AgentToolInstallSource::Npm);
    }
    if cli_path
        .and_then(|path| detect_brew_install(path, spec))
        .is_some()
    {
        return Some(AgentToolInstallSource::Brew);
    }
    cli_path.map(|_| AgentToolInstallSource::External)
}

fn detect_brew_install(path: &Path, spec: &AgentToolSpec) -> Option<String> {
    let formula = spec.brew_formula?;
    let canonical = std::fs::canonicalize(path).ok()?;
    let mut previous_was_cellar = false;
    for component in canonical.components() {
        let text = component.as_os_str().to_string_lossy();
        if previous_was_cellar && text == formula {
            return Some(formula.to_string());
        }
        previous_was_cellar = text == "Cellar";
    }
    None
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
    use super::{
        find_tool_spec, status_from_probe, AgentToolComponent, AgentToolInstallSource,
        AgentToolStatus,
    };

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
        assert_eq!(
            super::tool_install_command_display(spec, None),
            "npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.75.4 && pi install npm:pi-mcp-adapter"
        );
    }

    #[test]
    fn pi_installed_cli_without_adapter_is_incomplete() {
        let components = vec![AgentToolComponent {
            id: "pi_mcp_adapter".to_string(),
            display_name: "Pi MCP adapter".to_string(),
            status: AgentToolStatus::Missing,
            target: super::PI_MCP_ADAPTER_PACKAGE.to_string(),
            current: None,
            install_command: Some("pi install npm:pi-mcp-adapter".to_string()),
            last_error: None,
        }];

        let (status, error) =
            super::pi_aggregate_status(AgentToolStatus::Installed, None, &components);

        assert_eq!(status, AgentToolStatus::Incomplete);
        assert_eq!(
            error,
            Some("Pi MCP adapter missing; run `pi install npm:pi-mcp-adapter`".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_version_parses_version_from_stderr() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "bb-agent-tool-version-stderr-{}",
            std::process::id()
        ));
        std::fs::write(&path, "#!/bin/sh\nprintf '0.75.4\\n' >&2\n").unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();

        assert_eq!(
            super::command_version(&path).unwrap(),
            Some("0.75.4".to_string())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_tool_id_is_invalid() {
        let err = find_tool_spec("unknown").unwrap_err().to_string();
        assert!(err.contains("unknown agent tool id"));
    }

    #[test]
    fn opencode_mismatch_and_locked_version_statuses() {
        let spec = find_tool_spec("opencode").unwrap();
        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("1.15.1"),
            Some("1.15.1"),
            Some(AgentToolInstallSource::Npm),
            None,
        );
        assert_eq!(status, AgentToolStatus::VersionMismatch);

        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("1.15.0"),
            Some("1.15.0"),
            Some(AgentToolInstallSource::Npm),
            None,
        );
        assert_eq!(status, AgentToolStatus::Installed);
    }

    #[test]
    fn pi_mismatch_and_locked_version_statuses() {
        let spec = find_tool_spec("pi").unwrap();
        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("0.75.3"),
            Some("0.75.3"),
            Some(AgentToolInstallSource::Npm),
            None,
        );
        assert_eq!(status, AgentToolStatus::VersionMismatch);

        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("0.75.4"),
            Some("0.75.4"),
            Some(AgentToolInstallSource::Npm),
            None,
        );
        assert_eq!(status, AgentToolStatus::Installed);
    }

    #[test]
    fn external_install_when_cli_exists_without_global_package() {
        let spec = find_tool_spec("codex").unwrap();
        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("0.130.0"),
            None,
            Some(AgentToolInstallSource::External),
            None,
        );
        assert_eq!(status, AgentToolStatus::ExternalInstall);
    }

    #[test]
    fn npm_missing_applies_when_cli_is_missing() {
        let spec = find_tool_spec("codex").unwrap();
        let (status, _) = status_from_probe(spec, false, false, None, None, None, None);
        assert_eq!(status, AgentToolStatus::NpmMissing);
    }

    #[test]
    fn brew_install_still_respects_locked_target_version() {
        let spec = find_tool_spec("opencode").unwrap();
        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("1.14.31"),
            None,
            Some(AgentToolInstallSource::Brew),
            None,
        );
        assert_eq!(status, AgentToolStatus::VersionMismatch);
        let (status, _) = status_from_probe(
            spec,
            true,
            true,
            Some("1.15.0"),
            None,
            Some(AgentToolInstallSource::Brew),
            None,
        );
        assert_eq!(status, AgentToolStatus::Installed);
        assert_eq!(
            super::tool_install_command_display(spec, Some(AgentToolInstallSource::Brew)),
            "npm install -g opencode-ai@1.15.0"
        );
        assert!(super::should_apply_locked_version_with_npm(
            spec,
            Some("1.15.10")
        ));
        assert!(!super::should_apply_locked_version_with_npm(
            spec,
            Some("1.15.0")
        ));
    }
}
