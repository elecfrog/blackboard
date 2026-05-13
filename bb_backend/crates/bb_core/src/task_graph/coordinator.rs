//! Graph Coordinator — 控制平面单线程调度器。
//!
//! 负责 graph run 的完整生命周期：
//! 1. 从磁盘加载 RunState 到内存
//! 2. 主循环：Plan → Dispatch → Reduce → Persist
//! 3. 终止条件：Completed / Failed / Paused / Cancelled
//!
//! **核心原则**：所有 graph 状态变更都在 Coordinator 中发生（单写者）。
//! 执行平面（Executor）只返回 `NodeOutcome`，不写全局状态。

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use chrono::Utc;

use super::executor;
use super::interpreter::InterpreterOptions;
use super::node_exec::find_start_node;
use super::outcome::{ExecutionMode, NodeOutcome, ReadyNode, ReduceAction, SideEffect};
use super::run_state::{self, NodeRunStatus, RunStatus, TaskGraphRun, TaskGraphRunNode};
use super::types::{NodeType, TaskGraphDefinition, TaskGraphEdge, TaskGraphError, TaskGraphNode};

// ─── Graph Coordinator ──────────────────────────────────────────────────────

/// Graph Coordinator — 控制平面单线程调度器。
pub(super) struct GraphCoordinator<'a> {
    opts: &'a InterpreterOptions,
    graph: TaskGraphDefinition,
    run: TaskGraphRun,
    edge_map: HashMap<String, Vec<TaskGraphEdge>>,
    node_map: HashMap<String, TaskGraphNode>,
    join_nodes: HashSet<String>,
    /// 本轮需要持久化的节点状态
    pending_node_states: Vec<TaskGraphRunNode>,
    /// 本轮需要持久化的节点输出
    pending_node_outputs: Vec<(String, serde_json::Value)>,
    /// Run 开始执行的时刻，用于整体超时检测
    run_start: Instant,
}

impl<'a> GraphCoordinator<'a> {
    /// 从磁盘加载 RunState 并创建 Coordinator。
    pub fn load(opts: &'a InterpreterOptions) -> Result<Self, TaskGraphError> {
        let ws = &opts.workspace_root;
        let project = &opts.project;
        let run_id = &opts.run_id;

        let detail = run_state::read_run_detail(ws, project, run_id)?;
        let graph = detail.graph_snapshot;
        let run = detail.run;

        // 构建导航结构（owned 版本，避免生命周期问题）
        let edge_map = build_owned_edge_map(&graph.edges);
        let node_map = build_owned_node_map(&graph.nodes);
        let join_nodes = run_state::detect_join_nodes(&graph.edges);

        Ok(Self {
            opts,
            graph,
            run,
            edge_map,
            node_map,
            join_nodes,
            pending_node_states: Vec::new(),
            pending_node_outputs: Vec::new(),
            run_start: Instant::now(),
        })
    }

    /// 主循环：Plan → Dispatch → Reduce → Persist → 循环。
    pub fn run(&mut self) -> Result<super::interpreter::RunOutcome, TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;

        // Transition to running if pending
        if self.run.status == RunStatus::Pending {
            self.run = run_state::update_run_status(ws, project, run_id, RunStatus::Running)?;
        }

        // Initialize cursor if empty (start of run)
        if self.run.cursor.is_empty() {
            let start =
                find_start_node(&self.graph).ok_or_else(|| TaskGraphError::ValidationFailed {
                    count: 1,
                    errors: vec![super::types::TaskGraphValidationError {
                        path: "nodes".to_string(),
                        code: "missing_start".to_string(),
                        message: "No start node found in graph".to_string(),
                    }],
                })?;
            self.run.cursor = vec![start.id.clone()];

            // Mark start node as succeeded immediately
            let now = Utc::now().to_rfc3339();
            let start_state = TaskGraphRunNode {
                node_id: start.id.clone(),
                status: NodeRunStatus::Succeeded,
                started_at: Some(now.clone()),
                completed_at: Some(now),
                duration_ms: Some(0),
                iteration: None,
                exit_code: None,
                error: None,
                output_artifact: None,
                log_tail: None,
                child_run_id: None,
                runtime: None,
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            };
            run_state::update_node_state(ws, project, run_id, &start_state)?;
            self.persist_cursor()?;
        }

        // ── Main execution loop ──
        loop {
            // Cancel 检测：每轮调度前检查外部是否已取消
            if self.check_cancelled()? {
                return Ok(super::interpreter::RunOutcome::Cancelled);
            }

            // Run 级别超时检测
            if self.run_start.elapsed() >= self.opts.run_timeout {
                let msg = format!("Run timeout: exceeded {:?} limit", self.opts.run_timeout);
                run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                return Ok(super::interpreter::RunOutcome::Failed {
                    node_id: self.run.cursor.first().cloned().unwrap_or_default(),
                    message: msg,
                });
            }

            if self.run.cursor.is_empty() {
                run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                return Ok(super::interpreter::RunOutcome::Failed {
                    node_id: "unknown".to_string(),
                    message: "Cursor is empty but run not completed".to_string(),
                });
            }

            // ── Phase 1: Plan — 计算 ready nodes ──
            let (ready_nodes, waiting_cursor) = self.plan();

            if ready_nodes.is_empty() {
                // 所有节点都在等待 join，cursor 不变
                if waiting_cursor == self.run.cursor {
                    // 死锁检测：cursor 没有变化
                    run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                    return Ok(super::interpreter::RunOutcome::Failed {
                        node_id: "unknown".to_string(),
                        message: "Deadlock: no ready nodes and cursor unchanged".to_string(),
                    });
                }
                self.run.cursor = waiting_cursor;
                self.persist_cursor()?;
                continue;
            }

            // ── Phase 2: Dispatch — 执行 ready nodes ──
            // 构建 borrowed edge_map 供 executor 使用
            let borrowed_edge_map = self.borrow_edge_map();
            let borrowed_node_map = self.borrow_node_map();

            let outcomes = executor::execute_ready_nodes(
                self.opts,
                &ready_nodes,
                &self.run,
                &borrowed_edge_map,
                &borrowed_node_map,
            )?;

            // ── Phase 3: Reduce — 归并结果 ──
            let action = self.reduce(outcomes, &waiting_cursor)?;

            // ── Phase 4: Persist — 批量写入磁盘 ──
            self.persist()?;

            // ── Phase 5: 处理 ReduceAction ──
            match action {
                ReduceAction::Continue { next_cursor } => {
                    self.run.cursor = next_cursor;
                    self.persist_cursor()?;
                }
                ReduceAction::Completed {
                    node_id: end_node_id,
                    result,
                } => {
                    let final_status = if result == "succeeded" {
                        RunStatus::Succeeded
                    } else {
                        RunStatus::Failed
                    };
                    // 检查是否已被外部取消
                    let latest_status = run_state::read_run(ws, project, run_id)?.status;
                    let final_status = if latest_status == RunStatus::Cancelled {
                        RunStatus::Cancelled
                    } else {
                        final_status
                    };
                    run_state::update_run_status(ws, project, run_id, final_status)?;
                    return match final_status {
                        RunStatus::Cancelled => Ok(super::interpreter::RunOutcome::Cancelled),
                        RunStatus::Succeeded => Ok(super::interpreter::RunOutcome::Succeeded),
                        _ => Ok(super::interpreter::RunOutcome::Failed {
                            node_id: end_node_id,
                            message: format!("Run ended with result: {}", result),
                        }),
                    };
                }
                ReduceAction::Paused { node_id } => {
                    // 设置 run 为 Paused 状态
                    // paused 元数据已在 reduce 阶段通过 SideEffect::RunPaused 写入 self.run.paused
                    if let Some(ref paused) = self.run.paused {
                        run_state::set_run_paused(ws, project, run_id, paused.clone())?;
                    } else {
                        run_state::update_run_status(ws, project, run_id, RunStatus::Paused)?;
                    }
                    // paused cursor: 保留 paused 节点 + waiting 节点
                    let mut paused_cursor = vec![node_id.clone()];
                    paused_cursor.extend(waiting_cursor);
                    self.run.cursor = paused_cursor;
                    self.persist_cursor()?;
                    return Ok(super::interpreter::RunOutcome::Paused { node_id });
                }
                ReduceAction::Failed { node_id, message } => {
                    run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                    return Ok(super::interpreter::RunOutcome::Failed { node_id, message });
                }
                ReduceAction::Cancelled => {
                    return Ok(super::interpreter::RunOutcome::Cancelled);
                }
            }
        }
    }

    // ─── Planner ─────────────────────────────────────────────────────────────

    /// 计算当前 cursor 中哪些节点可以执行。
    ///
    /// 返回 (ready_nodes, waiting_cursor)：
    /// - ready_nodes: 可以立即执行的节点
    /// - waiting_cursor: 需要保留在 cursor 中等待的节点（join 点未就绪）
    fn plan(&self) -> (Vec<ReadyNode>, Vec<String>) {
        let mut ready = Vec::new();
        let mut waiting = Vec::new();

        for node_id in &self.run.cursor {
            let Some(node) = self.node_map.get(node_id) else {
                // 节点不存在，跳过（会在 reduce 阶段报错）
                continue;
            };

            // Start 节点：直接展开后继
            if node.node_type == NodeType::Start {
                // Start 节点已在初始化时处理，这里直接展开
                let borrowed_edge_map = self.borrow_edge_map();
                if let Ok(next) = super::node_exec::resolve_next_nodes(
                    &borrowed_edge_map,
                    node_id,
                    node,
                    &self.run.context,
                    None,
                ) {
                    for n in next {
                        if !waiting.contains(&n)
                            && !ready.iter().any(|r: &ReadyNode| r.node_id == n)
                        {
                            // 递归检查后继是否也是 ready
                            waiting.push(n);
                        }
                    }
                }
                continue;
            }

            // Join 点检查
            if self.join_nodes.contains(node_id) {
                let expected = run_state::count_exec_in_edges(&self.graph.edges, node_id);
                if !run_state::is_join_ready(&self.run, node_id, expected) {
                    // 未就绪，保留在 waiting
                    if !waiting.contains(node_id) {
                        waiting.push(node_id.clone());
                    }
                    continue;
                }
            }

            // 分类执行模式
            let mode = match node.node_type {
                NodeType::Start
                | NodeType::End
                | NodeType::Branch
                | NodeType::Loop
                | NodeType::InputVar
                | NodeType::HumanGate => ExecutionMode::Inline,
                NodeType::Llm | NodeType::SubGraph => ExecutionMode::Dispatch,
            };

            ready.push(ReadyNode {
                node_id: node_id.clone(),
                execution_mode: mode,
            });
        }

        (ready, waiting)
    }

    // ─── Reducer ─────────────────────────────────────────────────────────────

    /// 归并一批 NodeOutcome 到内存 RunState。
    ///
    /// 返回 ReduceAction 描述下一步应该做什么。
    fn reduce(
        &mut self,
        outcomes: Vec<NodeOutcome>,
        waiting_cursor: &[String],
    ) -> Result<ReduceAction, TaskGraphError> {
        let mut next_cursor: Vec<String> = Vec::new();
        let mut had_completion = false;
        let mut completion_result: Option<String> = None;
        let mut completion_node_id: Option<String> = None;
        let mut had_pause = false;
        let mut pause_node_id: Option<String> = None;

        for outcome in outcomes {
            // 1. 收集节点状态（稍后批量持久化）
            self.pending_node_states.push(outcome.node_state.clone());

            // 2. 收集节点输出
            if let Some(output) = &outcome.output {
                self.pending_node_outputs
                    .push((outcome.node_id.clone(), output.clone()));
                // 同时更新内存中的 context
                self.run
                    .context
                    .node_outputs
                    .insert(outcome.node_id.clone(), output.clone());
            }

            // 3. 应用 side_effects
            for effect in &outcome.side_effects {
                match effect {
                    SideEffect::BranchDecision(decision) => {
                        self.run.context.branch_decisions.push(decision.clone());
                    }
                    SideEffect::LoopIteration(state) => {
                        // Upsert
                        if let Some(existing) = self
                            .run
                            .context
                            .loop_iterations
                            .iter_mut()
                            .find(|l| l.loop_node_id == state.loop_node_id)
                        {
                            *existing = state.clone();
                        } else {
                            self.run.context.loop_iterations.push(state.clone());
                        }
                    }
                    SideEffect::LoopFramePush(frame) => {
                        self.run.context.loop_stack.push(frame.clone());
                    }
                    SideEffect::LoopFramePop(loop_node_id) => {
                        if self
                            .run
                            .context
                            .loop_stack
                            .last()
                            .map(|f| f.loop_node_id.as_str())
                            == Some(loop_node_id.as_str())
                        {
                            self.run.context.loop_stack.pop();
                        }
                    }
                    SideEffect::RunPaused(paused) => {
                        self.run.paused = Some(paused.clone());
                    }
                }
            }

            // 4. 处理终止条件
            if let Some(ref end_result) = outcome.end_result {
                had_completion = true;
                completion_result = Some(end_result.clone());
                completion_node_id = Some(outcome.node_id.clone());
                continue;
            }

            if outcome.status == NodeRunStatus::Paused {
                had_pause = true;
                pause_node_id = Some(outcome.node_id.clone());
                continue;
            }

            if outcome.status == NodeRunStatus::Failed {
                let msg = outcome
                    .node_state
                    .error
                    .as_ref()
                    .map(|e| e.message.clone())
                    .unwrap_or_else(|| "Node execution failed".to_string());
                return Ok(ReduceAction::Failed {
                    node_id: outcome.node_id.clone(),
                    message: msg,
                });
            }

            // 5. 计算 next_cursor（处理 join 点）
            for n in &outcome.next_nodes {
                if self.join_nodes.contains(n.as_str()) {
                    // 记录 branch completion（内存中操作）
                    self.run
                        .context
                        .completed_branches
                        .entry(n.clone())
                        .or_default()
                        .push(outcome.node_id.clone());

                    let expected = run_state::count_exec_in_edges(&self.graph.edges, n);
                    if run_state::is_join_ready(&self.run, n, expected) {
                        if !next_cursor.contains(n) {
                            next_cursor.push(n.clone());
                        }
                    } else {
                        // join 点未就绪，加入 waiting
                        // （不加入 next_cursor，保留在下一轮的 waiting 中）
                        if !next_cursor.contains(n) {
                            next_cursor.push(n.clone());
                        }
                    }
                } else {
                    if !next_cursor.contains(n) {
                        next_cursor.push(n.clone());
                    }
                }
            }
        }

        // 处理 completion
        if had_completion {
            if let Some(result) = completion_result {
                return Ok(ReduceAction::Completed {
                    node_id: completion_node_id.unwrap_or_default(),
                    result,
                });
            }
        }

        // 处理 pause
        if had_pause {
            if let Some(node_id) = pause_node_id {
                return Ok(ReduceAction::Paused { node_id });
            }
        }

        // 合并 waiting cursor
        for w in waiting_cursor {
            if !next_cursor.contains(w) {
                next_cursor.push(w.clone());
            }
        }

        Ok(ReduceAction::Continue { next_cursor })
    }

    // ─── Persistence ─────────────────────────────────────────────────────────

    /// 批量持久化本轮的状态变更。
    fn persist(&mut self) -> Result<(), TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;

        // 写入节点状态
        for state in self.pending_node_states.drain(..) {
            run_state::update_node_state(ws, project, run_id, &state)?;
        }

        // 写入节点输出
        for (node_id, output) in self.pending_node_outputs.drain(..) {
            run_state::set_node_output(ws, project, run_id, &node_id, output)?;
        }

        // 写入 run.json（包含 context 中的 branch_decisions / loop_iterations / loop_stack / completed_branches）
        run_state::write_run_json(ws, project, run_id, &self.run)?;

        Ok(())
    }

    /// 持久化 cursor 变更。
    fn persist_cursor(&self) -> Result<(), TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;
        run_state::update_cursor(ws, project, run_id, self.run.cursor.clone())
    }

    // ─── Cancel detection ────────────────────────────────────────────────────

    /// 检查 run 是否被外部取消。
    fn check_cancelled(&self) -> Result<bool, TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;
        let status = run_state::read_run(ws, project, run_id)?.status;
        Ok(status == RunStatus::Cancelled)
    }

    // ─── Helper: borrowed edge/node maps ─────────────────────────────────────

    /// 构建 borrowed edge_map（供 node_exec 函数使用）。
    fn borrow_edge_map(&self) -> HashMap<String, Vec<&TaskGraphEdge>> {
        let mut map: HashMap<String, Vec<&TaskGraphEdge>> = HashMap::new();
        for edges in self.edge_map.values() {
            for edge in edges {
                map.entry(edge.from.clone()).or_default().push(edge);
            }
        }
        map
    }

    /// 构建 borrowed node_map（供 node_exec 函数使用）。
    fn borrow_node_map(&self) -> HashMap<String, &TaskGraphNode> {
        self.node_map.iter().map(|(k, v)| (k.clone(), v)).collect()
    }
}

// ─── Owned map builders ──────────────────────────────────────────────────────

/// Build an owned adjacency map: node_id → outgoing edges (owned).
fn build_owned_edge_map(edges: &[TaskGraphEdge]) -> HashMap<String, Vec<TaskGraphEdge>> {
    let mut map: HashMap<String, Vec<TaskGraphEdge>> = HashMap::new();
    for edge in edges {
        map.entry(edge.from.clone()).or_default().push(edge.clone());
    }
    map
}

/// Build an owned node lookup map: node_id → node (owned).
fn build_owned_node_map(nodes: &[TaskGraphNode]) -> HashMap<String, TaskGraphNode> {
    nodes.iter().map(|n| (n.id.clone(), n.clone())).collect()
}
