use std::collections::BTreeMap;
use std::path::Path as StdPath;

use bb_core::{agents_registry, agents_registry::McpServerConfig, task_graph};

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// 默认的 codex 二进制路径。
const DEFAULT_CODEX_PATH: &str = "/Applications/Codex.app/Contents/Resources/codex";
/// 默认的节点执行超时时间（秒）。
const DEFAULT_NODE_TIMEOUT_SECS: u64 = 600;
/// 默认的整个 Run 执行超时时间（秒）。
const DEFAULT_RUN_TIMEOUT_SECS: u64 = 1800;

/// Optional overrides for RunnerOptions, used by daemon CLI to pass
/// through user-specified flags.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunnerOverrides {
    pub model: Option<String>,
    pub variant: Option<String>,
    pub dry_run: bool,
    pub codex_path: Option<String>,
    pub codebuddy_path: Option<String>,
    pub opencode_path: Option<String>,
    pub agent: Option<String>,
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

/// 构建 RunnerOptions，消除 tg_create_run 和 tg_resume_gate 中的重复代码。
/// When `overrides` is `None`, uses the default runner configuration.
pub(crate) fn build_runner_opts(
    root: &std::path::Path,
    project: String,
    run_id: String,
    overrides: Option<&RunnerOverrides>,
) -> task_graph::RunnerOptions {
    let agent = overrides
        .and_then(|o| o.agent.as_deref())
        .unwrap_or("bb-pm");
    let mcp_servers = overrides.map(|o| o.mcp_servers.as_slice()).unwrap_or(&[]);
    let opencode_config_content = build_opencode_task_graph_config(root, agent, mcp_servers);

    let codex_path = overrides
        .and_then(|o| o.codex_path.clone())
        .unwrap_or_else(|| DEFAULT_CODEX_PATH.to_string());
    let codebuddy_path = overrides
        .and_then(|o| o.codebuddy_path.clone())
        .unwrap_or_else(|| "codebuddy".to_string());
    let opencode_path = overrides
        .and_then(|o| o.opencode_path.clone())
        .unwrap_or_else(|| "opencode".to_string());
    let model = overrides.and_then(|o| o.model.clone());
    let dry_run = overrides.map(|o| o.dry_run).unwrap_or(false);
    let run_timeout = overrides
        .and_then(|o| o.run_timeout_secs)
        .unwrap_or(DEFAULT_RUN_TIMEOUT_SECS);

    task_graph::RunnerOptions {
        workspace_root: root.to_path_buf(),
        project,
        run_id,
        codex_path,
        codebuddy_path,
        opencode_path,
        opencode_config_content,
        model,
        node_timeout: std::time::Duration::from_secs(DEFAULT_NODE_TIMEOUT_SECS),
        run_timeout: std::time::Duration::from_secs(run_timeout),
        dry_run,
        custom_env: overrides.map(|o| o.custom_env.clone()).unwrap_or_default(),
        custom_args: overrides.map(|o| o.custom_args.clone()).unwrap_or_default(),
        mcp_servers: overrides.map(|o| o.mcp_servers.clone()).unwrap_or_default(),
        skills: overrides.map(|o| o.skills.clone()).unwrap_or_default(),
    }
}

pub(crate) fn build_opencode_task_graph_config(
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
                serde_json::to_value(&server.env).unwrap_or(serde_json::json!({}))
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
pub(crate) fn resolve_overrides_from_profile(
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
        agent: cli_overrides.agent.clone(),
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
