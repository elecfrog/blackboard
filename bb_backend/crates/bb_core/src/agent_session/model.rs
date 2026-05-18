use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolPolicyMode {
    Off,
    #[default]
    Block,
    Ask,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBashToolPolicy {
    /// Regex allow rules kept for compatibility with existing graphs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    /// Regex deny rules kept for compatibility with existing graphs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    /// Shell-style wildcard allow rules, for example `cargo clippy*`.
    #[serde(
        default,
        alias = "white_list",
        alias = "allowlist",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub whitelist: Vec<String>,
    /// Shell-style wildcard deny rules. Deny/blacklist rules always win.
    #[serde(
        default,
        alias = "black_list",
        alias = "denylist",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub blacklist: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMcpToolPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolPolicy {
    #[serde(default)]
    pub mode: AgentToolPolicyMode,
    /// `None` leaves Pi's active tool set unchanged; `Some([])` disables all tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Write/edit roots accepted by the Blackboard Pi policy extension.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_roots: Vec<String>,
    /// Write/edit roots always denied by the Blackboard Pi policy extension.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_deny_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "AgentBashToolPolicy::is_empty")]
    pub bash: AgentBashToolPolicy,
    #[serde(default, skip_serializing_if = "AgentMcpToolPolicy::is_empty")]
    pub mcp: AgentMcpToolPolicy,
}

impl Default for AgentToolPolicy {
    fn default() -> Self {
        Self {
            mode: AgentToolPolicyMode::Block,
            allowed_tools: None,
            write_roots: Vec::new(),
            write_deny_roots: Vec::new(),
            bash: AgentBashToolPolicy::default(),
            mcp: AgentMcpToolPolicy::default(),
        }
    }
}

impl AgentBashToolPolicy {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.allow.is_empty()
            && self.deny.is_empty()
            && self.whitelist.is_empty()
            && self.blacklist.is_empty()
    }
}

impl AgentMcpToolPolicy {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.allow.is_none() && self.deny.is_empty()
    }
}

impl AgentToolPolicy {
    #[must_use]
    pub fn is_off(&self) -> bool {
        self.mode == AgentToolPolicyMode::Off
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Pending,
    Running,
    Idle,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResultStatus {
    Completed,
    Failed,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventType {
    Status,
    Text,
    Thinking,
    ToolUse,
    ToolResult,
    Error,
    Log,
    UsageUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentSessionParent {
    TaskGraphNode { run_id: String, node_id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

impl TokenUsage {
    pub const fn add_assign(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_write_tokens == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub status: AgentSessionStatus,
    pub runtime: String,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<AgentSessionParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub event_count: u64,
    #[serde(default)]
    pub tool_count: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub usage: BTreeMap<String, TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionSummary {
    pub id: String,
    pub status: AgentSessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub event_count: u64,
    pub tool_count: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub usage: BTreeMap<String, TokenUsage>,
    pub updated_at: String,
}

impl From<&AgentSession> for AgentSessionSummary {
    fn from(session: &AgentSession) -> Self {
        Self {
            id: session.id.clone(),
            status: session.status,
            provider_session_id: session.provider_session_id.clone(),
            event_count: session.event_count,
            tool_count: session.tool_count,
            usage: session.usage.clone(),
            updated_at: session.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub seq: u64,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: AgentEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub usage: BTreeMap<String, TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub status: AgentResultStatus,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub usage: BTreeMap<String, TokenUsage>,
}
