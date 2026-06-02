use std::collections::BTreeMap;
use std::path::{Path as StdPath, PathBuf};

use bb_core::InboxError;
use bb_core::{agents_registry, agents_registry::McpServerConfig, task_graph};
use serde::{Deserialize, Serialize};

// ─── Shared helpers ──────────────────────────────────────────────────────────

pub const RUNNER_CONFIG_PATH: &str = "config/task_graph_runner.toml";

/// Last-resort command fallback when no workspace config or override is set.
pub const FALLBACK_CODEX_COMMAND: &str = "codex";
pub const FALLBACK_CODEBUDDY_COMMAND: &str = "codebuddy";
pub const FALLBACK_OPENCODE_COMMAND: &str = "opencode";
pub const FALLBACK_PI_COMMAND: &str = "pi";
/// Last-resort timeout fallbacks when no workspace config or override is set.
pub const FALLBACK_NODE_TIMEOUT_SECS: u64 = 900;
pub const FALLBACK_RUN_TIMEOUT_SECS: u64 = 1800;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunnerConfig {
    #[serde(alias = "codex_path")]
    codex_command: Option<String>,
    #[serde(alias = "codebuddy_path")]
    codebuddy_command: Option<String>,
    #[serde(alias = "opencode_path")]
    opencode_command: Option<String>,
    #[serde(alias = "pi_path")]
    pi_command: Option<String>,
    node_timeout_secs: Option<u64>,
    run_timeout_secs: Option<u64>,
    runtime_max_concurrency: Option<BTreeMap<String, u32>>,
    opencode_max_concurrent: Option<u32>,
    codex_max_concurrent: Option<u32>,
    codebuddy_max_concurrent: Option<u32>,
    /// 飞书终态通知配置（088）。`deny_unknown_fields` 下必须显式声明，
    /// 否则 `[feishu]` section 会导致整个 runner config 解析失败。
    feishu: Option<bb_core::feishu::FeishuTomlConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerRuntimeCommand {
    pub id: String,
    pub command: String,
    pub configured_command: Option<String>,
    pub fallback_command: String,
    pub max_concurrency: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerConfigSnapshot {
    pub config_path: PathBuf,
    pub node_timeout_secs: u64,
    pub run_timeout_secs: u64,
    pub runtimes: Vec<RunnerRuntimeCommand>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunnerConfigPatch {
    pub commands: BTreeMap<String, String>,
    pub runtime_max_concurrency: BTreeMap<String, u32>,
    pub node_timeout_secs: Option<u64>,
    pub run_timeout_secs: Option<u64>,
}

/// Optional overrides for `RunnerOptions`, used by daemon CLI to pass
/// through user-specified flags.
#[derive(Debug, Clone, Default)]
pub struct RunnerOverrides {
    pub model: Option<String>,
    pub variant: Option<String>,
    pub dry_run: bool,
    pub codex_path: Option<String>,
    pub codebuddy_path: Option<String>,
    pub opencode_path: Option<String>,
    pub pi_path: Option<String>,
    pub agent: Option<String>,
    pub node_timeout_secs: Option<u64>,
    pub run_timeout_secs: Option<u64>,
    /// Extra environment variables from Agent Profile.
    pub custom_env: BTreeMap<String, String>,
    /// Extra CLI arguments from Agent Profile.
    pub custom_args: Vec<String>,
    /// Per-agent MCP server declarations (Ticket #000049).
    pub mcp_servers: Vec<McpServerConfig>,
    /// Per-agent skill names (Ticket #000050).
    pub skills: Vec<String>,
}

/// 构建 `RunnerOptions`，消除 `tg_create_run` 和 `tg_resume_gate` 中的重复代码。
/// When `overrides` is `None`, uses the default runner configuration.
pub fn build_runner_opts(
    root: &std::path::Path,
    project: String,
    run_id: String,
    overrides: Option<&RunnerOverrides>,
) -> task_graph::RunnerOptions {
    let config = read_runner_config(root);
    let agent = overrides
        .and_then(|o| o.agent.as_deref())
        .unwrap_or("bb-pm");
    let mcp_servers: &[McpServerConfig] =
        overrides.map_or_else(|| &[] as &[McpServerConfig], |o| o.mcp_servers.as_slice());
    let opencode_config_content = build_opencode_task_graph_config(root, agent, mcp_servers);

    let codex_path = choose_command(
        overrides.and_then(|o| o.codex_path.clone()),
        config.codex_command,
        FALLBACK_CODEX_COMMAND,
    );
    let codebuddy_path = choose_command(
        overrides.and_then(|o| o.codebuddy_path.clone()),
        config.codebuddy_command,
        FALLBACK_CODEBUDDY_COMMAND,
    );
    let opencode_path = choose_command(
        overrides.and_then(|o| o.opencode_path.clone()),
        config.opencode_command,
        FALLBACK_OPENCODE_COMMAND,
    );
    let pi_path = choose_command(
        overrides.and_then(|o| o.pi_path.clone()),
        config.pi_command,
        FALLBACK_PI_COMMAND,
    );
    let model = overrides.and_then(|o| o.model.clone());
    let dry_run = overrides.is_some_and(|o| o.dry_run);
    let node_timeout = choose_secs(
        overrides.and_then(|o| o.node_timeout_secs),
        config.node_timeout_secs,
        FALLBACK_NODE_TIMEOUT_SECS,
    );
    let run_timeout = choose_secs(
        overrides.and_then(|o| o.run_timeout_secs),
        config.run_timeout_secs,
        FALLBACK_RUN_TIMEOUT_SECS,
    );

    task_graph::RunnerOptions {
        workspace_root: root.to_path_buf(),
        scripts_dir: task_graph::resolve_scripts_dir(root),
        project,
        run_id,
        codex_path,
        codebuddy_path,
        opencode_path,
        opencode_config_content,
        pi_path,
        model,
        node_timeout: std::time::Duration::from_secs(node_timeout),
        run_timeout: std::time::Duration::from_secs(run_timeout),
        dry_run,
        custom_env: overrides.map(|o| o.custom_env.clone()).unwrap_or_default(),
        custom_args: overrides.map(|o| o.custom_args.clone()).unwrap_or_default(),
        mcp_servers: overrides.map(|o| o.mcp_servers.clone()).unwrap_or_default(),
        skills: overrides.map(|o| o.skills.clone()).unwrap_or_default(),
    }
}

fn read_runner_config(root: &StdPath) -> RunnerConfig {
    let path = root.join(RUNNER_CONFIG_PATH);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return RunnerConfig::default();
        }
        Err(err) => {
            eprintln!(
                "bb warning: failed to read task graph runner config {}: {}",
                path.display(),
                err
            );
            return RunnerConfig::default();
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(err) => {
            eprintln!(
                "bb warning: failed to parse task graph runner config {}: {}",
                path.display(),
                err
            );
            RunnerConfig::default()
        }
    }
}

/// 读取 workspace 的 `[feishu]` section（088 飞书通知）。
/// 读取/解析失败时返回 `None`，由调用方按"未启用"处理。
pub fn read_runner_feishu(root: &StdPath) -> Option<bb_core::feishu::FeishuTomlConfig> {
    read_runner_config(root).feishu
}

pub fn read_runner_config_snapshot(root: &StdPath) -> RunnerConfigSnapshot {
    let config = read_runner_config(root);
    let runtime_max_concurrency = config.runtime_max_concurrency.clone().unwrap_or_default();
    RunnerConfigSnapshot {
        config_path: root.join(RUNNER_CONFIG_PATH),
        node_timeout_secs: choose_secs(None, config.node_timeout_secs, FALLBACK_NODE_TIMEOUT_SECS),
        run_timeout_secs: choose_secs(None, config.run_timeout_secs, FALLBACK_RUN_TIMEOUT_SECS),
        runtimes: vec![
            runtime_command(
                "codex",
                config.codex_command,
                FALLBACK_CODEX_COMMAND,
                runtime_max_concurrency
                    .get("codex")
                    .copied()
                    .or(config.codex_max_concurrent),
            ),
            runtime_command(
                "codebuddy",
                config.codebuddy_command,
                FALLBACK_CODEBUDDY_COMMAND,
                runtime_max_concurrency
                    .get("codebuddy")
                    .copied()
                    .or(config.codebuddy_max_concurrent),
            ),
            runtime_command(
                "opencode",
                config.opencode_command,
                FALLBACK_OPENCODE_COMMAND,
                runtime_max_concurrency
                    .get("opencode")
                    .copied()
                    .or(config.opencode_max_concurrent),
            ),
            runtime_command(
                "pi",
                config.pi_command,
                FALLBACK_PI_COMMAND,
                runtime_max_concurrency.get("pi").copied(),
            ),
        ],
    }
}

pub fn patch_runner_config(
    root: &StdPath,
    patch: RunnerConfigPatch,
) -> Result<RunnerConfigSnapshot, InboxError> {
    let current = read_runner_config_snapshot(root);
    let mut commands: BTreeMap<String, String> = current
        .runtimes
        .iter()
        .map(|runtime| (runtime.id.clone(), runtime.command.clone()))
        .collect();
    for (id, command) in patch.commands {
        if is_known_runtime(&id) && !command.trim().is_empty() {
            commands.insert(id, command.trim().to_string());
        }
    }

    let mut runtime_max_concurrency: BTreeMap<String, u32> = current
        .runtimes
        .iter()
        .filter_map(|runtime| {
            runtime
                .max_concurrency
                .map(|value| (runtime.id.clone(), value))
        })
        .collect();
    for (id, value) in patch.runtime_max_concurrency {
        if is_known_runtime(&id) {
            runtime_max_concurrency.insert(id, value);
        }
    }

    let node_timeout_secs = patch
        .node_timeout_secs
        .filter(|value| *value > 0)
        .unwrap_or(current.node_timeout_secs);
    let run_timeout_secs = patch
        .run_timeout_secs
        .filter(|value| *value > 0)
        .unwrap_or(current.run_timeout_secs);

    let content = render_runner_config(
        &commands,
        node_timeout_secs,
        run_timeout_secs,
        &runtime_max_concurrency,
    );
    let path = root.join(RUNNER_CONFIG_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| InboxError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&path, content).map_err(|source| InboxError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(read_runner_config_snapshot(root))
}

fn runtime_command(
    id: &str,
    configured_command: Option<String>,
    fallback_command: &str,
    max_concurrency: Option<u32>,
) -> RunnerRuntimeCommand {
    let command = choose_command(None, configured_command.clone(), fallback_command);
    RunnerRuntimeCommand {
        id: id.to_string(),
        command,
        configured_command,
        fallback_command: fallback_command.to_string(),
        max_concurrency,
    }
}

fn is_known_runtime(id: &str) -> bool {
    matches!(id, "codex" | "codebuddy" | "opencode" | "pi")
}

fn render_runner_config(
    commands: &BTreeMap<String, String>,
    node_timeout_secs: u64,
    run_timeout_secs: u64,
    runtime_max_concurrency: &BTreeMap<String, u32>,
) -> String {
    let command = |id: &str, fallback: &str| {
        toml_string(commands.get(id).map(String::as_str).unwrap_or(fallback))
    };
    let mut output = String::new();
    output.push_str("# Workspace-level defaults for Task Graph execution.\n");
    output.push_str("# CLI/HTTP overrides take precedence over these values.\n");
    output.push_str(&format!(
        "codex_command = {}\n",
        command("codex", FALLBACK_CODEX_COMMAND)
    ));
    output.push_str(&format!(
        "codebuddy_command = {}\n",
        command("codebuddy", FALLBACK_CODEBUDDY_COMMAND)
    ));
    output.push_str(&format!(
        "opencode_command = {}\n",
        command("opencode", FALLBACK_OPENCODE_COMMAND)
    ));
    output.push_str(&format!(
        "pi_command = {}\n\n",
        command("pi", FALLBACK_PI_COMMAND)
    ));
    output.push_str(&format!("node_timeout_secs = {node_timeout_secs}\n"));
    output.push_str(&format!("run_timeout_secs = {run_timeout_secs}\n\n"));
    output
        .push_str("# Strict runtime-level process concurrency across all TaskGraph runs in this\n");
    output
        .push_str("# bb-server process. Parent graph concurrency still controls run admission;\n");
    output.push_str("# this guard also covers dynamic fanout scouts.\n");
    output.push_str("[runtime_max_concurrency]\n");
    for (id, value) in runtime_max_concurrency {
        if is_known_runtime(id) {
            output.push_str(&format!("{id} = {value}\n"));
        }
    }
    output
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn choose_command(
    override_value: Option<String>,
    config_value: Option<String>,
    fallback: &str,
) -> String {
    override_value
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config_value.filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| fallback.to_string())
}

fn choose_secs(override_value: Option<u64>, config_value: Option<u64>, fallback: u64) -> u64 {
    override_value
        .filter(|value| *value > 0)
        .or_else(|| config_value.filter(|value| *value > 0))
        .unwrap_or(fallback)
}

pub fn build_opencode_task_graph_config(
    root: &StdPath,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Option<String> {
    let bb_exe = std::env::current_exe().ok()?;
    let mut config = read_opencode_config().unwrap_or_else(|| serde_json::json!({}));
    let object = config.as_object_mut()?;
    let mcp = object
        .entry("mcp")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()?;
    mcp.insert(
        "bb".to_string(),
        serde_json::json!({
            "enabled": true,
            "type": "local",
            "command": [
                bb_exe.display().to_string(),
                "--root",
                root.display().to_string(),
                "stdio"
            ],
            "environment": {
                "BB_DAEMON": "1",
                "BB_DAEMON_AGENT": agent
            },
            "timeout": 30000
        }),
    );

    // Inject per-agent MCP servers (Ticket #000049)
    for server in extra_mcp_servers {
        if server.transport == "stdio" {
            let cmd = server.command.as_deref().unwrap_or_default();
            let mut command_arr = vec![cmd.to_string()];
            command_arr.extend(server.args.iter().cloned());
            let env_obj: serde_json::Value = if server.env.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::to_value(&server.env).unwrap_or_else(|_| serde_json::json!({}))
            };
            mcp.insert(
                server.name.clone(),
                serde_json::json!({
                    "enabled": true,
                    "type": "local",
                    "command": command_arr,
                    "environment": env_obj,
                    "timeout": 30000
                }),
            );
        } else if server.transport == "sse" {
            if let Some(ref url) = server.url {
                mcp.insert(
                    server.name.clone(),
                    serde_json::json!({
                        "enabled": true,
                        "type": "remote",
                        "url": url,
                        "timeout": 30000
                    }),
                );
            }
        }
    }

    serde_json::to_string(&config).ok()
}

fn read_opencode_config() -> Option<serde_json::Value> {
    let home = std::env::var_os("HOME")?;
    let path = std::path::PathBuf::from(home).join(".config/opencode/opencode.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Resolve `RunnerOverrides` by merging Agent Profile defaults with CLI
/// overrides. CLI values always take precedence over Profile values.
pub fn resolve_overrides_from_profile(
    bb_root: &StdPath,
    cli_overrides: &RunnerOverrides,
) -> RunnerOverrides {
    let agent_id = cli_overrides.agent.as_deref().unwrap_or("bb-pm");

    // Try to load the agent profile; if it fails, just return CLI overrides as-is.
    let profile = match agents_registry::list_agents(bb_root) {
        Ok(registry) => registry.agents.into_iter().find(|a| a.id == agent_id),
        Err(_) => None,
    };

    let Some(profile) = profile else {
        return cli_overrides.clone();
    };

    RunnerOverrides {
        // CLI model takes precedence over Profile model
        model: cli_overrides.model.clone().or(profile.model),
        // CLI variant takes precedence over Profile variant.
        variant: cli_overrides.variant.clone().or(profile.variant),
        dry_run: cli_overrides.dry_run,
        codex_path: cli_overrides.codex_path.clone(),
        codebuddy_path: cli_overrides.codebuddy_path.clone(),
        opencode_path: cli_overrides.opencode_path.clone(),
        pi_path: cli_overrides.pi_path.clone(),
        agent: cli_overrides.agent.clone(),
        node_timeout_secs: cli_overrides.node_timeout_secs,
        run_timeout_secs: cli_overrides.run_timeout_secs,
        // Merge custom_env: Profile provides base, CLI (if any) would override
        custom_env: {
            let mut env = profile.custom_env;
            env.extend(cli_overrides.custom_env.clone());
            env
        },
        // Merge custom_args: Profile provides base, CLI appends
        custom_args: {
            let mut args = profile.custom_args;
            args.extend(cli_overrides.custom_args.clone());
            args
        },
        // MCP servers from Profile (CLI doesn't override these)
        mcp_servers: profile.mcp_servers,
        // Skills from Profile
        skills: profile.skills,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_runner_config(root: &StdPath, content: &str) {
        let config_dir = root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("task_graph_runner.toml"), content).unwrap();
    }

    #[test]
    fn defaults_use_shell_commands_not_platform_absolute_paths() {
        let temp = tempfile::tempdir().unwrap();
        let opts = build_runner_opts(
            temp.path(),
            "blackboard".to_string(),
            "run-defaults".to_string(),
            None,
        );

        assert_eq!(opts.codex_path, "codex");
        assert_eq!(opts.codebuddy_path, "codebuddy");
        assert_eq!(opts.opencode_path, "opencode");
        assert_eq!(opts.pi_path, "pi");
        assert_eq!(opts.node_timeout.as_secs(), FALLBACK_NODE_TIMEOUT_SECS);
        assert_eq!(opts.run_timeout.as_secs(), FALLBACK_RUN_TIMEOUT_SECS);
    }

    #[test]
    fn workspace_runner_config_overrides_fallbacks() {
        let temp = tempfile::tempdir().unwrap();
        write_runner_config(
            temp.path(),
            r#"
codex_command = "codex-from-config"
codebuddy_command = "codebuddy-from-config"
opencode_command = "opencode-from-config"
pi_command = "pi-from-config"
node_timeout_secs = 12
run_timeout_secs = 34

[runtime_max_concurrency]
opencode = 1
"#,
        );

        let opts = build_runner_opts(
            temp.path(),
            "blackboard".to_string(),
            "run-config".to_string(),
            None,
        );

        assert_eq!(opts.codex_path, "codex-from-config");
        assert_eq!(opts.codebuddy_path, "codebuddy-from-config");
        assert_eq!(opts.opencode_path, "opencode-from-config");
        assert_eq!(opts.pi_path, "pi-from-config");
        assert_eq!(opts.node_timeout.as_secs(), 12);
        assert_eq!(opts.run_timeout.as_secs(), 34);
    }

    #[test]
    fn explicit_runner_overrides_take_precedence_over_workspace_config() {
        let temp = tempfile::tempdir().unwrap();
        write_runner_config(
            temp.path(),
            r#"
codex_command = "codex-from-config"
node_timeout_secs = 12
run_timeout_secs = 34
"#,
        );

        let overrides = RunnerOverrides {
            codex_path: Some("codex-from-override".to_string()),
            node_timeout_secs: Some(56),
            run_timeout_secs: Some(78),
            ..Default::default()
        };
        let opts = build_runner_opts(
            temp.path(),
            "blackboard".to_string(),
            "run-overrides".to_string(),
            Some(&overrides),
        );

        assert_eq!(opts.codex_path, "codex-from-override");
        assert_eq!(opts.node_timeout.as_secs(), 56);
        assert_eq!(opts.run_timeout.as_secs(), 78);
    }

    #[test]
    fn patch_runner_config_persists_runtime_connector_settings() {
        let temp = tempfile::tempdir().unwrap();
        let mut commands = BTreeMap::new();
        commands.insert("codex".to_string(), "codex-next".to_string());
        commands.insert("pi".to_string(), "pi-next".to_string());
        let mut runtime_max_concurrency = BTreeMap::new();
        runtime_max_concurrency.insert("codex".to_string(), 2);
        runtime_max_concurrency.insert("pi".to_string(), 4);

        let snapshot = patch_runner_config(
            temp.path(),
            RunnerConfigPatch {
                commands,
                runtime_max_concurrency,
                node_timeout_secs: Some(11),
                run_timeout_secs: Some(22),
            },
        )
        .unwrap();

        assert_eq!(snapshot.node_timeout_secs, 11);
        assert_eq!(snapshot.run_timeout_secs, 22);
        assert_eq!(
            snapshot
                .runtimes
                .iter()
                .find(|runtime| runtime.id == "codex")
                .unwrap()
                .command,
            "codex-next"
        );
        assert_eq!(
            snapshot
                .runtimes
                .iter()
                .find(|runtime| runtime.id == "pi")
                .unwrap()
                .max_concurrency,
            Some(4)
        );

        let opts = build_runner_opts(
            temp.path(),
            "blackboard".to_string(),
            "run-patched".to_string(),
            None,
        );
        assert_eq!(opts.codex_path, "codex-next");
        assert_eq!(opts.pi_path, "pi-next");
        assert_eq!(opts.node_timeout.as_secs(), 11);
        assert_eq!(opts.run_timeout.as_secs(), 22);
    }
}
