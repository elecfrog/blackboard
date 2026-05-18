//! Provider-neutral tool capability layer for TaskGraph LLM nodes.
//!
//! Graph nodes declare toolkits as product-level capabilities. This layer
//! resolves those declarations into a neutral injection plan, then renders the
//! provider-specific config required by Codex, Pi, OpenCode, and CodeBuddy.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents_registry::McpServerConfig;
use crate::fs_util::resolve_slash;

use super::definition::types::{LlmToolkitConfig, TaskGraphError};

pub const TOOLKIT_BLACKBOARD_MCP: &str = "blackboard_mcp";
pub const TOOLKIT_BROWSER_USE: &str = "browser_use";
pub const BROWSER_USE_DEFAULT_BACKENDS: &str = "iab,chrome";
pub const BROWSER_USE_DEFAULT_BACKEND_TIMEOUT_MS: &str = "15000";
pub const KNOWN_LLM_TOOLKITS: &[&str] = &[TOOLKIT_BLACKBOARD_MCP, TOOLKIT_BROWSER_USE];
pub const DEFAULT_BLACKBOARD_MCP_DIRECT_TOOLS: &[&str] = &[
    "list_projects",
    "create_inbox_note",
    "read_ticket_by_id",
    "search_tickets",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolTransportKind {
    Mcp,
    Cli,
    Rest,
    Shell,
}

#[derive(Debug, Clone)]
pub struct ToolEndpointSpec {
    pub id: String,
    pub transport: ToolTransportKind,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct McpServerToolOptions {
    pub direct_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolInjectionPlan {
    pub skills: Vec<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub mcp_server_options: BTreeMap<String, McpServerToolOptions>,
    pub mcp_tools: Vec<ToolEndpointSpec>,
    pub cli_tools: Vec<ToolEndpointSpec>,
    pub rest_tools: Vec<ToolEndpointSpec>,
    pub shell_tools: Vec<ToolEndpointSpec>,
    pub prompt_preludes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeToolRender {
    pub opencode_config_content: Option<String>,
    pub codex_config_args: Vec<String>,
    pub codebuddy_mcp_config_content: Option<String>,
    pub codebuddy_settings_json: Option<String>,
    pub pi_mcp_config_content: Option<String>,
}

pub fn resolve_tool_injection_plan(
    root: &Path,
    runtime: &str,
    agent: &str,
    toolkits: &[LlmToolkitConfig],
) -> Result<ToolInjectionPlan, TaskGraphError> {
    let mut plan = ToolInjectionPlan::default();
    let mut seen = BTreeSet::new();
    for toolkit in normalized_toolkits(toolkits) {
        let id = toolkit.id();
        if id.is_empty() || !seen.insert(id.to_string()) {
            continue;
        }
        match id {
            TOOLKIT_BLACKBOARD_MCP => {
                inject_blackboard_mcp_toolkit(root, agent, &toolkit, &mut plan)?
            }
            TOOLKIT_BROWSER_USE => inject_browser_use_toolkit(runtime, &mut plan)?,
            other => {
                return Err(TaskGraphError::InvalidGraphId(format!(
                    "unsupported LLM toolkit `{other}`"
                )));
            }
        }
    }
    Ok(plan)
}

#[derive(Debug, Clone)]
struct NormalizedToolkit {
    id: String,
    exposure: Option<String>,
    tools: Vec<String>,
}

impl NormalizedToolkit {
    fn id(&self) -> &str {
        self.id.trim()
    }
}

fn normalized_toolkits(toolkits: &[LlmToolkitConfig]) -> Vec<NormalizedToolkit> {
    let mut normalized: Vec<NormalizedToolkit> = toolkits
        .iter()
        .filter_map(|toolkit| {
            let id = toolkit.id().trim();
            if id.is_empty() {
                return None;
            }
            Some(NormalizedToolkit {
                id: id.to_string(),
                exposure: toolkit.exposure().map(str::to_string),
                tools: toolkit
                    .tools()
                    .iter()
                    .map(|tool| tool.trim())
                    .filter(|tool| !tool.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
        })
        .collect();
    if !normalized
        .iter()
        .any(|toolkit| toolkit.id() == TOOLKIT_BLACKBOARD_MCP)
    {
        normalized.insert(
            0,
            NormalizedToolkit {
                id: TOOLKIT_BLACKBOARD_MCP.to_string(),
                exposure: Some("direct".to_string()),
                tools: default_blackboard_mcp_direct_tools(),
            },
        );
    }
    normalized
}

pub fn apply_tool_prompt_preludes(prompt: String, prelude: &[String]) -> String {
    if prelude.is_empty() {
        return prompt;
    }
    format!("{}\n\n{}", prelude.join("\n\n"), prompt)
}

pub fn render_runtime_tool_config(
    root: &Path,
    agent: &str,
    mcp_servers: &[McpServerConfig],
    mcp_server_options: &BTreeMap<String, McpServerToolOptions>,
    variant: Option<&str>,
    opencode_fallback: Option<String>,
) -> RuntimeToolRender {
    let opencode_config_content =
        build_opencode_task_graph_config(root, agent, mcp_servers).or(opencode_fallback);
    let mut codex_config_args = build_codex_mcp_config_args(root, agent, mcp_servers);
    if let Some(variant) = variant.map(str::trim).filter(|value| !value.is_empty()) {
        codex_config_args.push(toml_config_arg(
            "model_reasoning_effort",
            toml::Value::String(variant.to_string()),
        ));
    }
    RuntimeToolRender {
        opencode_config_content,
        codex_config_args,
        codebuddy_mcp_config_content: build_codebuddy_mcp_config_content(root, agent, mcp_servers),
        codebuddy_settings_json: build_codebuddy_settings_json(root, agent, mcp_servers, variant),
        pi_mcp_config_content: build_pi_mcp_adapter_config_content_with_options(
            root,
            agent,
            mcp_servers,
            mcp_server_options,
        ),
    }
}

#[must_use]
pub fn merge_strings(base: &[String], extra: &[String]) -> Vec<String> {
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

pub fn merge_mcp_servers(base: &mut Vec<McpServerConfig>, extra: &[McpServerConfig]) {
    for server in extra {
        if let Some(existing) = base.iter_mut().find(|item| item.name == server.name) {
            *existing = server.clone();
        } else {
            base.push(server.clone());
        }
    }
}

#[must_use]
pub fn build_opencode_task_graph_config(
    _root: &Path,
    _agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Option<String> {
    let mut config = read_opencode_config().unwrap_or_else(|| serde_json::json!({}));
    let object = config.as_object_mut()?;
    let mcp = object
        .entry("mcp")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()?;

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
    _root: &Path,
    _agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Vec<String> {
    let mut servers = Vec::new();
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

#[must_use]
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

#[must_use]
pub fn build_pi_mcp_adapter_config_content(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Option<String> {
    build_pi_mcp_adapter_config_content_with_options(
        root,
        agent,
        extra_mcp_servers,
        &BTreeMap::new(),
    )
}

#[must_use]
pub fn build_pi_mcp_adapter_config_content_with_options(
    root: &Path,
    agent: &str,
    extra_mcp_servers: &[McpServerConfig],
    mcp_server_options: &BTreeMap<String, McpServerToolOptions>,
) -> Option<String> {
    let mut mcp_servers = serde_json::Map::new();
    for server in pi_mcp_servers(root, agent, extra_mcp_servers) {
        let name = server.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let options = mcp_server_options.get(&name);
        if let Some(config) = pi_mcp_server_value(server, options) {
            mcp_servers.insert(name, config);
        }
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "settings": {
            "toolPrefix": "server",
            "idleTimeout": 10,
            "directTools": false,
        },
        "mcpServers": mcp_servers,
    }))
    .ok()
}

fn inject_blackboard_mcp_toolkit(
    root: &Path,
    agent: &str,
    toolkit: &NormalizedToolkit,
    plan: &mut ToolInjectionPlan,
) -> Result<(), TaskGraphError> {
    let server = blackboard_mcp_server(root, agent).ok_or_else(|| {
        TaskGraphError::InvalidGraphId(
            "LLM toolkit `blackboard_mcp` could not resolve the current bb executable".to_string(),
        )
    })?;

    let direct_tools = if toolkit.tools.is_empty() {
        default_blackboard_mcp_direct_tools()
    } else {
        toolkit.tools.clone()
    };
    let direct_tools = match toolkit
        .exposure
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("direct")
    {
        "direct" => Some(direct_tools),
        "proxy" => None,
        other => {
            return Err(TaskGraphError::InvalidGraphId(format!(
                "LLM toolkit `blackboard_mcp` exposure `{other}` is not supported; use `direct` or `proxy`"
            )));
        }
    };

    plan.mcp_servers.push(server);
    plan.mcp_server_options
        .insert("bb".to_string(), McpServerToolOptions { direct_tools });
    plan.mcp_tools.push(ToolEndpointSpec {
        id: "blackboard_mcp".to_string(),
        transport: ToolTransportKind::Mcp,
        description: "Blackboard daemon-backed project, ticket, and inbox tools".to_string(),
    });
    Ok(())
}

fn inject_browser_use_toolkit(
    runtime: &str,
    plan: &mut ToolInjectionPlan,
) -> Result<(), TaskGraphError> {
    if runtime != "codex" {
        return Err(TaskGraphError::InvalidGraphId(format!(
            "LLM toolkit `{TOOLKIT_BROWSER_USE}` currently supports runtime `codex` only; got `{runtime}`"
        )));
    }

    let mut server = load_codex_mcp_server("node_repl").ok_or_else(|| {
        TaskGraphError::InvalidGraphId(
            "LLM toolkit `browser_use` requires `mcp_servers.node_repl` in CODEX_HOME/config.toml or ~/.codex/config.toml".to_string(),
        )
    })?;
    normalize_browser_use_mcp_server_env(&mut server);
    let browser_client = find_codex_browser_client().ok_or_else(|| {
        TaskGraphError::InvalidGraphId(
            "LLM toolkit `browser_use` requires the bundled Browser plugin with scripts/browser-client.mjs".to_string(),
        )
    })?;

    plan.mcp_servers.push(server);
    plan.prompt_preludes
        .push(browser_use_prompt_prelude(&browser_client));
    plan.mcp_tools.push(ToolEndpointSpec {
        id: "browser_use".to_string(),
        transport: ToolTransportKind::Mcp,
        description: "Codex in-app browser access through node_repl".to_string(),
    });
    Ok(())
}

fn browser_use_prompt_prelude(browser_client: &Path) -> String {
    let browser_client = resolve_slash(browser_client);
    format!(
        r#"[Injected toolkit: browser_use]
This LLM node has Browser Use enabled through the `node_repl` MCP server.
When browser interaction is required, use the Node REPL JavaScript tool and bootstrap the Codex in-app browser with:

```js
if (!globalThis.agent) {{
  const {{ setupBrowserRuntime }} = await import("{browser_client}");
  await setupBrowserRuntime({{ globals: globalThis }});
}}
if (!globalThis.browser) {{
  const browserUseEnv = globalThis.process?.env ?? {{}};
  const backendCandidates = String(browserUseEnv.BROWSER_USE_AVAILABLE_BACKENDS || "{BROWSER_USE_DEFAULT_BACKENDS}")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
  const backendTimeoutMs = Number(browserUseEnv.BROWSER_USE_BACKEND_TIMEOUT_MS || "{BROWSER_USE_DEFAULT_BACKEND_TIMEOUT_MS}");
  async function withBrowserUseTimeout(promise, label) {{
    let timer;
    try {{
      return await Promise.race([
        promise,
        new Promise((_, reject) => {{
          timer = setTimeout(() => reject(new Error(`${{label}} timed out after ${{backendTimeoutMs}}ms`)), backendTimeoutMs);
        }}),
      ]);
    }} finally {{
      if (timer) clearTimeout(timer);
    }}
  }}
  const browserUseErrors = [];
  const browserUseAttempts = [];
  for (const backend of backendCandidates) {{
    browserUseAttempts.push(backend);
    try {{
      globalThis.browser = await withBrowserUseTimeout(agent.browsers.get(backend), `agent.browsers.get(${{backend}})`);
      globalThis.browserBackend = backend;
      break;
    }} catch (error) {{
      browserUseErrors.push(`${{backend}}: ${{error?.message ?? String(error)}}`);
    }}
  }}
  globalThis.browserUseBootstrap = {{
    backend: globalThis.browserBackend ?? null,
    attempted: browserUseAttempts,
    errors: browserUseErrors,
    timeoutMs: backendTimeoutMs,
  }};
  if (!globalThis.browser) {{
    throw new Error(`Browser Use is unavailable: ${{browserUseErrors.join("; ")}}`);
  }}
}}
if (typeof tab === "undefined") {{
  globalThis.tab = await browser.tabs.new();
}}
```

Use `tab.goto`, `tab.playwright.domSnapshot`, console logs, and screenshots as needed. Report `globalThis.browserBackend` as the backend you used.
If Browser Use is unavailable, report it as blocked; do not silently replace it with curl or shell-only checks."#
    )
}

fn normalize_browser_use_mcp_server_env(server: &mut McpServerConfig) {
    server
        .env
        .entry("BROWSER_USE_AVAILABLE_BACKENDS".to_string())
        .or_insert_with(|| BROWSER_USE_DEFAULT_BACKENDS.to_string());
    server
        .env
        .entry("BROWSER_USE_BACKEND_TIMEOUT_MS".to_string())
        .or_insert_with(|| BROWSER_USE_DEFAULT_BACKEND_TIMEOUT_MS.to_string());
}

fn load_codex_mcp_server(name: &str) -> Option<McpServerConfig> {
    let config_path = codex_home()?.join("config.toml");
    let content = fs::read_to_string(config_path).ok()?;
    codex_mcp_server_from_config(&content, name)
}

fn codex_home() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return Some(path);
    }
    if let Some(home) = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return Some(home.join(".codex"));
    }
    std::env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".codex"))
}

fn find_codex_browser_client() -> Option<PathBuf> {
    let browser_cache = codex_home()?
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("browser");
    let mut candidates: Vec<PathBuf> = fs::read_dir(browser_cache)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("scripts").join("browser-client.mjs"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}

pub(crate) fn codex_mcp_server_from_config(content: &str, name: &str) -> Option<McpServerConfig> {
    let doc: toml::Value = content.parse().ok()?;
    let table = doc.get("mcp_servers")?.get(name)?.as_table()?;
    let command = table
        .get("command")
        .and_then(toml_value_to_string)
        .filter(|value| !value.trim().is_empty());
    let url = table
        .get("url")
        .and_then(toml_value_to_string)
        .filter(|value| !value.trim().is_empty());
    let args = table
        .get("args")
        .and_then(toml::Value::as_array)
        .map(|items| items.iter().filter_map(toml_value_to_string).collect())
        .unwrap_or_default();
    let env = table
        .get("env")
        .and_then(toml::Value::as_table)
        .map(|env| {
            env.iter()
                .filter_map(|(key, value)| {
                    toml_value_to_string(value).map(|value| (key.clone(), value))
                })
                .collect()
        })
        .unwrap_or_default();
    let transport = if command.is_some() {
        "stdio"
    } else if url.is_some() {
        "sse"
    } else {
        return None;
    };
    Some(McpServerConfig {
        name: name.to_string(),
        transport: transport.to_string(),
        command,
        args,
        url,
        env,
    })
}

fn toml_value_to_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(value.clone()),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Float(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

fn default_blackboard_mcp_direct_tools() -> Vec<String> {
    DEFAULT_BLACKBOARD_MCP_DIRECT_TOOLS
        .iter()
        .map(|tool| (*tool).to_string())
        .collect()
}

fn blackboard_mcp_server(root: &Path, agent: &str) -> Option<McpServerConfig> {
    let bb_exe = std::env::current_exe().ok()?;
    let mut env = BTreeMap::new();
    env.insert("BB_DAEMON".to_string(), "1".to_string());
    env.insert("BB_DAEMON_AGENT".to_string(), agent.to_string());
    Some(McpServerConfig {
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
    })
}

fn codebuddy_mcp_servers(
    root: &Path,
    _agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Vec<McpServerConfig> {
    let mut servers = Vec::new();
    merge_mcp_servers(&mut servers, extra_mcp_servers);
    servers
        .into_iter()
        .map(|server| normalize_codebuddy_mcp_server(root, server))
        .collect()
}

fn pi_mcp_servers(
    root: &Path,
    _agent: &str,
    extra_mcp_servers: &[McpServerConfig],
) -> Vec<McpServerConfig> {
    let mut servers = Vec::new();
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

fn pi_mcp_server_value(
    server: McpServerConfig,
    options: Option<&McpServerToolOptions>,
) -> Option<serde_json::Value> {
    match server.transport.as_str() {
        "stdio" => {
            let command = server.command.filter(|cmd| !cmd.trim().is_empty())?;
            let mut value = serde_json::json!({
                "command": command,
                "args": server.args,
                "lifecycle": "lazy",
                "idleTimeout": 10,
            });
            if !server.env.is_empty() {
                value["env"] = serde_json::to_value(server.env).ok()?;
            }
            if let Some(direct_tools) = options.and_then(|options| options.direct_tools.as_ref()) {
                value["directTools"] = serde_json::to_value(direct_tools).ok()?;
            }
            Some(value)
        }
        "sse" | "http" | "streamable-http" => {
            server.url.filter(|url| !url.trim().is_empty()).map(|url| {
                let mut value = serde_json::json!({
                    "url": url,
                    "lifecycle": "lazy",
                    "idleTimeout": 10,
                });
                if let Some(direct_tools) =
                    options.and_then(|options| options.direct_tools.as_ref())
                {
                    value["directTools"] = serde_json::to_value(direct_tools)
                        .unwrap_or_else(|_| serde_json::json!([]));
                }
                value
            })
        }
        _ => None,
    }
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
    format!("{key}={value}")
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
    use super::*;

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
    fn browser_use_mcp_env_adds_safe_defaults_without_overriding() {
        let mut server = McpServerConfig {
            name: "node_repl".to_string(),
            transport: "stdio".to_string(),
            command: Some("node_repl".to_string()),
            args: Vec::new(),
            url: None,
            env: BTreeMap::from([(
                "BROWSER_USE_AVAILABLE_BACKENDS".to_string(),
                "chrome,iab".to_string(),
            )]),
        };

        normalize_browser_use_mcp_server_env(&mut server);

        assert_eq!(
            server
                .env
                .get("BROWSER_USE_AVAILABLE_BACKENDS")
                .map(String::as_str),
            Some("chrome,iab")
        );
        assert_eq!(
            server
                .env
                .get("BROWSER_USE_BACKEND_TIMEOUT_MS")
                .map(String::as_str),
            Some(BROWSER_USE_DEFAULT_BACKEND_TIMEOUT_MS)
        );
    }

    #[test]
    fn explicit_bb_mcp_overrides_default_codex_mcp() {
        let tmp = tempfile::TempDir::new().unwrap();
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
        let tmp = tempfile::TempDir::new().unwrap();
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
        let tmp = tempfile::TempDir::new().unwrap();
        let content = build_codebuddy_mcp_config_content(
            tmp.path(),
            "native",
            &[McpServerConfig {
                name: "qmd".to_string(),
                transport: "stdio".to_string(),
                command: Some("<bb-root>/tools/qmd".to_string()),
                args: vec!["--root=<bb-root>".to_string()],
                url: None,
                env: BTreeMap::from([("BB_ROOT".to_string(), "<bb-root>".to_string())]),
            }],
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let qmd = &parsed["mcpServers"]["qmd"];

        assert_eq!(
            qmd["command"],
            format!("{}/tools/qmd", tmp.path().display())
        );
        assert_eq!(qmd["args"][0], format!("--root={}", tmp.path().display()));
        assert_eq!(qmd["env"]["BB_ROOT"], tmp.path().display().to_string());
    }

    #[test]
    fn pi_mcp_adapter_config_uses_project_pi_shape() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toolkits: Vec<LlmToolkitConfig> = Vec::new();
        let plan =
            resolve_tool_injection_plan(tmp.path(), "pi", "native", &toolkits).expect("tool plan");
        let content = build_pi_mcp_adapter_config_content_with_options(
            tmp.path(),
            "native",
            &plan.mcp_servers,
            &plan.mcp_server_options,
        )
        .expect("pi mcp config");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["settings"]["toolPrefix"], "server");
        assert_eq!(parsed["settings"]["directTools"], false);
        assert!(parsed["mcpServers"]["bb"]["command"].is_string());
        assert_eq!(parsed["mcpServers"]["bb"]["lifecycle"], "lazy");
        assert_eq!(
            parsed["mcpServers"]["bb"]["directTools"][0],
            "list_projects"
        );
    }

    #[test]
    fn blackboard_mcp_toolkit_can_override_pi_direct_tools() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toolkits: Vec<LlmToolkitConfig> = serde_json::from_value(serde_json::json!([
            {
                "id": "blackboard_mcp",
                "exposure": "direct",
                "tools": ["create_inbox_note"]
            }
        ]))
        .unwrap();
        let plan =
            resolve_tool_injection_plan(tmp.path(), "pi", "native", &toolkits).expect("tool plan");
        let content = build_pi_mcp_adapter_config_content_with_options(
            tmp.path(),
            "native",
            &plan.mcp_servers,
            &plan.mcp_server_options,
        )
        .expect("pi mcp config");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            parsed["mcpServers"]["bb"]["directTools"],
            serde_json::json!(["create_inbox_note"])
        );
    }

    #[test]
    fn blackboard_mcp_proxy_mode_keeps_pi_tools_behind_server_prefix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toolkits: Vec<LlmToolkitConfig> = serde_json::from_value(serde_json::json!([
            { "id": "blackboard_mcp", "exposure": "proxy" }
        ]))
        .unwrap();
        let plan =
            resolve_tool_injection_plan(tmp.path(), "pi", "native", &toolkits).expect("tool plan");
        let content = build_pi_mcp_adapter_config_content_with_options(
            tmp.path(),
            "native",
            &plan.mcp_servers,
            &plan.mcp_server_options,
        )
        .expect("pi mcp config");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed["mcpServers"]["bb"].get("directTools").is_none());
    }

    #[test]
    fn runtime_tool_render_keeps_provider_specific_outputs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let toolkits: Vec<LlmToolkitConfig> = Vec::new();
        let plan = resolve_tool_injection_plan(tmp.path(), "codex", "native", &toolkits)
            .expect("tool plan");
        let render = render_runtime_tool_config(
            tmp.path(),
            "native",
            &plan.mcp_servers,
            &plan.mcp_server_options,
            Some("high"),
            None,
        );

        assert!(render
            .codex_config_args
            .iter()
            .any(|arg| arg.starts_with("mcp_servers.bb=")));
        assert!(render
            .codex_config_args
            .iter()
            .any(|arg| arg.starts_with("model_reasoning_effort=")));
        assert!(render.pi_mcp_config_content.is_some());
        assert!(render.codebuddy_mcp_config_content.is_some());
        assert!(render.codebuddy_settings_json.is_some());
    }
}
