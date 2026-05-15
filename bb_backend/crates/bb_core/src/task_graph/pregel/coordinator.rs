//! Graph Coordinator — 控制平面单线程调度器。
//!
//! 负责 graph run 的完整生命周期：
//! 1. 从磁盘加载 RunState 到内存
//! 2. 主循环：Plan → Dispatch → Reduce → Persist
//! 3. 终止条件：Completed / Failed / Paused / Cancelled
//!
//! **核心原则**：所有最终 graph 状态变更都在 Coordinator 中发生（单写者）。
//! 执行平面（Executor）返回 `NodeOutcome`；长运行节点只允许写 streaming log / running projection。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use chrono::Utc;

use super::executor;
use super::outcome::{
    ExecutionMode, NodeOutcome, ReadyNode, ReduceAction, SideEffect, SuperstepPlan,
};
use super::runner::{RunOutcome, RunnerOptions};
use crate::task_graph::compile::compiler::{
    compile_graph_for_execution, CompiledGraph, CompiledProcessMode,
};
use crate::task_graph::definition::types::{
    TaskGraphDefinition, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::pregel::{
    checkpoint_config, checkpoint_metadata, initial_checkpoint as initial_pregel_checkpoint,
    interrupt_write, writes_from_node_outcome, PregelLoop, PregelLoopConfig, PregelLoopStatus,
    PregelPreparedStep, DEFAULT_CHECKPOINT_NAMESPACE,
};
use crate::task_graph::run_state::{
    self, NodeRunStatus, PausedAction, PendingWrite, RunPaused, RunStatus, SuperstepCheckpoint,
    SuperstepStatus, TaskGraphRun, TaskGraphRunNode,
};

// ─── Graph Coordinator ──────────────────────────────────────────────────────

/// Graph Coordinator — 控制平面单线程调度器。
pub(super) struct GraphCoordinator<'a> {
    opts: &'a RunnerOptions,
    graph: TaskGraphDefinition,
    compiled: CompiledGraph,
    pregel_loop: PregelLoop,
    run: TaskGraphRun,
    edge_map: HashMap<String, Vec<TaskGraphEdge>>,
    node_map: HashMap<String, TaskGraphNode>,
    join_nodes: HashSet<String>,
    /// 本轮需要持久化的节点状态
    pending_node_states: Vec<TaskGraphRunNode>,
    /// 本轮需要持久化的节点输出
    pending_node_outputs: Vec<(String, serde_json::Value)>,
    /// 本轮 barrier 前收集到的 pending writes，写入 superstep checkpoint。
    pending_writes: Vec<PendingWrite>,
    /// Run 开始执行的时刻，用于整体超时检测
    run_start: Instant,
}

impl<'a> GraphCoordinator<'a> {
    /// 从磁盘加载 RunState 并创建 Coordinator。
    pub fn load(opts: &'a RunnerOptions) -> Result<Self, TaskGraphError> {
        let ws = &opts.workspace_root;
        let project = &opts.project;
        let run_id = &opts.run_id;

        let detail = run_state::read_run_detail(ws, project, run_id)?;
        let graph = detail.graph_snapshot;
        let compiled = compile_graph_for_execution(&graph)?;
        let mut run = detail.run;
        if run.pregel_checkpoint.is_none() {
            run.pregel_checkpoint = Some(initial_pregel_checkpoint(
                &compiled,
                run.context.input.clone(),
            ));
        }
        let checkpoint_ns = run
            .checkpoint_ns
            .clone()
            .unwrap_or_else(|| DEFAULT_CHECKPOINT_NAMESPACE.to_string());
        let tuple = run_state::read_pregel_checkpoint_tuple(ws, project, run_id, &checkpoint_ns)?;
        let (checkpoint, recovered_pregel_writes) = if let Some(tuple) = tuple {
            let checkpoint = tuple.checkpoint;
            run.current_superstep = run.current_superstep.max(checkpoint.superstep);
            run.last_checkpoint_id = Some(tuple.config.checkpoint_id);
            run.pregel_checkpoint = Some(checkpoint.clone());
            (checkpoint, tuple.pending_writes)
        } else {
            let checkpoint = run
                .pregel_checkpoint
                .clone()
                .unwrap_or_else(|| initial_pregel_checkpoint(&compiled, run.context.input.clone()));
            run.pregel_checkpoint = Some(checkpoint.clone());
            (checkpoint, Vec::new())
        };
        run_state::write_run_json(ws, project, run_id, &run)?;
        let pregel_loop_config = graph
            .metadata
            .as_ref()
            .map(|metadata| PregelLoopConfig {
                stop: metadata.recursion_limit,
                interrupt_before: metadata.interrupt_before.clone().unwrap_or_default(),
                interrupt_after: metadata.interrupt_after.clone().unwrap_or_default(),
            })
            .unwrap_or_default();
        let pregel_loop = PregelLoop::with_config(
            compiled.clone(),
            checkpoint,
            recovered_pregel_writes,
            pregel_loop_config,
        );

        // 构建导航结构（owned 版本，避免生命周期问题）
        let edge_map = build_owned_edge_map(&graph.edges);
        let node_map = build_owned_node_map(&graph.nodes);
        let join_nodes = compiled.join_nodes.iter().cloned().collect();

        Ok(Self {
            opts,
            graph,
            compiled,
            pregel_loop,
            run,
            edge_map,
            node_map,
            join_nodes,
            pending_node_states: Vec::new(),
            pending_node_outputs: Vec::new(),
            pending_writes: Vec::new(),
            run_start: Instant::now(),
        })
    }

    /// 主循环：Plan → Dispatch → Reduce → Persist → 循环。
    pub fn run(&mut self) -> Result<RunOutcome, TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;

        // Transition to running if pending
        if self.run.status == RunStatus::Pending {
            self.run = run_state::update_run_status(ws, project, run_id, RunStatus::Running)?;
            self.append_event(
                0,
                "run_started",
                None,
                "task graph run started",
                serde_json::json!({
                    "graph_id": self.graph.id.clone(),
                    "graph_version": self.graph.version,
                    "entrypoint": self.compiled.entrypoint.clone(),
                }),
            )?;
        }

        // ── Main execution loop ──
        loop {
            // Cancel 检测：每轮调度前检查外部是否已取消
            if self.check_cancelled()? {
                return Ok(RunOutcome::Cancelled);
            }

            // Run 级别超时检测
            if self.run_start.elapsed() >= self.opts.run_timeout {
                let msg = format!("Run timeout: exceeded {:?} limit", self.opts.run_timeout);
                run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                return Ok(RunOutcome::Failed {
                    node_id: self.run.cursor.first().cloned().unwrap_or_default(),
                    message: msg,
                });
            }

            // ── Phase 1: Plan — 计算 ready nodes ──
            let superstep = self.run.current_superstep + 1;
            let plan = self.prepare_superstep_plan(superstep);

            if plan.pregel_loop_status == PregelLoopStatus::InterruptBefore {
                return self.pause_for_interrupt_before(&plan);
            }

            if plan.ready_nodes.is_empty() && plan.replayed_pregel_tasks.is_empty() {
                let message = match plan.pregel_loop_status {
                    PregelLoopStatus::OutOfSteps => {
                        format!("Pregel scheduler exceeded recursion limit before superstep {superstep}")
                    }
                    PregelLoopStatus::Done
                    | PregelLoopStatus::Pending
                    | PregelLoopStatus::InterruptBefore
                    | PregelLoopStatus::InterruptAfter => {
                        "Pregel scheduler produced no runnable tasks before reaching an end node"
                            .to_string()
                    }
                };
                run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                return Ok(RunOutcome::Failed {
                    node_id: "unknown".to_string(),
                    message,
                });
            }

            self.run.cursor = plan
                .ready_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect();
            self.persist_cursor()?;

            self.append_event(
                superstep,
                "superstep_started",
                None,
                format!("superstep {superstep} started"),
                serde_json::json!({
                    "cursor_before": plan.cursor_before.clone(),
                    "ready_nodes": plan.ready_nodes.iter().map(|node| node.node_id.clone()).collect::<Vec<_>>(),
                    "waiting_nodes": plan.waiting_nodes.clone(),
                    "replayed_tasks": plan.replayed_pregel_tasks.iter().map(|task| task.node_id.clone()).collect::<Vec<_>>(),
                    "pregel_loop_status": format!("{:?}", plan.pregel_loop_status),
                }),
            )?;
            self.record_node_started_events(superstep, &plan.ready_nodes)?;

            // ── Phase 2: Dispatch — 执行 ready nodes ──
            // 构建 borrowed edge_map 供 executor 使用
            let borrowed_edge_map = self.borrow_edge_map();
            let borrowed_node_map = self.borrow_node_map();

            let outcomes = if plan.ready_nodes.is_empty() {
                Vec::new()
            } else {
                executor::execute_ready_nodes(
                    self.opts,
                    &plan.ready_nodes,
                    &self.run,
                    &borrowed_edge_map,
                    &borrowed_node_map,
                )?
            };
            self.record_node_finished_events(superstep, &outcomes)?;

            // ── Phase 3: Reduce — 归并结果 ──
            let action = self.reduce(outcomes, &plan)?;

            // ── Phase 4: Persist — 批量写入磁盘 ──
            self.persist()?;

            // ── Phase 5: 处理 ReduceAction ──
            match action {
                ReduceAction::Continue { next_cursor } => {
                    let cursor_after = next_cursor.clone();
                    self.run.cursor = next_cursor;
                    self.persist_cursor()?;
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Succeeded,
                        cursor_after,
                        "superstep_completed",
                        "superstep completed",
                    )?;
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
                        self.run = run_state::read_run(ws, project, run_id)?;
                        RunStatus::Cancelled
                    } else {
                        final_status
                    };
                    if latest_status != RunStatus::Cancelled {
                        self.run = run_state::update_run_status(ws, project, run_id, final_status)?;
                    }
                    let superstep_status = match final_status {
                        RunStatus::Succeeded => SuperstepStatus::Succeeded,
                        RunStatus::Cancelled => SuperstepStatus::Cancelled,
                        _ => SuperstepStatus::Failed,
                    };
                    self.record_superstep(
                        &plan,
                        superstep_status,
                        Vec::new(),
                        if final_status == RunStatus::Succeeded {
                            "run_completed"
                        } else {
                            "run_failed"
                        },
                        format!("run ended with result: {result}"),
                    )?;
                    return match final_status {
                        RunStatus::Cancelled => Ok(RunOutcome::Cancelled),
                        RunStatus::Succeeded => Ok(RunOutcome::Succeeded),
                        _ => Ok(RunOutcome::Failed {
                            node_id: end_node_id,
                            message: format!("Run ended with result: {}", result),
                        }),
                    };
                }
                ReduceAction::Paused { node_id } => {
                    // 设置 run 为 Paused 状态
                    // paused 元数据已在 reduce 阶段通过 SideEffect::RunPaused 写入 self.run.paused
                    if let Some(ref paused) = self.run.paused {
                        self.run = run_state::set_run_paused(ws, project, run_id, paused.clone())?;
                    } else {
                        self.run =
                            run_state::update_run_status(ws, project, run_id, RunStatus::Paused)?;
                    }
                    // paused cursor: 保留 paused 节点 + waiting 节点
                    let mut paused_cursor = vec![node_id.clone()];
                    paused_cursor.extend(plan.waiting_nodes.clone());
                    let cursor_after = paused_cursor.clone();
                    self.run.cursor = paused_cursor;
                    self.persist_cursor()?;
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Paused,
                        cursor_after,
                        "run_paused",
                        format!("run paused at node {node_id}"),
                    )?;
                    return Ok(RunOutcome::Paused { node_id });
                }
                ReduceAction::Failed { node_id, message } => {
                    self.run =
                        run_state::update_run_status(ws, project, run_id, RunStatus::Failed)?;
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Failed,
                        self.run.cursor.clone(),
                        "run_failed",
                        message.clone(),
                    )?;
                    return Ok(RunOutcome::Failed { node_id, message });
                }
                ReduceAction::Cancelled => {
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Cancelled,
                        self.run.cursor.clone(),
                        "run_cancelled",
                        "run cancelled",
                    )?;
                    return Ok(RunOutcome::Cancelled);
                }
            }
        }
    }

    // ─── Planner ─────────────────────────────────────────────────────────────

    fn prepare_superstep_plan(&mut self, superstep: u64) -> SuperstepPlan {
        let prepared = self.pregel_loop.prepare_next(superstep);
        let pregel_loop_status = self.pregel_loop.status();
        let ready_nodes = prepared
            .tasks
            .iter()
            .map(|task| {
                let execution_mode = self
                    .compiled
                    .processes
                    .get(&task.node_id)
                    .map(|process| match process.mode {
                        CompiledProcessMode::Inline => ExecutionMode::Inline,
                        CompiledProcessMode::RuntimeAdapter => ExecutionMode::Dispatch,
                    })
                    .unwrap_or(ExecutionMode::Inline);
                ReadyNode {
                    node_id: task.node_id.clone(),
                    execution_mode,
                    task_kind: task.kind.clone(),
                    task_input: task.input.clone(),
                }
            })
            .collect();
        SuperstepPlan {
            superstep,
            cursor_before: self.run.cursor.clone(),
            ready_nodes,
            waiting_nodes: Vec::new(),
            pregel_tasks: prepared.tasks,
            replayed_pregel_tasks: prepared.replayed_tasks,
            replayed_pregel_writes: prepared.replayed_writes,
            pregel_loop_status,
        }
    }

    fn pause_for_interrupt_before(
        &mut self,
        plan: &SuperstepPlan,
    ) -> Result<RunOutcome, TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;
        let node_id = plan
            .ready_nodes
            .first()
            .map(|node| node.node_id.clone())
            .unwrap_or_else(|| "__interrupt__".to_string());
        self.run.status = RunStatus::Paused;
        self.run.paused = Some(RunPaused {
            node_id: node_id.clone(),
            reason: "interrupt_before".to_string(),
            actions: vec![
                PausedAction {
                    id: "resume".to_string(),
                    label: "Resume".to_string(),
                    result: "resume".to_string(),
                },
                PausedAction {
                    id: "cancel".to_string(),
                    label: "Cancel".to_string(),
                    result: "cancel".to_string(),
                },
            ],
        });
        self.run.cursor = plan
            .ready_nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect();
        self.run.pregel_checkpoint = Some(self.pregel_loop.checkpoint().clone());
        run_state::write_run_json(ws, project, run_id, &self.run)?;
        self.append_event(
            plan.superstep,
            "run_interrupted",
            Some(node_id.clone()),
            format!("run interrupted before node {node_id}"),
            serde_json::json!({
                "reason": "interrupt_before",
                "ready_nodes": plan.ready_nodes.iter().map(|node| node.node_id.clone()).collect::<Vec<_>>(),
                "pregel_checkpoint_id": self.pregel_loop.checkpoint().id.clone(),
                "pregel_loop_status": format!("{:?}", plan.pregel_loop_status),
            }),
        )?;
        Ok(RunOutcome::Paused { node_id })
    }

    // ─── Reducer ─────────────────────────────────────────────────────────────

    /// 归并一批 NodeOutcome 到内存 RunState。
    ///
    /// 返回 ReduceAction 描述下一步应该做什么。
    fn reduce(
        &mut self,
        outcomes: Vec<NodeOutcome>,
        plan: &SuperstepPlan,
    ) -> Result<ReduceAction, TaskGraphError> {
        let mut next_cursor: Vec<String> = Vec::new();
        let mut had_completion = false;
        let mut completion_result: Option<String> = None;
        let mut completion_node_id: Option<String> = None;
        let mut had_pause = false;
        let mut pause_node_id: Option<String> = None;

        for outcome in outcomes {
            let task = plan
                .pregel_tasks
                .iter()
                .find(|task| task.node_id == outcome.node_id)
                .cloned();

            // 1. 收集节点状态（稍后批量持久化）
            self.pending_node_states.push(outcome.node_state.clone());

            // 2. 收集 Pregel writes。节点输出和后继触发都先进入 pending writes，
            // barrier 时统一 apply 到 channel checkpoint。
            if let Some(task) = &task {
                let mut pregel_writes = writes_from_node_outcome(
                    &self.compiled,
                    task,
                    outcome.output.as_ref(),
                    &outcome.next_nodes,
                    outcome.end_result.as_deref(),
                )?;
                if outcome.status == NodeRunStatus::Paused {
                    pregel_writes.push(interrupt_write(task, "node_paused"));
                }
                self.pregel_loop.put_writes(&task.id, pregel_writes.clone());
                for write in &pregel_writes {
                    if write.channel.starts_with("node_outputs.")
                        || write.channel.starts_with("data:")
                    {
                        self.pending_writes.push(PendingWrite {
                            source_node_id: write.source_node_id.clone(),
                            target: write.channel.clone(),
                            value: write.value.clone(),
                        });
                    }
                }
            }

            // 3. 保持现有 node_outputs 投影，供 UI 和旧节点输入读取。
            if let Some(output) = &outcome.output {
                self.pending_node_outputs
                    .push((outcome.node_id.clone(), output.clone()));
                // 同时更新内存中的 context
                self.run
                    .context
                    .node_outputs
                    .insert(outcome.node_id.clone(), output.clone());
            }

            // 4. 应用 side_effects
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

            // 5. 处理终止条件
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

            // 6. 计算 next_cursor（兼容 UI projection；调度事实来自 Pregel checkpoint）
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
        for w in &plan.waiting_nodes {
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

        // LangGraph-style durability slice: task writes are persisted before
        // the barrier checkpoint, so a restart can skip already-done tasks and
        // replay their writes.
        if !self.pregel_loop.pending_writes().is_empty() {
            run_state::write_pending_pregel_writes(
                ws,
                project,
                run_id,
                self.pregel_loop.pending_writes(),
            )?;
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

    fn record_superstep(
        &mut self,
        plan: &SuperstepPlan,
        status: SuperstepStatus,
        cursor_after: Vec<String>,
        event_kind: &str,
        message: impl Into<String>,
    ) -> Result<(), TaskGraphError> {
        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;
        let message = message.into();
        let detail = run_state::read_run_detail(ws, project, run_id)?;
        let node_statuses: BTreeMap<String, NodeRunStatus> = detail
            .nodes
            .iter()
            .map(|node| (node.node_id.clone(), node.status))
            .collect();
        let now = Utc::now().to_rfc3339();
        let pending_writes = self.pending_writes.clone();
        let pregel_parent_config = Some(checkpoint_config(
            run_id,
            self.run
                .checkpoint_ns
                .as_deref()
                .unwrap_or(DEFAULT_CHECKPOINT_NAMESPACE),
            self.pregel_loop.checkpoint().id.clone(),
        ));
        let pregel_checkpoint_metadata =
            checkpoint_metadata("loop", plan.superstep as i64, pregel_parent_config.as_ref());
        let commit = self.pregel_loop.commit_step(&PregelPreparedStep {
            superstep: plan.superstep,
            tasks: plan.pregel_tasks.clone(),
            replayed_tasks: plan.replayed_pregel_tasks.clone(),
            replayed_writes: plan.replayed_pregel_writes.clone(),
        })?;
        let executed_task_count = commit.executed_task_count;
        let replayed_task_count = commit.replayed_task_count;
        let completed_task_count = commit.completed_tasks.len();
        let applied_pregel_write_count = commit.applied_writes.len();
        let new_versions = commit.new_versions.clone();
        let pregel_loop_status = self.pregel_loop.status();
        let pregel_checkpoint = commit.checkpoint;
        let checkpoint = SuperstepCheckpoint {
            id: format!("checkpoint-{:06}", plan.superstep),
            run_id: run_id.clone(),
            superstep: plan.superstep,
            status,
            created_at: now.clone(),
            completed_at: Some(now),
            cursor_before: plan.cursor_before.clone(),
            cursor_after: cursor_after.clone(),
            ready_nodes: plan
                .ready_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect(),
            waiting_nodes: plan.waiting_nodes.clone(),
            node_statuses,
            context: detail.run.context,
            pending_writes: pending_writes.clone(),
            pregel_checkpoint: Some(pregel_checkpoint.clone()),
            pregel_parent_config,
            pregel_checkpoint_metadata: Some(pregel_checkpoint_metadata),
            message: Some(message.clone()),
        };
        run_state::write_superstep_checkpoint(ws, project, run_id, &checkpoint)?;
        self.run.current_superstep = plan.superstep;
        self.run.last_checkpoint_id = Some(checkpoint.id.clone());
        self.run.pregel_checkpoint = Some(pregel_checkpoint.clone());
        run_state::write_run_json(ws, project, run_id, &self.run)?;
        self.append_event(
            plan.superstep,
            "writes_committed",
            None,
            format!("{} pending write(s) committed at barrier", pending_writes.len()),
            serde_json::json!({
                "checkpoint_id": checkpoint.id.clone(),
                "count": pending_writes.len(),
                "targets": pending_writes.iter().map(|write| write.target.clone()).collect::<Vec<_>>(),
                "executed_task_count": executed_task_count,
                "replayed_task_count": replayed_task_count,
                "completed_task_count": completed_task_count,
                "applied_pregel_write_count": applied_pregel_write_count,
                "new_versions": new_versions,
                "pregel_loop_status": format!("{:?}", pregel_loop_status),
                "updated_channels": pregel_checkpoint.updated_channels.clone(),
            }),
        )?;
        self.append_event(
            plan.superstep,
            "checkpoint_saved",
            None,
            format!("checkpoint {} saved", checkpoint.id),
            serde_json::json!({
                "checkpoint_id": checkpoint.id.clone(),
                "status": status,
                "pregel_checkpoint_id": pregel_checkpoint.id,
            }),
        )?;
        self.append_event(
            plan.superstep,
            event_kind.to_string(),
            None,
            message,
            serde_json::json!({
                "checkpoint_id": checkpoint.id.clone(),
                "status": status,
                "cursor_after": cursor_after,
            }),
        )?;
        self.pending_writes.clear();
        run_state::clear_pending_pregel_writes(ws, project, run_id)?;
        Ok(())
    }

    fn append_event(
        &self,
        superstep: u64,
        kind: impl Into<String>,
        node_id: Option<String>,
        message: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<(), TaskGraphError> {
        run_state::append_run_event(
            &self.opts.workspace_root,
            &self.opts.project,
            &self.opts.run_id,
            superstep,
            kind,
            node_id,
            message,
            payload,
        )?;
        Ok(())
    }

    fn record_node_started_events(
        &self,
        superstep: u64,
        ready_nodes: &[ReadyNode],
    ) -> Result<(), TaskGraphError> {
        for node in ready_nodes {
            self.append_event(
                superstep,
                "node_started",
                Some(node.node_id.clone()),
                format!("node {} started", node.node_id),
                serde_json::json!({
                    "execution_mode": format!("{:?}", node.execution_mode).to_lowercase(),
                    "task_kind": format!("{:?}", node.task_kind).to_lowercase(),
                }),
            )?;
        }
        Ok(())
    }

    fn record_node_finished_events(
        &self,
        superstep: u64,
        outcomes: &[NodeOutcome],
    ) -> Result<(), TaskGraphError> {
        for outcome in outcomes {
            self.append_event(
                superstep,
                "node_finished",
                Some(outcome.node_id.clone()),
                format!(
                    "node {} finished with {:?}",
                    outcome.node_id, outcome.status
                ),
                serde_json::json!({
                    "status": outcome.status,
                    "next_nodes": outcome.next_nodes.clone(),
                    "has_output": outcome.output.is_some(),
                    "end_result": outcome.end_result.clone(),
                }),
            )?;
        }
        Ok(())
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

    /// 构建 borrowed edge_map（供 nodes 函数使用）。
    fn borrow_edge_map(&self) -> HashMap<String, Vec<&TaskGraphEdge>> {
        let mut map: HashMap<String, Vec<&TaskGraphEdge>> = HashMap::new();
        for edges in self.edge_map.values() {
            for edge in edges {
                map.entry(edge.from.clone()).or_default().push(edge);
            }
        }
        map
    }

    /// 构建 borrowed node_map（供 nodes 函数使用）。
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
