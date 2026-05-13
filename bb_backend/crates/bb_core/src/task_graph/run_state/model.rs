use serde::{Deserialize, Serialize};

use crate::agent_session::AgentSessionSummary;

use super::super::types::{TaskGraphDefinition, TaskGraphScope};

// ─── Types ───────────────────────────────────────────────────────────────────

/// Reference to the graph that spawned this run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRef {
    pub scope: TaskGraphScope,
    pub id: String,
    pub version: u32,
}

/// Run-level status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

/// Node-level run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRunStatus {
    Idle,
    Queued,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Paused,
}

/// Paused state metadata (human gate / manual pause).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPaused {
    pub node_id: String,
    pub reason: String,
    pub actions: Vec<PausedAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PausedAction {
    pub id: String,
    pub label: String,
    pub result: String,
}

/// Run context — tracks inputs, outputs, branch decisions, loop iterations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContext {
    pub input: serde_json::Value,
    pub node_outputs: serde_json::Map<String, serde_json::Value>,
    pub branch_decisions: Vec<BranchDecision>,
    pub loop_iterations: Vec<LoopIterationState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loop_stack: Vec<LoopFrame>,
    /// 并行 fork/join 追踪：key 为 join 点节点 ID，value 为已完成到达该 join 点的上游节点 ID 列表。
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub completed_branches: std::collections::HashMap<String, Vec<String>>,
}

/// Interpreter-private continuation frame for a running loop body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopFrame {
    pub loop_node_id: String,
    pub iteration: u32,
    pub body_entry_node_id: String,
    pub started_at: String,
}

/// A single branch evaluation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDecision {
    pub node_id: String,
    pub selected_rule_id: String,
    pub selected_edge_id: String,
    pub evaluated_at: String,
}

/// Loop iteration tracking for one loop node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIterationState {
    pub loop_node_id: String,
    pub current_iteration: u32,
    pub max_iterations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
    pub history: Vec<LoopIterationEntry>,
}

/// A single loop iteration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIterationEntry {
    pub iteration: u32,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub result: LoopIterationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopIterationResult {
    Continued,
    Exited,
    Failed,
}

/// Per-node run state stored in `nodes/<node-id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphRunNode {
    pub node_id: String,
    pub status: NodeRunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NodeError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_artifact: Option<OutputArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    /// If this node is a sub_graph node, this is the child run ID it spawned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_run_id: Option<String>,
    /// Runtime used for execution (e.g. "opencode", "codex").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Agent profile used (e.g. "bb-pm", "codex", "native").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Model used for LLM execution (e.g. "claude-sonnet-4-20250514").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// AgentSession backing this node when it is executed through the shared runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session_id: Option<String>,
    /// Lightweight AgentSession summary for UI rendering without loading events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session: Option<AgentSessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputArtifact {
    pub id: String,
    pub path: String,
    pub content_type: ArtifactContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactContentType {
    Markdown,
    Json,
    Text,
}

/// Run metadata stored in `run.json` (does NOT include graph snapshot or nodes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphRun {
    pub id: String,
    pub project: String,
    pub graph_ref: GraphRef,
    pub status: RunStatus,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<RunPaused>,
    pub cursor: Vec<String>,
    pub context: RunContext,
    /// If this run is a child run invoked by a sub_graph node, this is the parent run ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
}

/// Aggregated run detail (run.json + graph.snapshot.json + nodes/*).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphRunDetail {
    #[serde(flatten)]
    pub run: TaskGraphRun,
    pub graph_snapshot: TaskGraphDefinition,
    pub nodes: Vec<TaskGraphRunNode>,
}

/// Lightweight summary for listing runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphRunSummary {
    pub id: String,
    pub project: String,
    pub graph_ref: GraphRef,
    pub status: RunStatus,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}
