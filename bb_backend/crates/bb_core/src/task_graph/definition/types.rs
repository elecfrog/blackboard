//! Task Graph domain types — mirrors the MVP contract.

use crate::agents_registry::McpServerConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ─── Graph Definition ────────────────────────────────────────────────────────

/// Top-level graph definition shared by system and project graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphDefinition {
    pub schema_version: u32,
    pub id: String,
    pub scope: TaskGraphScope,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: u32,
    pub readonly: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<GraphOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<GraphMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<TaskGraphInputParam>>,
    pub nodes: Vec<TaskGraphNode>,
    pub edges: Vec<TaskGraphEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<TaskGraphLayout>,
}

/// Scope discriminator: system (readonly, builtin) or project (user-editable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGraphScope {
    System,
    Project,
}

/// Records the origin of a fork from a system graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOrigin {
    pub scope: TaskGraphScope,
    pub id: String,
    pub version: u32,
    pub forked_at: String,
}

/// Optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_tickets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
    /// Optional Pregel recursion limit. When set, the loop fails before
    /// preparing a superstep greater than this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursion_limit: Option<u64>,
    /// Optional LangGraph-style interruptBefore nodes. Use ["*"] for all visible nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_before: Option<Vec<String>>,
    /// Optional LangGraph-style interruptAfter nodes. Use ["*"] for all visible nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_after: Option<Vec<String>>,
}

/// User-configurable graph-level input exposed before a run starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphInputParam {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub value_type: String,
    /// Optional StateGraph-style reducer. Supported values: append, merge_object, sum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reducer: Option<String>,
    /// Optional explicit channel class. Supported values: last_value, any_value,
    /// topic, topic_unique, topic_accumulate, topic_unique_accumulate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_class: Option<String>,
    #[serde(default, rename = "default")]
    pub default_value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Layout hint for the editor viewport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphLayout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

// ─── Pin Model ───────────────────────────────────────────────────────────────

/// Pin 的类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinCategory {
    /// 执行流 Pin（控制流走向）
    Exec,
    /// 数据流 Pin（传递变量值）
    Data,
}

/// Pin 的方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    In,
    Out,
}

/// 数据 Pin 的值类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinValueType {
    String,
    Text,
    Int,
    Float,
    Bool,
    Json,
    Array,
    Any,
    Markdown,
    FileRef,
    WikiRef,
    TicketRef,
    Diff,
    TestResult,
    ReviewComment,
    HandoffSummary,
    RuntimeLog,
    ArtifactRef,
}

/// 节点上的一个 Pin 声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePin {
    /// Pin 唯一 ID（节点内唯一，如 "exec_in", "exec_out", "output", "rule:yes"）
    pub id: String,
    /// 显示标签
    pub label: String,
    /// 方向
    pub direction: PinDirection,
    /// 类别
    pub category: PinCategory,
    /// 数据类型（仅 category=Data 时有意义）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<PinValueType>,
    /// 是否必须连接
    #[serde(default)]
    pub required: bool,
}

// ─── Node Model ──────────────────────────────────────────────────────────────

/// A single node in the task graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub config: serde_json::Value,
    /// 显式 Pin 声明（新增：类型化 In/Out Pin）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<NodePin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Node type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Start,
    End,
    Llm,
    HumanGate,
    Branch,
    Loop,
    Shell,
    /// 输入变量节点：从 graph inputs 中读取一个变量值并输出到 node_outputs。
    InputVar,
    /// 子图调用节点：触发另一个 task graph 的执行。
    #[serde(alias = "sub_pipeline")]
    SubGraph,
}

// ─── Typed Node Configs (for validation) ─────────────────────────────────────

/// Start node config — empty object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartConfig {}

/// Input variable node config — references a graph-level input by id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputVarConfig {
    /// The graph input param id to read from (e.g. "ticket-refs").
    pub input_id: String,
}

/// End node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndConfig {
    pub result: String,
}

/// LLM node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_run_as")]
    pub run_as: LlmRunAs,
    #[serde(default)]
    pub runtime: String,
    #[serde(default = "default_llm_agent")]
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(alias = "model_reasoning_effort", alias = "reasoning_effort")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_profile: Option<String>,
    #[serde(default)]
    pub prompt: LlmPrompt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_args: Vec<String>,
}

/// Local shell/tool node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    #[serde(default = "default_shell_cwd")]
    pub cwd: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_shell_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_shell_permission")]
    pub permission: ShellPermission,
    #[serde(default = "default_shell_expected_exit_codes")]
    pub expected_exit_codes: Vec<i32>,
    #[serde(default)]
    pub capture: ShellCaptureConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellPermission {
    ReadOnly,
    ProjectWrite,
    GitWrite,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCaptureConfig {
    #[serde(default = "default_shell_capture_max_bytes")]
    pub max_bytes: usize,
    #[serde(default = "default_shell_capture_strip_ansi")]
    pub strip_ansi: bool,
}

impl Default for ShellCaptureConfig {
    fn default() -> Self {
        Self {
            max_bytes: default_shell_capture_max_bytes(),
            strip_ansi: default_shell_capture_strip_ansi(),
        }
    }
}

fn default_shell_cwd() -> String {
    ".".to_string()
}

fn default_shell_timeout_ms() -> u64 {
    600_000
}

fn default_shell_permission() -> ShellPermission {
    ShellPermission::ReadOnly
}

fn default_shell_expected_exit_codes() -> Vec<i32> {
    vec![0]
}

fn default_shell_capture_max_bytes() -> usize {
    1_048_576
}

fn default_shell_capture_strip_ansi() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPrompt {
    #[serde(default = "default_llm_prompt_mode")]
    pub mode: String,
    #[serde(default)]
    pub template: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmRunAs {
    Llm,
    Agent,
}

fn default_llm_run_as() -> LlmRunAs {
    LlmRunAs::Llm
}

fn default_llm_agent() -> String {
    "native".to_string()
}

fn default_llm_prompt_mode() -> String {
    "inline".to_string()
}

impl Default for LlmPrompt {
    fn default() -> Self {
        Self {
            mode: default_llm_prompt_mode(),
            template: String::new(),
        }
    }
}

/// Sub-graph node config — invokes another task graph as a child run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraphConfig {
    /// The ID of the child graph to invoke.
    #[serde(alias = "pipeline_id")]
    pub graph_id: String,
    /// Scope of the child graph (system or project). Defaults to project.
    #[serde(default = "default_sub_graph_scope", alias = "pipeline_scope")]
    pub graph_scope: TaskGraphScope,
    /// Input bindings: maps child graph input IDs to values or template expressions.
    /// Values can be constants or template references like `{{inputs.xxx}}` or `{{nodes.yyy.output}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_bindings: Option<serde_json::Value>,
}

/// Backward-compatible alias.
pub type SubPipelineConfig = SubGraphConfig;

fn default_sub_graph_scope() -> TaskGraphScope {
    TaskGraphScope::Project
}

/// Human gate node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanGateConfig {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub actions: Vec<HumanGateAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanGateAction {
    pub id: String,
    pub label: String,
    pub result: String,
}

/// Branch node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConfig {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<String>,
    pub rules: Vec<BranchRule>,
    pub default_rule_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchRule {
    pub id: String,
    pub label: String,
    pub when: serde_json::Value,
}

/// Loop node config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    pub max_iterations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<serde_json::Value>,
    pub body_entry: String,
    pub body_exit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_max_iterations: Option<String>,
}

// ─── Edge Model ──────────────────────────────────────────────────────────────

/// A control-flow edge in the task graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 源节点的 OutPin ID（新格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_pin: Option<String>,
    /// 目标节点的 InPin ID（新格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_pin: Option<String>,
    /// 旧格式兼容字段（保留用于向后兼容）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// 执行流连线
    #[serde(alias = "control", rename = "exec")]
    Exec,
    /// 数据流连线
    #[serde(rename = "data")]
    Data,
}

// ─── Summary (for catalog listing) ──────────────────────────────────────────

/// Lightweight summary returned by list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphSummary {
    pub scope: TaskGraphScope,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: u32,
    pub readonly: bool,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<GraphOrigin>,
    pub node_count: usize,
    pub edge_count: usize,
    /// When a graph JSON file fails to parse, this field carries the error message
    /// so the catalog can still list the broken graph instead of failing entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_error: Option<String>,
}

// ─── Validation Error ────────────────────────────────────────────────────────

/// Structured validation error — serializable for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphValidationError {
    pub path: String,
    pub code: String,
    pub message: String,
}

// ─── Module Error ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TaskGraphError {
    #[error("task graph not found: scope={scope}, id={id}")]
    NotFound { scope: String, id: String },
    #[error("run not found: project={project}, run_id={run_id}")]
    RunNotFound { project: String, run_id: String },
    #[error("invalid run status transition: {from} -> {to}")]
    InvalidRunTransition { from: String, to: String },
    #[error("system graph is readonly: {0}")]
    ReadonlyGraph(String),
    #[error("duplicate graph id in project: {0}")]
    DuplicateGraphId(String),
    #[error("stale version: expected {expected}, found {found}")]
    StaleVersion { expected: u32, found: u32 },
    #[error("validation failed: {count} error(s)")]
    ValidationFailed {
        count: usize,
        errors: Vec<TaskGraphValidationError>,
    },
    #[error("invalid graph id: {0}")]
    InvalidGraphId(String),
    #[error("schedule not found: project={project}, id={id}")]
    ScheduleNotFound { project: String, id: String },
    #[error("duplicate schedule id in project: {0}")]
    DuplicateScheduleId(String),
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON parse error at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

// ─── Known runtimes for MVP ──────────────────────────────────────────────────

pub const KNOWN_RUNTIMES: &[&str] = &["codex", "opencode", "codebuddy"];

/// Valid end node result values.
pub const VALID_END_RESULTS: &[&str] = &["succeeded", "failed", "cancelled"];
