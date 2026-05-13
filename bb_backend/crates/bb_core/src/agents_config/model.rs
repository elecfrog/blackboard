use serde::{Deserialize, Serialize};

use super::AgentMcpConfigFormat;

/// Connector type: determines how the source content is written to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentConnectorType {
    /// Plain copy of the source file (AGENTS.md style).
    AgentsMd,
    /// Wrap the source content in a CodeBuddy Rules YAML frontmatter header.
    Rules,
    /// Upsert an MCP server entry into the Agent's per-user MCP config file.
    /// Unlike `AgentsMd` / `Rules`, this does NOT overwrite the whole target
    /// file — it merges into the existing config object, preserving every
    /// unrelated key (tools, permissions, commands, other MCP servers …).
    McpServer,
}

/// Declarative description of an MCP server injection target attached to an
/// Agent connector. Built into the static catalog; resolved into a normal
/// `ResolvedConnectorTargetSpec` at runtime.
#[derive(Debug, Clone, Copy)]
pub struct AgentMcpTargetSpec {
    pub label: &'static str,
    pub target_template: &'static str,
    pub format: AgentMcpConfigFormat,
    pub server_id: &'static str,
    pub server_description: &'static str,
    pub remote_url: &'static str,
}

/// The one MCP server Blackboard currently injects into every Agent config.
/// Remote-HTTP only: stdio is intentionally not distributed (see ticket 000006
/// for the rationale).
pub const BB_MCP_TARGET: AgentMcpTargetSpec = AgentMcpTargetSpec {
    label: "MCP (bb)",
    target_template: "", // overridden per-Agent below
    format: AgentMcpConfigFormat::CodexToml,
    server_id: "bb",
    server_description: "Blackboard local MCP",
    remote_url: "http://127.0.0.1:3001/mcp",
};

/// Static catalog entry for a supported Agent connector.
#[derive(Debug, Clone, Copy)]
pub struct AgentConnectorSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub targets: &'static [AgentConnectorTargetSpec],
    /// Optional MCP server injection target for this Agent. When `Some`, the
    /// connector gains an extra `McpServer`-typed target that writes into the
    /// Agent's per-user MCP config file.
    pub mcp_target: Option<AgentMcpTargetSpec>,
}

/// One file target owned by an Agent connector.
#[derive(Debug, Clone, Copy)]
pub struct AgentConnectorTargetSpec {
    pub label: &'static str,
    /// Target path template using either an absolute path or one prefixed
    /// with `~/` (which expands to `$HOME`) or `<bb-root>/` (which expands
    /// to the workspace root).
    pub target_template: &'static str,
    /// How the source content is written to the target.
    pub connector_type: AgentConnectorType,
}

/// First-version catalog. Order is preserved for UI listing.
pub const AGENT_CONNECTORS: &[AgentConnectorSpec] = &[
    AgentConnectorSpec {
        id: "codex",
        display_name: "Codex",
        targets: &[AgentConnectorTargetSpec {
            label: "AGENTS.md",
            target_template: "~/.codex/AGENTS.md",
            connector_type: AgentConnectorType::AgentsMd,
        }],
        mcp_target: Some(AgentMcpTargetSpec {
            label: "MCP (bb)",
            target_template: "~/.codex/config.toml",
            format: AgentMcpConfigFormat::CodexToml,
            server_id: BB_MCP_TARGET.server_id,
            server_description: BB_MCP_TARGET.server_description,
            remote_url: BB_MCP_TARGET.remote_url,
        }),
    },
    AgentConnectorSpec {
        id: "codebuddy",
        display_name: "CodeBuddy",
        targets: &[
            AgentConnectorTargetSpec {
                label: "AGENTS.md",
                target_template: "~/.codebuddy/AGENTS.md",
                connector_type: AgentConnectorType::AgentsMd,
            },
            AgentConnectorTargetSpec {
                label: "Rules.mdc",
                target_template: "~/.codebuddy/rules/blackboard-rules.mdc",
                connector_type: AgentConnectorType::Rules,
            },
        ],
        mcp_target: Some(AgentMcpTargetSpec {
            label: "MCP (bb)",
            target_template: "~/.codebuddy/mcp.json",
            format: AgentMcpConfigFormat::CodebuddyJson,
            server_id: BB_MCP_TARGET.server_id,
            server_description: BB_MCP_TARGET.server_description,
            remote_url: BB_MCP_TARGET.remote_url,
        }),
    },
    AgentConnectorSpec {
        id: "opencode",
        display_name: "OpenCode",
        targets: &[AgentConnectorTargetSpec {
            label: "AGENTS.md",
            target_template: "~/.config/opencode/AGENTS.md",
            connector_type: AgentConnectorType::AgentsMd,
        }],
        mcp_target: Some(AgentMcpTargetSpec {
            label: "MCP (bb)",
            target_template: "~/.config/opencode/opencode.json",
            format: AgentMcpConfigFormat::OpencodeJson,
            server_id: BB_MCP_TARGET.server_id,
            server_description: BB_MCP_TARGET.server_description,
            remote_url: BB_MCP_TARGET.remote_url,
        }),
    },
];

/// Per-connector state machine. Mirrors the spec in ticket `000002` §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentConnectorState {
    /// Target exists and `sha256(target) == sha256(source)`.
    Synced,
    /// Target exists but content differs from source.
    Drift,
    /// Parent directory exists, target file does not.
    Missing,
    /// Parent directory itself does not exist.
    Unreachable,
    /// `<bb-root>/agents/AGENTS.md` itself is missing.
    SourceMissing,
}

/// Top-level state of the source-of-truth file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceState {
    /// Source file exists and is readable.
    Present,
    /// Source file is missing (every connector reports `source_missing`).
    Missing,
}

/// Detailed view of a single Agent connector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConnector {
    pub id: String,
    pub display_name: String,
    /// Backward-compatible primary target fields. Multi-target connectors
    /// expose the full list in `targets`.
    pub target_template: String,
    /// Expanded absolute target path (after `~` / `<bb-root>` substitution).
    pub target_path: String,
    pub connector_type: AgentConnectorType,
    pub state: AgentConnectorState,
    /// First 16 hex chars of `sha256(source)`, when source is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256_short: Option<String>,
    /// First 16 hex chars of `sha256(target)`, when target is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sha256_short: Option<String>,
    /// Target mtime as RFC3339 UTC timestamp, when target is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_mtime: Option<String>,
    /// True when the target path is currently a symlink. Sync still
    /// overwrites it with a regular file.
    pub is_symlink: bool,
    pub targets: Vec<AgentConnectorTarget>,
    /// Side-effects surfaced by the most recent `sync_agent_connector` call.
    /// Always empty on `list` / `inspect` paths — these are **transient**
    /// records produced at sync time (e.g. "we flattened a user-managed
    /// symlink into a regular file"). Kept structured on purpose instead of
    /// being logged to stderr, so the bb-server HTTP / MCP consumers (and
    /// ultimately the Settings UI and Agent transcripts) can show them to
    /// the human without scraping logs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AgentConnectorSyncEvent>,
}

/// Detailed view of one file target inside an Agent connector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConnectorTarget {
    pub label: String,
    pub target_template: String,
    pub target_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub connector_type: AgentConnectorType,
    pub state: AgentConnectorState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sha256_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_mtime: Option<String>,
    pub is_symlink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Per-target side-effects from the most recent `sync` call. See the
    /// comment on `AgentConnector::events` for the wire-contract rationale.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AgentConnectorSyncEvent>,
}

/// Structured event emitted by `sync_agent_connector` to describe a
/// user-visible side-effect of the sync. Serialised with an internally
/// tagged `type` discriminator so new variants can be added without
/// breaking existing consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentConnectorSyncEvent {
    /// The target path was a symlink and `sync` replaced it with a regular
    /// file (the canonical bb-managed copy). `previous_link_target` records
    /// what the symlink used to point at — best-effort, `None` if the link
    /// could not be read back. The strict "flatten on sync" semantics are
    /// unchanged; this event only surfaces the fact so callers don't see it
    /// happen silently.
    FlattenedSymlink {
        target_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        previous_link_target: Option<String>,
    },
}

/// Aggregate listing returned by `list_agent_connectors`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConnectorList {
    pub source_state: AgentSourceState,
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256_short: Option<String>,
    pub connectors: Vec<AgentConnector>,
}
