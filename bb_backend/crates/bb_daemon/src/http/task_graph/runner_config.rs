use std::collections::BTreeMap;
use std::path::Path as StdPath;

use bb_core::{agents_registry, agents_registry::McpServerConfig, task_graph};
use serde::Deserialize;

// ─── Shared helpers ──────────────────────────────────────────────────────────

const RUNNER_CONFIG_PATH: &str = "config/task_graph_runner.toml";

/// Last-resort command fallback when no workspace config or override is set.
const FALLBACK_CODEX_COMMAND: &str = "codex";
const FALLBACK_CODEBUDDY_COMMAND: &str = "codebuddy";
const FALLBACK_OPENCODE_COMMAND: &str = "opencode";
const FALLBACK_PI_COMMAND: &str = "pi";
/// Last-resort timeout fallbacks when no workspace config or override is set.
const FALLBACK_NODE_TIMEOUT_SECS: u64 = 900;
const FALLBACK_RUN_TIMEOUT_SECS: u64 = 1800;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RunnerConfig {
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
}
