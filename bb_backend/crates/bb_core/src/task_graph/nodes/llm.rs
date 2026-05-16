//! LLM node resolver and provider config helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents_registry::{self, AgentProfile, McpServerConfig};

use super::eval::render_prompt_template;
use crate::task_graph::definition::types::{LlmConfig, LlmRunAs, TaskGraphError, KNOWN_RUNTIMES};
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::RunContext;

#[derive(Debug, Clone)]
pub struct ResolvedLlmInvocation {
    pub run_as: LlmRunAs,
    pub runtime: String,
    pub agent: String,
    pub model: Option<String>,
    pub variant: Option<String>,
    pub prompt: String,
    pub output: Option<serde_json::Value>,
    pub skills: Vec<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub custom_env: BTreeMap<String, String>,
    pub custom_args: Vec<String>,
    pub opencode_config_content: Option<String>,
    pub codex_config_args: Vec<String>,
    pub codebuddy_mcp_config_content: Option<String>,
    pub codebuddy_settings_json: Option<String>,
}

pub fn resolve_llm_invocation(
    opts: &RunnerOptions,
    config: &LlmConfig,
    context: &RunContext,
) -> Result<ResolvedLlmInvocation, TaskGraphError> {
    match config.run_as {
        LlmRunAs::Agent => resolve_agent_invocation(opts, config, context),
        LlmRunAs::Llm => resolve_inline_llm_invocation(opts, config, context),
    }
}

pub fn build_opencode_task_graph_config(
    root: &Path,
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

    for server in extra_mcp_servers {
        if server.transport == "stdio" {
            let Some(cmd) = server
                .command
                .as_deref()
                .filter(|cmd| !cmd.trim().is_empty())
            else {
                continue;
            };
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

pub fn build_codex_mcp_config_args(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Vec<String> {
    let mut servers = Vec::new();
    if let Ok(bb_exe) = std::env::current_exe() {
        let mut env = BTreeMap::new();
        env.insert("BB_DAEMON".to_string(), "1".to_string());
        env.insert("BB_DAEMON_AGENT".to_string(), agent.to_string());
        servers.push(McpServerConfig {
            name: "bb".to_string(),
            transport: "stdio".to_string(),
            command: Some(bb_exe.display().to_string()),
            args: vec![
                "--root".to_string(),
                root.display().to_string(),
                "stdio".to_string(),
            ],
            url: None,
            env,
        });
    }
    merge_mcp_servers(&mut servers, extra_mcp_servers);

    let mut args = Vec::new();
    for server in servers {
        if server.name.trim().is_empty() {
            continue;
        }
        let base = format!("mcp_servers.{}", toml_key_segment(&server.name));
        let mut table = toml::Table::new();
        if server.transport == "stdio" {
            if let Some(command) = server.command.filter(|cmd| !cmd.trim().is_empty()) {
                table.insert("command".to_string(), toml::Value::String(command));
                if !server.args.is_empty() {
                    table.insert(
                        "args".to_string(),
                        toml::Value::Array(
                            server.args.into_iter().map(toml::Value::String).collect(),
                        ),
                    );
                }
            }
        } else if server.transport == "sse" {
            if let Some(url) = server.url.filter(|url| !url.trim().is_empty()) {
                table.insert("url".to_string(), toml::Value::String(url));
            }
        }

        if !server.env.is_empty() {
            let env = server
                .env
                .into_iter()
                .map(|(key, value)| (toml_key_segment(&key), toml::Value::String(value)))
                .collect();
            table.insert("env".to_string(), toml::Value::Table(env));
        }

        if !table.is_empty() {
            args.push(toml_config_arg(&base, toml::Value::Table(table)));
        }
    }
    args
}

pub fn build_codebuddy_mcp_config_content(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Option<String> {
    let mut mcp_servers = serde_json::Map::new();
    for server in codebuddy_mcp_servers(root, agent, extra_mcp_servers) {
        let name = server.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if let Some(config) = codebuddy_mcp_server_value(server) {
            mcp_servers.insert(name, config);
        }
    }
    serde_json::to_string(&serde_json::json!({ "mcpServers": mcp_servers })).ok()
}

pub fn build_codebuddy_settings_json(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
    variant: Option<&str>,
) -> Option<String> {
    let enabled: Vec<String> = codebuddy_mcp_servers(root, agent, extra_mcp_servers)
        .into_iter()
        .filter_map(|server| {
            let name = server.name.trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect();
    let mut settings = serde_json::json!({
        "trustAll": true,
        "enableAllProjectMcpServers": true,
        "enabledMcpjsonServers": enabled,
    });
    if let Some(variant) = variant.map(str::trim).filter(|value| !value.is_empty()) {
        settings["reasoningEffort"] = serde_json::Value::String(variant.to_string());
    }
    serde_json::to_string(&settings).ok()
}

fn resolve_inline_llm_invocation(
    opts: &RunnerOptions,
    config: &LlmConfig,
    context: &RunContext,
) -> Result<ResolvedLlmInvocation, TaskGraphError> {
    let runtime = resolve_runtime(&config.runtime)?;
    let agent = if config.agent.trim().is_empty() {
        "native".to_string()
    } else {
        config.agent.trim().to_string()
    };
    let prompt_template = prompt_template_for_config(&opts.workspace_root, config)?;
    let prompt = render_prompt_template(
        &prompt_template,
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        context,
        config.inputs.as_ref(),
    );
    let skills = merge_strings(&opts.skills, &config.skills);
    let mut mcp_servers = opts.mcp_servers.clone();
    merge_mcp_servers(&mut mcp_servers, &config.mcp_servers);
    let mut custom_env = opts.custom_env.clone();
    custom_env.extend(config.custom_env.clone());
    let mut custom_args = opts.custom_args.clone();
    custom_args.extend(config.custom_args.clone());

    Ok(build_resolved_invocation(
        opts,
        LlmRunAs::Llm,
        runtime,
        agent,
        opts.model.clone().or_else(|| config.model.clone()),
        config.variant.clone(),
        prompt,
        config.output.clone(),
        skills,
        mcp_servers,
        custom_env,
        custom_args,
    ))
}

fn resolve_agent_invocation(
    opts: &RunnerOptions,
    config: &LlmConfig,
    context: &RunContext,
) -> Result<ResolvedLlmInvocation, TaskGraphError> {
    let profile_id = config
        .agent_profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            TaskGraphError::InvalidGraphId(
                "LLM node run_as=agent requires config.agent_profile".to_string(),
            )
        })?;
    let profile = find_project_agent(&opts.workspace_root, &opts.project, profile_id)?;
    let runtime = profile.runtime.clone().ok_or_else(|| {
        TaskGraphError::InvalidGraphId(format!(
            "agent profile `{profile_id}` does not define runtime"
        ))
    })?;
    let runtime = resolve_runtime(&runtime)?;
    let prompt_template = prompt_template_for_agent_config(&opts.workspace_root, config, &profile)?;
    let prompt = render_prompt_template(
        &prompt_template,
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        context,
        config.inputs.as_ref(),
    );

    Ok(build_resolved_invocation(
        opts,
        LlmRunAs::Agent,
        runtime,
        profile.id.clone(),
        opts.model.clone().or_else(|| profile.model.clone()),
        profile.variant.clone(),
        prompt,
        config.output.clone(),
        profile.skills.clone(),
        profile.mcp_servers.clone(),
        profile.custom_env.clone(),
        profile.custom_args.clone(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_resolved_invocation(
    opts: &RunnerOptions,
    run_as: LlmRunAs,
    runtime: String,
    agent: String,
    model: Option<String>,
    variant: Option<String>,
    prompt: String,
    output: Option<serde_json::Value>,
    skills: Vec<String>,
    mcp_servers: Vec<McpServerConfig>,
    custom_env: BTreeMap<String, String>,
    custom_args: Vec<String>,
) -> ResolvedLlmInvocation {
    let opencode_config_content =
        build_opencode_task_graph_config(&opts.workspace_root, &agent, &mcp_servers)
            .or_else(|| opts.opencode_config_content.clone());
    let mut codex_config_args =
        build_codex_mcp_config_args(&opts.workspace_root, &agent, &mcp_servers);
    if let Some(variant) = variant
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        codex_config_args.push(toml_config_arg(
            "model_reasoning_effort",
            toml::Value::String(variant.to_string()),
        ));
    }
    let codebuddy_mcp_config_content =
        build_codebuddy_mcp_config_content(&opts.workspace_root, &agent, &mcp_servers);
    let codebuddy_settings_json = build_codebuddy_settings_json(
        &opts.workspace_root,
        &agent,
        &mcp_servers,
        variant.as_deref(),
    );
    ResolvedLlmInvocation {
        run_as,
        runtime,
        agent,
        model: model.filter(|value| !value.trim().is_empty()),
        variant: variant.filter(|value| !value.trim().is_empty()),
        prompt,
        output,
        skills,
        mcp_servers,
        custom_env,
        custom_args,
        opencode_config_content,
        codex_config_args,
        codebuddy_mcp_config_content,
        codebuddy_settings_json,
    }
}

fn prompt_template_for_config(
    workspace_root: &Path,
    config: &LlmConfig,
) -> Result<String, TaskGraphError> {
    if config.prompt.mode == "file" {
        let path = workspace_root.join(&config.prompt.template);
        return fs::read_to_string(&path).map_err(|source| TaskGraphError::Io { path, source });
    }
    Ok(config.prompt.template.clone())
}

fn prompt_template_for_agent_config(
    workspace_root: &Path,
    config: &LlmConfig,
    profile: &AgentProfile,
) -> Result<String, TaskGraphError> {
    if has_node_prompt(config) {
        return prompt_template_for_config(workspace_root, config);
    }

    let prompt_path = profile
        .instructions_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            TaskGraphError::InvalidGraphId(format!(
                "agent profile `{}` does not define instructions_path and node task prompt is empty",
                profile.id
            ))
        })?;
    read_profile_prompt(workspace_root, &profile.id, prompt_path)
}

fn has_node_prompt(config: &LlmConfig) -> bool {
    !config.prompt.template.trim().is_empty()
}

fn resolve_runtime(runtime: &str) -> Result<String, TaskGraphError> {
    let runtime = match runtime.trim() {
        "" => "codex",
        value => value,
    };
    if !KNOWN_RUNTIMES.contains(&runtime) {
        return Err(TaskGraphError::InvalidGraphId(format!(
            "unsupported LLM runtime `{runtime}`; use `codex`, `opencode`, or `codebuddy`"
        )));
    }
    Ok(runtime.to_string())
}

fn find_project_agent(
    workspace_root: &Path,
    project: &str,
    profile_id: &str,
) -> Result<AgentProfile, TaskGraphError> {
    let list = agents_registry::list_project_agents(workspace_root, project).map_err(|err| {
        TaskGraphError::InvalidGraphId(format!("failed to load agent registry: {err}"))
    })?;
    list.agents
        .into_iter()
        .map(|profile| profile.agent)
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| {
            TaskGraphError::InvalidGraphId(format!(
                "agent profile `{profile_id}` is not active/assignable for project `{project}`"
            ))
        })
}

fn read_profile_prompt(
    workspace_root: &Path,
    profile_id: &str,
    prompt_path: &str,
) -> Result<String, TaskGraphError> {
    let path = workspace_root.join(prompt_path);
    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let canonical_path = path.canonicalize().map_err(|source| TaskGraphError::Io {
        path: path.clone(),
        source,
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(TaskGraphError::InvalidGraphId(format!(
            "agent profile `{profile_id}` instructions_path `{prompt_path}` escapes workspace root"
        )));
    }
    fs::read_to_string(&canonical_path).map_err(|source| TaskGraphError::Io {
        path: canonical_path,
        source,
    })
}

fn merge_strings(base: &[String], extra: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for value in base.iter().chain(extra.iter()) {
        let trimmed = value.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        merged.push(trimmed.to_string());
    }
    merged
}

fn merge_mcp_servers(base: &mut Vec<McpServerConfig>, extra: &[McpServerConfig]) {
    for server in extra {
        if let Some(existing) = base.iter_mut().find(|item| item.name == server.name) {
            *existing = server.clone();
        } else {
            base.push(server.clone());
        }
    }
}

fn codebuddy_mcp_servers(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Vec<McpServerConfig> {
    let mut servers = Vec::new();
    if let Ok(bb_exe) = std::env::current_exe() {
        let mut env = BTreeMap::new();
        env.insert("BB_DAEMON".to_string(), "1".to_string());
        env.insert("BB_DAEMON_AGENT".to_string(), agent.to_string());
        servers.push(McpServerConfig {
            name: "bb".to_string(),
            transport: "stdio".to_string(),
            command: Some(bb_exe.display().to_string()),
            args: vec![
                "--root".to_string(),
                root.display().to_string(),
                "stdio".to_string(),
            ],
            url: None,
            env,
        });
    }
    merge_mcp_servers(&mut servers, extra_mcp_servers);
    servers
        .into_iter()
        .map(|server| normalize_codebuddy_mcp_server(root, server))
        .collect()
}

fn normalize_codebuddy_mcp_server(root: &Path, mut server: McpServerConfig) -> McpServerConfig {
    if let Some(command) = server.command.as_mut() {
        *command = expand_bb_root(root, command);
        if command.as_str() == "bb" && server.args.iter().any(|arg| arg == "stdio") {
            if let Ok(bb_exe) = std::env::current_exe() {
                *command = bb_exe.display().to_string();
            }
        }
    }
    if let Some(url) = server.url.as_mut() {
        *url = expand_bb_root(root, url);
    }
    for arg in &mut server.args {
        *arg = expand_bb_root(root, arg);
    }
    for value in server.env.values_mut() {
        *value = expand_bb_root(root, value);
    }
    server
}

fn expand_bb_root(root: &Path, value: &str) -> String {
    value.replace("<bb-root>", &root.display().to_string())
}

fn codebuddy_mcp_server_value(server: McpServerConfig) -> Option<serde_json::Value> {
    match server.transport.as_str() {
        "stdio" => {
            let command = server.command.filter(|cmd| !cmd.trim().is_empty())?;
            let mut value = serde_json::json!({
                "type": "stdio",
                "command": command,
                "timeout": 30000,
            });
            if !server.args.is_empty() {
                value["args"] = serde_json::to_value(server.args).ok()?;
            }
            if !server.env.is_empty() {
                value["env"] = serde_json::to_value(server.env).ok()?;
            }
            Some(value)
        }
        "sse" => server.url.filter(|url| !url.trim().is_empty()).map(|url| {
            serde_json::json!({
                "type": "sse",
                "url": url,
                "timeout": 30000,
            })
        }),
        "http" | "streamable-http" => server.url.filter(|url| !url.trim().is_empty()).map(|url| {
            serde_json::json!({
                "type": "http",
                "url": url,
                "timeout": 30000,
            })
        }),
        _ => None,
    }
}

fn toml_config_arg(key: &str, value: toml::Value) -> String {
    format!("{key}={}", value)
}

fn toml_key_segment(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        value.to_string()
    } else {
        format!("{value:?}")
    }
}

fn read_opencode_config() -> Option<serde_json::Value> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".config/opencode/opencode.json");
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    fn opts(root: &Path, project: &str) -> RunnerOptions {
        RunnerOptions {
            workspace_root: root.to_path_buf(),
            scripts_dir: root.join("scripts"),
            project: project.to_string(),
            run_id: "run-1".to_string(),
            codex_path: "codex".to_string(),
            codebuddy_path: "codebuddy".to_string(),
            opencode_path: "opencode".to_string(),
            opencode_config_content: None,
            model: None,
            node_timeout: Duration::from_secs(60),
            run_timeout: Duration::from_secs(60),
            dry_run: false,
            custom_env: BTreeMap::new(),
            custom_args: Vec::new(),
            mcp_servers: Vec::new(),
            skills: Vec::new(),
        }
    }

    fn context() -> RunContext {
        RunContext {
            input: serde_json::json!({ "topic": "hello" }),
            node_outputs: serde_json::Map::new(),
            branch_decisions: Vec::new(),
            loop_iterations: Vec::new(),
            loop_stack: Vec::new(),
            completed_branches: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn default_config_resolves_as_llm_mode() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "opencode",
            "agent": "native",
            "prompt": { "mode": "inline", "template": "say {{inputs.topic}}" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.run_as, LlmRunAs::Llm);
        assert_eq!(resolved.runtime, "opencode");
        assert_eq!(resolved.agent, "native");
        assert_eq!(resolved.prompt, "say hello");
    }

    #[test]
    fn llm_mode_variant_becomes_codex_reasoning_effort_config() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "codex",
            "agent": "codex",
            "model": "gpt-5.4-mini",
            "variant": "low",
            "prompt": { "mode": "inline", "template": "say hi" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.model.as_deref(), Some("gpt-5.4-mini"));
        assert_eq!(resolved.variant.as_deref(), Some("low"));
        assert!(resolved
            .codex_config_args
            .iter()
            .any(|arg| arg == r#"model_reasoning_effort="low""#));
    }

    #[test]
    fn llm_mode_variant_becomes_codebuddy_reasoning_effort_setting() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "codebuddy",
            "agent": "native",
            "model": "gpt-5",
            "variant": "low",
            "prompt": { "mode": "inline", "template": "say hi" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context()).unwrap();
        let settings: serde_json::Value =
            serde_json::from_str(resolved.codebuddy_settings_json.as_ref().unwrap()).unwrap();

        assert_eq!(resolved.runtime, "codebuddy");
        assert_eq!(resolved.model.as_deref(), Some("gpt-5"));
        assert_eq!(settings["reasoningEffort"], "low");
        assert_eq!(settings["enableAllProjectMcpServers"], true);
    }

    #[test]
    fn codex_interactive_runtime_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "codex-interactive",
            "agent": "codex",
            "prompt": { "mode": "inline", "template": "say hi" }
        }))
        .unwrap();

        let err = resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context())
            .unwrap_err();

        assert!(err.to_string().contains("unsupported LLM runtime"));
    }

    #[test]
    fn agent_mode_reads_profile_markdown_prompt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents/prompts")).unwrap();
        fs::write(
            root.join("agents/prompts/bb-pm.md"),
            "profile {{inputs.topic}}",
        )
        .unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
model = "model-a"
variant = "high"
instructions_path = "agents/prompts/bb-pm.md"
skills = ["triage"]

[[agents.mcp_servers]]
name = "qmd"
transport = "stdio"
command = "qmd"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "bb-pm"
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.run_as, LlmRunAs::Agent);
        assert_eq!(resolved.runtime, "opencode");
        assert_eq!(resolved.agent, "bb-pm");
        assert_eq!(resolved.model.as_deref(), Some("model-a"));
        assert_eq!(resolved.variant.as_deref(), Some("high"));
        assert_eq!(resolved.prompt, "profile hello");
        assert_eq!(resolved.skills, vec!["triage"]);
        assert_eq!(resolved.mcp_servers[0].name, "qmd");
    }

    #[test]
    fn agent_mode_profile_variant_becomes_codex_reasoning_effort_config() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "codex-worker"
display_name = "Codex Worker"
kind = "platform_agent"
runtime = "codex"
model = "gpt-5.4-mini"
variant = "low"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "codex-worker",
            "prompt": { "mode": "inline", "template": "task" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.model.as_deref(), Some("gpt-5.4-mini"));
        assert_eq!(resolved.variant.as_deref(), Some("low"));
        assert!(resolved
            .codex_config_args
            .iter()
            .any(|arg| arg == r#"model_reasoning_effort="low""#));
    }

    #[test]
    fn agent_mode_inline_prompt_overrides_profile_default() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents/prompts")).unwrap();
        fs::write(root.join("agents/prompts/bb-pm.md"), "profile default").unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
instructions_path = "agents/prompts/bb-pm.md"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "bb-pm",
            "prompt": { "mode": "inline", "template": "node {{inputs.topic}}" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.prompt, "node hello");
    }

    #[test]
    fn agent_mode_inline_prompt_allows_profile_without_default_prompt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "bb-pm",
            "prompt": { "mode": "inline", "template": "do {{inputs.topic}}" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.prompt, "do hello");
    }

    #[test]
    fn agent_mode_file_prompt_reads_node_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents/prompts")).unwrap();
        fs::write(root.join("agents/prompts/task.md"), "file {{inputs.topic}}").unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "bb-pm",
            "prompt": { "mode": "file", "template": "agents/prompts/task.md" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap();

        assert_eq!(resolved.prompt, "file hello");
    }

    #[test]
    fn explicit_bb_mcp_overrides_default_codex_mcp() {
        let tmp = TempDir::new().unwrap();
        let args = build_codex_mcp_config_args(
            tmp.path(),
            "native",
            &[McpServerConfig {
                name: "bb".to_string(),
                transport: "stdio".to_string(),
                command: Some("custom-bb".to_string()),
                args: vec!["stdio".to_string()],
                url: None,
                env: BTreeMap::new(),
            }],
        );

        let bb_arg = args
            .iter()
            .find(|arg| arg.starts_with("mcp_servers.bb="))
            .expect("bb MCP config arg");
        assert!(bb_arg.contains(r#"command = "custom-bb""#));
        assert!(bb_arg.contains(r#"args = ["stdio"]"#));
        assert!(!bb_arg.contains("--root"));
        assert!(!bb_arg.contains("url"));
    }

    #[test]
    fn explicit_bb_mcp_overrides_default_codebuddy_mcp() {
        let tmp = TempDir::new().unwrap();
        let content = build_codebuddy_mcp_config_content(
            tmp.path(),
            "native",
            &[McpServerConfig {
                name: "bb".to_string(),
                transport: "stdio".to_string(),
                command: Some("custom-bb".to_string()),
                args: vec!["stdio".to_string()],
                url: None,
                env: BTreeMap::new(),
            }],
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bb = &parsed["mcpServers"]["bb"];

        assert_eq!(bb["type"], "stdio");
        assert_eq!(bb["command"], "custom-bb");
        assert_eq!(bb["args"][0], "stdio");
        assert!(bb.get("url").is_none());
    }

    #[test]
    fn codebuddy_mcp_config_expands_bb_root_placeholders() {
        let tmp = TempDir::new().unwrap();
        let content = build_codebuddy_mcp_config_content(
            tmp.path(),
            "native",
            &[McpServerConfig {
                name: "bb".to_string(),
                transport: "stdio".to_string(),
                command: Some("custom-bb".to_string()),
                args: vec![
                    "--root".to_string(),
                    "<bb-root>".to_string(),
                    "stdio".to_string(),
                ],
                url: None,
                env: BTreeMap::new(),
            }],
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bb = &parsed["mcpServers"]["bb"];

        assert_eq!(bb["command"], "custom-bb");
        assert_eq!(bb["args"][1], tmp.path().display().to_string());
    }

    #[test]
    fn agent_mode_requires_task_prompt_or_profile_default_prompt() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
"#,
        )
        .unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "run_as": "agent",
            "agent_profile": "bb-pm"
        }))
        .unwrap();

        let err =
            resolve_llm_invocation(&opts(root, "blackboard"), &config, &context()).unwrap_err();

        assert!(err.to_string().contains("node task prompt is empty"));
    }
}
