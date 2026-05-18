//! Core types for the Graph Runtime architecture.
//!
//! Defines the data structures that flow between the three layers:
//! - **Planner** produces `ReadyNode` (which nodes are ready to execute)
//! - **Executor** produces `NodeOutcome` (execution results)
//! - **Reducer** consumes `NodeOutcome` and produces `ReduceAction` (state mutations)

use crate::task_graph::pregel::PregelLoopStatus;
use crate::task_graph::pregel::{PregelTask, PregelTaskKind, PregelWrite};
use crate::task_graph::run_state::{
    BranchDecision, LoopFrame, LoopIterationState, NodeRunStatus, RunPaused, TaskGraphRunNode,
};
use crate::task_graph::topology::GraphMutationRequest;

// ─── Execution Mode ──────────────────────────────────────────────────────────

/// 节点的执行模式：内联（控制平面直接执行）或派发（执行平面并行执行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 纯逻辑节点（Start/End/Branch/Loop/InputVar/HumanGate），在控制平面内联执行。
    Inline,
    /// I/O 节点（LLM/SubGraph），派发给执行平面并行执行。
    Dispatch,
}

// ─── Ready Node ──────────────────────────────────────────────────────────────

/// Planner 计算出的就绪节点，携带执行模式信息。
#[derive(Debug, Clone)]
pub struct ReadyNode {
    /// 节点 ID。
    pub node_id: String,
    /// 执行模式：Inline 或 Dispatch。
    pub execution_mode: ExecutionMode,
    /// Pregel task kind that produced this runnable node.
    pub task_kind: PregelTaskKind,
    /// Pregel task-local input. PUSH tasks use this as their invocation input.
    pub task_input: serde_json::Value,
}

// ─── Superstep Plan ─────────────────────────────────────────────────────────

/// Runnable set produced for one superstep.
///
/// The coordinator persists this plan through checkpoints/events after the
/// barrier. It is deliberately runtime-agnostic: business prompts and Agent
/// Profiles remain outside the execution kernel.
#[derive(Debug, Clone)]
pub struct SuperstepPlan {
    pub superstep: u64,
    pub ready_nodes: Vec<ReadyNode>,
    pub waiting_nodes: Vec<String>,
    pub pregel_tasks: Vec<PregelTask>,
    pub replayed_pregel_tasks: Vec<PregelTask>,
    pub replayed_pregel_writes: Vec<PregelWrite>,
    pub pregel_loop_status: PregelLoopStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlDirective {
    Goto {
        target: String,
    },
    Send {
        node: String,
        args: serde_json::Value,
    },
}

impl ControlDirective {
    pub fn goto(target: impl Into<String>) -> Self {
        Self::Goto {
            target: target.into(),
        }
    }
}

// ─── Side Effect ─────────────────────────────────────────────────────────────

/// 节点执行过程中产生的副作用，由 Reducer 统一应用到 `RunState`。
///
/// `execute_node` 不直接写全局状态，而是将需要写入的变更封装为 `SideEffect` 返回。
#[derive(Debug, Clone)]
pub enum SideEffect {
    /// Branch 节点的分支决策记录。
    BranchDecision(BranchDecision),

    /// Loop 节点的迭代状态记录（创建或更新）。
    LoopIteration(LoopIterationState),

    /// 进入 Loop body 前推入的 continuation frame。
    LoopFramePush(LoopFrame),

    /// 从 Loop body 返回时弹出的 continuation frame。
    /// 值为 `loop_node_id`。
    LoopFramePop(String),

    /// `HumanGate` 暂停元数据。
    RunPaused(RunPaused),
}

// ─── Node Outcome ────────────────────────────────────────────────────────────

/// 节点执行的完整结果。
///
/// `execute_node` 返回此结构体，Coordinator 的 Reducer 负责将其归并到 `RunState`。
/// **核心原则**：`execute_node` 不写全局状态，只返回 `NodeOutcome`。
#[derive(Debug, Clone)]
pub struct NodeOutcome {
    /// 执行的节点 ID。
    pub node_id: String,

    /// 节点最终状态（Succeeded / Failed / Paused）。
    pub status: NodeRunStatus,

    /// 节点输出数据（写入 `node_outputs/{node_id}.json`）。
    pub output: Option<serde_json::Value>,

    /// 完整的节点状态快照（写入 `nodes/{node_id}.json`）。
    pub node_state: TaskGraphRunNode,

    /// 需要写入 run context 的副作用列表。
    pub side_effects: Vec<SideEffect>,

    /// `SubGraph` 节点产生的子 run ID。
    pub child_run_id: Option<String>,

    /// 是否为 End 节点完成（携带 end result: "succeeded" / "failed"）。
    pub end_result: Option<String>,

    /// Explicit control flow produced by control nodes.
    pub control: Vec<ControlDirective>,

    /// Topology mutation requests emitted by this node.
    pub graph_mutations: Vec<GraphMutationRequest>,
}

// ─── Reduce Action ───────────────────────────────────────────────────────────

/// Reducer 归并后产生的状态变更描述。
///
/// 用于描述一轮调度后 `RunState` 应该如何变化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReduceAction {
    /// 继续执行。
    Continue,

    /// Run 完成（到达 End 节点）。
    Completed { node_id: String, result: String },

    /// Run 暂停（HumanGate）。
    Paused { node_id: String },

    /// Run 失败（某个节点执行失败）。
    Failed { node_id: String, message: String },

    /// Run 被取消。
    Cancelled,
}
