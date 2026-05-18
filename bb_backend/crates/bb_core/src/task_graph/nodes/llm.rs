//! LLM node resolver and provider config helpers.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::agent_session::model::AgentToolPolicy;
use crate::agents_registry::{self, AgentProfile, McpServerConfig};

use super::eval::render_prompt_template;
use crate::task_graph::definition::types::{LlmConfig, LlmRunAs, TaskGraphError, KNOWN_RUNTIMES};
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::RunContext;
use crate::task_graph::tool_layer::{
    apply_tool_prompt_preludes, merge_mcp_servers, merge_strings, render_runtime_tool_config,
    resolve_tool_injection_plan, McpServerToolOptions,
};

pub use crate::task_graph::tool_layer::{
    build_codebuddy_mcp_config_content, build_codebuddy_settings_json, build_codex_mcp_config_args,
    build_opencode_task_graph_config, build_pi_mcp_adapter_config_content,
};

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
    pub tool_policy: Option<AgentToolPolicy>,
    pub opencode_config_content: Option<String>,
    pub codex_config_args: Vec<String>,
    pub codebuddy_mcp_config_content: Option<String>,
    pub codebuddy_settings_json: Option<String>,
    pub pi_mcp_config_content: Option<String>,
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
    let tool_plan =
        resolve_tool_injection_plan(&opts.workspace_root, &runtime, &agent, &config.toolkits)?;
    let prompt = apply_tool_prompt_preludes(prompt, &tool_plan.prompt_preludes);
    let skills = merge_strings(
        &merge_strings(&opts.skills, &tool_plan.skills),
        &config.skills,
    );
    let mut mcp_servers = opts.mcp_servers.clone();
    merge_mcp_servers(&mut mcp_servers, &tool_plan.mcp_servers);
    merge_mcp_servers(&mut mcp_servers, &config.mcp_servers);
    let mut custom_env = opts.custom_env.clone();
    custom_env.extend(config.custom_env.clone());
    custom_env = render_env_map(opts, context, config.inputs.as_ref(), custom_env);
    let mut custom_args = opts.custom_args.clone();
    custom_args.extend(config.custom_args.clone());
    let tool_policy =
        render_tool_policy(opts, context, config.inputs.as_ref(), &config.tool_policy);

    Ok(build_resolved_invocation(
        opts,
        LlmRunAs::Llm,
        runtime,
        agent,
        opts.model.clone().or_else(|| config.model.clone()),
        config.variant.clone(),
        prompt,
        config
            .output_contract
            .clone()
            .or_else(|| config.output.clone()),
        skills,
        mcp_servers,
        tool_plan.mcp_server_options,
        custom_env,
        custom_args,
        tool_policy,
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
    let tool_plan = resolve_tool_injection_plan(
        &opts.workspace_root,
        &runtime,
        &profile.id,
        &config.toolkits,
    )?;
    let prompt = apply_tool_prompt_preludes(prompt, &tool_plan.prompt_preludes);

    let mut custom_env = opts.custom_env.clone();
    custom_env.extend(profile.custom_env.clone());
    custom_env.extend(config.custom_env.clone());
    custom_env = render_env_map(opts, context, config.inputs.as_ref(), custom_env);
    let mut custom_args = opts.custom_args.clone();
    custom_args.extend(profile.custom_args.clone());
    custom_args.extend(config.custom_args.clone());
    let tool_policy =
        render_tool_policy(opts, context, config.inputs.as_ref(), &config.tool_policy);

    Ok(build_resolved_invocation(
        opts,
        LlmRunAs::Agent,
        runtime,
        profile.id.clone(),
        opts.model.clone().or_else(|| profile.model.clone()),
        profile.variant.clone(),
        prompt,
        config
            .output_contract
            .clone()
            .or_else(|| config.output.clone()),
        {
            let mut skills = merge_strings(&profile.skills, &tool_plan.skills);
            skills = merge_strings(&skills, &config.skills);
            skills
        },
        {
            let mut mcp_servers = profile.mcp_servers.clone();
            merge_mcp_servers(&mut mcp_servers, &tool_plan.mcp_servers);
            merge_mcp_servers(&mut mcp_servers, &config.mcp_servers);
            mcp_servers
        },
        tool_plan.mcp_server_options,
        custom_env,
        custom_args,
        tool_policy,
    ))
}

fn render_env_map(
    opts: &RunnerOptions,
    context: &RunContext,
    node_inputs: Option<&serde_json::Value>,
    env: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    env.into_iter()
        .map(|(key, value)| {
            (
                key,
                render_prompt_template(
                    &value,
                    &opts.project,
                    &opts.workspace_root,
                    &opts.scripts_dir,
                    context,
                    node_inputs,
                ),
            )
        })
        .collect()
}

fn render_tool_policy(
    opts: &RunnerOptions,
    context: &RunContext,
    node_inputs: Option<&serde_json::Value>,
    policy: &Option<AgentToolPolicy>,
) -> Option<AgentToolPolicy> {
    policy.as_ref().map(|policy| {
        let render = |value: &String| {
            render_prompt_template(
                value,
                &opts.project,
                &opts.workspace_root,
                &opts.scripts_dir,
                context,
                node_inputs,
            )
        };
        let mut rendered = policy.clone();
        rendered.write_roots = rendered.write_roots.iter().map(render).collect();
        rendered.write_deny_roots = rendered.write_deny_roots.iter().map(render).collect();
        rendered.bash.allow = rendered.bash.allow.iter().map(render).collect();
        rendered.bash.deny = rendered.bash.deny.iter().map(render).collect();
        rendered.bash.whitelist = rendered.bash.whitelist.iter().map(render).collect();
        rendered.bash.blacklist = rendered.bash.blacklist.iter().map(render).collect();
        if let Some(allow) = rendered.mcp.allow.as_mut() {
            *allow = allow.iter().map(render).collect();
        }
        rendered.mcp.deny = rendered.mcp.deny.iter().map(render).collect();
        rendered
    })
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
    mcp_server_options: BTreeMap<String, McpServerToolOptions>,
    custom_env: BTreeMap<String, String>,
    custom_args: Vec<String>,
    tool_policy: Option<AgentToolPolicy>,
) -> ResolvedLlmInvocation {
    let tool_render = render_runtime_tool_config(
        &opts.workspace_root,
        &agent,
        &mcp_servers,
        &mcp_server_options,
        variant.as_deref(),
        opts.opencode_config_content.clone(),
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
        tool_policy,
        opencode_config_content: tool_render.opencode_config_content,
        codex_config_args: tool_render.codex_config_args,
        codebuddy_mcp_config_content: tool_render.codebuddy_mcp_config_content,
        codebuddy_settings_json: tool_render.codebuddy_settings_json,
        pi_mcp_config_content: tool_render.pi_mcp_config_content,
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
            "unsupported LLM runtime `{runtime}`; use `codex`, `opencode`, `codebuddy`, or `pi`"
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;
    use crate::task_graph::tool_layer::codex_mcp_server_from_config;

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
            pi_path: "pi".to_string(),
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
    fn llm_custom_env_templates_are_rendered() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "pi",
            "agent": "native",
            "custom_env": {
                "CARGO_TARGET_DIR": "{{env.workspace}}/runtime/cargo-target/{{inputs.topic}}"
            },
            "prompt": { "mode": "inline", "template": "say hi" }
        }))
        .unwrap();

        let resolved =
            resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context()).unwrap();
        let target_dir = resolved
            .custom_env
            .get("CARGO_TARGET_DIR")
            .unwrap()
            .replace('\\', "/");

        assert!(target_dir.ends_with("/runtime/cargo-target/hello"));
        assert!(target_dir.starts_with(&tmp.path().display().to_string().replace('\\', "/")));
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
    fn codex_mcp_server_from_config_reads_node_repl() {
        let server = codex_mcp_server_from_config(
            r#"
[mcp_servers.node_repl]
command = "node_repl.exe"
args = []

[mcp_servers.node_repl.env]
BROWSER_USE_AVAILABLE_BACKENDS = "chrome,iab"
CODEX_HOME = "C:\\Users\\demo\\.codex"
"#,
            "node_repl",
        )
        .expect("node_repl config");

        assert_eq!(server.name, "node_repl");
        assert_eq!(server.transport, "stdio");
        assert_eq!(server.command.as_deref(), Some("node_repl.exe"));
        assert_eq!(
            server
                .env
                .get("BROWSER_USE_AVAILABLE_BACKENDS")
                .map(String::as_str),
            Some("chrome,iab")
        );
    }

    #[test]
    fn browser_use_toolkit_rejects_non_codex_runtime() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "pi",
            "agent": "native",
            "toolkits": ["browser_use"],
            "prompt": { "mode": "inline", "template": "check browser" }
        }))
        .unwrap();

        let err = resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context())
            .unwrap_err();

        assert!(err.to_string().contains("browser_use"));
        assert!(err.to_string().contains("codex"));
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
    fn pi_mcp_adapter_config_uses_project_pi_shape() {
        let tmp = TempDir::new().unwrap();
        let config: LlmConfig = serde_json::from_value(serde_json::json!({
            "runtime": "pi",
            "agent": "native",
            "prompt": { "mode": "inline", "template": "hello" }
        }))
        .unwrap();
        let resolved =
            resolve_llm_invocation(&opts(tmp.path(), "blackboard"), &config, &context()).unwrap();
        let content = resolved.pi_mcp_config_content.expect("pi mcp config");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bb = &parsed["mcpServers"]["bb"];

        assert_eq!(parsed["settings"]["toolPrefix"], "server");
        assert_eq!(bb["lifecycle"], "lazy");
        assert!(!bb["command"].as_str().unwrap().is_empty());
        assert_eq!(bb["args"][0], "--root");
        assert_eq!(bb["args"][1], tmp.path().display().to_string());
        assert_eq!(bb["directTools"][0], "list_projects");
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
