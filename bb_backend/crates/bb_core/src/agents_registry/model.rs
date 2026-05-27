use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRegistrySourceState {
    Present,
    Missing,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentRegistryFile {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub runtimes: Vec<RuntimeProfile>,
    #[serde(default, alias = "employees")]
    pub agents: Vec<AgentProfile>,
    #[serde(default, alias = "project_memberships")]
    pub project_agents: Vec<ProjectAgentRegistration>,
}

/// Runtime profile — represents an available execution runtime (e.g. codex, opencode).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfile {
    pub id: String,
    pub display_name: String,
    #[serde(default = "default_assignable")]
    pub assignable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// MCP server configuration for per-agent MCP injection (Ticket #000049).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct McpServerConfig {
    /// Server identifier name (kebab-case).
    pub name: String,
    /// Transport type: "stdio" or "sse".
    pub transport: String,
    /// Command to start the MCP server (required for stdio transport).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Arguments passed to the MCP server command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// URL for SSE transport (required for sse transport).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Environment variables injected into the MCP server process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfile {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_assignable")]
    pub assignable: bool,
    #[serde(default)]
    pub distribute: bool,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,

    // ── Runtime binding (Ticket #000048) ──
    /// Default LLM model identifier, e.g. "minimax/MiniMax-M2.7-highspeed".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Model variant / reasoning effort, e.g. "high" or "xhigh".
    /// Accepts legacy `model_reasoning_effort` on input and normalizes to `variant`.
    #[serde(alias = "model_reasoning_effort", alias = "reasoning_effort")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Inline system instructions for this agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Path (relative to `bb_root`) to a Markdown file containing system instructions.
    /// Mutually exclusive with `instructions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_path: Option<String>,

    // ── Execution config (Ticket #000048) ──
    /// Extra environment variables injected when running this agent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_env: BTreeMap<String, String>,
    /// Extra CLI arguments appended when spawning this agent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_args: Vec<String>,
    /// Maximum number of concurrent tasks this agent may run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_tasks: Option<u32>,

    // ── MCP servers (Ticket #000049) ──
    /// Per-agent MCP server declarations. Daemon injects these based on runtime type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServerConfig>,

    // ── Skills (Ticket #000050) ──
    /// Skill names this agent can use. References skills in `<bb_root>/skills/{name}/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectAgentRegistration {
    pub project: String,
    #[serde(alias = "employee")]
    pub agent: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub lanes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RemovedProjectAgentRegistration {
    pub project: String,
    pub agent: String,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AgentRegistryList {
    pub source_state: AgentRegistrySourceState,
    pub source_path: String,
    pub runtimes: Vec<RuntimeProfile>,
    pub agents: Vec<AgentProfile>,
    pub project_agents: Vec<ProjectAgentRegistration>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectAgentList {
    pub source_state: AgentRegistrySourceState,
    pub source_path: String,
    pub project: String,
    pub agents: Vec<ProjectAgentProfile>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectAgentProfile {
    #[serde(flatten)]
    pub agent: AgentProfile,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_role: Option<String>,
    pub project_lanes: Vec<String>,
}

pub(super) fn default_scope() -> String {
    "global".to_string()
}

pub(super) fn default_status() -> String {
    "active".to_string()
}

const fn default_assignable() -> bool {
    true
}
