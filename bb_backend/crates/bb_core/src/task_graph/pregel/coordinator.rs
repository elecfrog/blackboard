//! Graph Coordinator — 控制平面单线程调度器。
//!
//! 负责 graph run 的完整生命周期：
//! 1. 从磁盘加载 RunState 到内存
//! 2. 主循环：Plan → Dispatch → Reduce → Persist
//! 3. 终止条件：Completed / Failed / Paused / Cancelled
//!
//! **核心原则**：所有最终 graph 状态变更都在 Coordinator 中发生（单写者）。
//! 执行平面（Executor）返回 `NodeOutcome`；长运行节点只允许写 streaming log / running projection。

use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    TaskGraphDefinition, TaskGraphEdge, TaskGraphError, TaskGraphNode, TaskGraphValidationError,
};
use crate::task_graph::pregel::{
    checkpoint_config, checkpoint_metadata, initial_checkpoint as initial_pregel_checkpoint,
    interrupt_write, writes_from_node_outcome, PregelLoop, PregelLoopConfig, PregelLoopStatus,
    PregelPreparedStep, DEFAULT_CHECKPOINT_NAMESPACE,
};
use crate::task_graph::run_state::{
    self, NodeRunStatus, PausedAction, PendingTaskEffect, PendingWrite, RunPaused, RunStatus,
    SuperstepCheckpoint, SuperstepStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::topology::{
    apply_mutation_requests, conflict_validation_error, graph_mutations_from_output,
    migrate_checkpoint_channels, GraphMutationBatch, GraphMutationBatchResult,
    GraphMutationConflict, GraphMutationRequest, GraphRevision,
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
    /// 本轮需要持久化的节点状态
    pending_node_states: Vec<TaskGraphRunNode>,
    /// 本轮需要持久化的节点输出
    pending_node_outputs: Vec<(String, serde_json::Value)>,
    /// 本轮 barrier 前收集到的 pending writes，写入 superstep checkpoint。
    pending_writes: Vec<PendingWrite>,
    pending_effects: Vec<PendingTaskEffect>,
    pending_graph_mutations: Vec<GraphMutationRequest>,
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
            let mut checkpoint = tuple.checkpoint;
            checkpoint.graph_revision = run.current_graph_revision;
            run.current_superstep = run.current_superstep.max(checkpoint.superstep);
            run.last_checkpoint_id = Some(tuple.config.checkpoint_id);
            run.pregel_checkpoint = Some(checkpoint.clone());
            (checkpoint, tuple.pending_writes)
        } else {
            let mut checkpoint = run
                .pregel_checkpoint
                .clone()
                .unwrap_or_else(|| initial_pregel_checkpoint(&compiled, run.context.input.clone()));
            checkpoint.graph_revision = run.current_graph_revision;
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
        Ok(Self {
            opts,
            graph,
            compiled,
            pregel_loop,
            run,
            edge_map,
            node_map,
            pending_node_states: Vec::new(),
            pending_node_outputs: Vec::new(),
            pending_writes: Vec::new(),
            pending_effects: Vec::new(),
            pending_graph_mutations: Vec::new(),
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
                    node_id: self.run.active_nodes.first().cloned().unwrap_or_default(),
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

            self.run.active_nodes = plan
                .ready_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect();
            run_state::write_run_json(ws, project, run_id, &self.run)?;

            self.append_event(
                superstep,
                "superstep_started",
                None,
                format!("superstep {superstep} started"),
                serde_json::json!({
                    "active_nodes": self.run.active_nodes.clone(),
                    "graph_revision": self.run.current_graph_revision,
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
                ReduceAction::Continue => {
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Succeeded,
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
                    self.run.active_nodes = vec![node_id.clone()];
                    run_state::write_run_json(ws, project, run_id, &self.run)?;
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Paused,
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
                        "run_failed",
                        message.clone(),
                    )?;
                    return Ok(RunOutcome::Failed { node_id, message });
                }
                ReduceAction::Cancelled => {
                    self.record_superstep(
                        &plan,
                        SuperstepStatus::Cancelled,
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
        self.run.active_nodes = plan
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
                let mut graph_mutations = outcome.graph_mutations.clone();
                graph_mutations.extend(graph_mutations_from_output(
                    outcome.output.as_ref(),
                    &task.id,
                    &outcome.node_id,
                ));
                let mut pregel_writes = writes_from_node_outcome(
                    &self.compiled,
                    task,
                    outcome.output.as_ref(),
                    &outcome.control,
                    outcome.end_result.as_deref(),
                    outcome.status == NodeRunStatus::Succeeded,
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
                self.pending_effects.push(PendingTaskEffect {
                    task_id: task.id.clone(),
                    source_node_id: outcome.node_id.clone(),
                    normal_writes: pregel_writes,
                    graph_mutations: graph_mutations.clone(),
                });
                self.pending_graph_mutations.extend(graph_mutations);
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

        Ok(ReduceAction::Continue)
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

    fn record_superstep(
        &mut self,
        plan: &SuperstepPlan,
        status: SuperstepStatus,
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
        let mut pregel_checkpoint = commit.checkpoint;
        let mutation_batch_id = self.apply_topology_mutations_after_commit(
            plan.superstep,
            &mut pregel_checkpoint,
            &commit.completed_tasks,
        )?;
        self.pregel_loop
            .replace_compiled_and_checkpoint(self.compiled.clone(), pregel_checkpoint.clone());
        let checkpoint = SuperstepCheckpoint {
            id: format!("checkpoint-{:06}", plan.superstep),
            run_id: run_id.clone(),
            superstep: plan.superstep,
            status,
            created_at: now.clone(),
            completed_at: Some(now),
            graph_revision_before: self
                .run
                .pregel_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.graph_revision)
                .unwrap_or(self.run.current_graph_revision),
            graph_revision_after: pregel_checkpoint.graph_revision,
            mutation_batch_id: mutation_batch_id.clone(),
            ready_nodes: plan
                .ready_nodes
                .iter()
                .map(|node| node.node_id.clone())
                .collect(),
            waiting_nodes: plan.waiting_nodes.clone(),
            node_statuses,
            context: detail.run.context,
            pending_writes: pending_writes.clone(),
            pending_effects: self.pending_effects.clone(),
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
                "pending_effect_count": self.pending_effects.len(),
                "executed_task_count": executed_task_count,
                "replayed_task_count": replayed_task_count,
                "completed_task_count": completed_task_count,
                "applied_pregel_write_count": applied_pregel_write_count,
                "new_versions": new_versions,
                "pregel_loop_status": format!("{:?}", pregel_loop_status),
                "updated_channels": pregel_checkpoint.updated_channels.clone(),
                "graph_revision": pregel_checkpoint.graph_revision,
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
                "graph_revision": pregel_checkpoint.graph_revision,
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
                "active_nodes": self.run.active_nodes.clone(),
                "graph_revision": pregel_checkpoint.graph_revision,
            }),
        )?;
        self.pending_writes.clear();
        self.pending_effects.clear();
        self.pending_graph_mutations.clear();
        run_state::clear_pending_pregel_writes(ws, project, run_id)?;
        Ok(())
    }

    fn apply_topology_mutations_after_commit(
        &mut self,
        superstep: u64,
        checkpoint: &mut crate::task_graph::pregel::PregelCheckpoint,
        completed_tasks: &[crate::task_graph::pregel::PregelTask],
    ) -> Result<Option<String>, TaskGraphError> {
        if self.pending_graph_mutations.is_empty() {
            checkpoint.graph_revision = self.run.current_graph_revision;
            return Ok(None);
        }

        let ws = &self.opts.workspace_root;
        let project = &self.opts.project;
        let run_id = &self.opts.run_id;
        let base_revision = self.run.current_graph_revision;
        let new_revision = base_revision + 1;
        let requests = self.pending_graph_mutations.clone();
        let apply = apply_mutation_requests(
            &self.graph,
            base_revision,
            new_revision,
            superstep,
            requests,
        );
        let batch = apply.batch;

        if let GraphMutationBatchResult::Rejected { conflicts } = &batch.result {
            run_state::write_mutation_batch(ws, project, run_id, &batch)?;
            self.append_topology_mutation_event(superstep, &batch)?;
            return Err(TaskGraphError::ValidationFailed {
                count: conflicts.len().max(1),
                errors: conflicts.iter().map(conflict_validation_error).collect(),
            });
        }

        let Some(candidate_graph) = apply.graph else {
            return Err(topology_validation_error(
                "topology_mutation_invalid_state",
                "mutation batch was applied without a candidate graph",
            ));
        };
        let candidate_compiled = match compile_graph_for_execution(&candidate_graph) {
            Ok(compiled) => compiled,
            Err(source) => {
                let rejected = rejected_compile_batch(batch, source.to_string());
                run_state::write_mutation_batch(ws, project, run_id, &rejected)?;
                self.append_topology_mutation_event(superstep, &rejected)?;
                return Err(topology_validation_error(
                    "topology_mutation_compile_failed",
                    "topology mutation candidate graph failed to compile",
                ));
            }
        };

        let completed_source_nodes = completed_tasks
            .iter()
            .map(|task| task.node_id.clone())
            .collect::<BTreeSet<_>>();
        let mut migrated = migrate_checkpoint_channels(
            &self.compiled,
            &candidate_compiled,
            checkpoint.clone(),
            &self.pending_graph_mutations,
            &completed_source_nodes,
        );
        migrated.graph_revision = new_revision;
        *checkpoint = migrated;

        let revision = GraphRevision {
            revision: new_revision,
            graph: candidate_graph.clone(),
            created_at: Utc::now().to_rfc3339(),
            parent_revision: Some(base_revision),
            mutation_batch_id: Some(batch.id.clone()),
        };
        run_state::write_graph_revision(ws, project, run_id, &revision)?;
        run_state::write_mutation_batch(ws, project, run_id, &batch)?;
        self.initialize_added_node_states(&batch)?;

        self.graph = candidate_graph;
        self.compiled = candidate_compiled;
        self.edge_map = build_owned_edge_map(&self.graph.edges);
        self.node_map = build_owned_node_map(&self.graph.nodes);
        self.run.current_graph_revision = new_revision;
        self.append_topology_mutation_event(superstep, &batch)?;
        Ok(Some(batch.id))
    }

    fn initialize_added_node_states(
        &self,
        batch: &GraphMutationBatch,
    ) -> Result<(), TaskGraphError> {
        let GraphMutationBatchResult::Applied { summary, .. } = &batch.result else {
            return Ok(());
        };
        for node_id in &summary.added_nodes {
            let state = TaskGraphRunNode {
                node_id: node_id.clone(),
                status: NodeRunStatus::Idle,
                started_at: None,
                completed_at: None,
                duration_ms: None,
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
            run_state::update_node_state(
                &self.opts.workspace_root,
                &self.opts.project,
                &self.opts.run_id,
                &state,
            )?;
        }
        Ok(())
    }

    fn append_topology_mutation_event(
        &self,
        superstep: u64,
        batch: &GraphMutationBatch,
    ) -> Result<(), TaskGraphError> {
        let (graph_revision_after, result_status) = match &batch.result {
            GraphMutationBatchResult::Applied { new_revision, .. } => (*new_revision, "applied"),
            GraphMutationBatchResult::Rejected { .. } => (batch.base_revision, "rejected"),
        };
        self.append_event(
            superstep,
            "topology_mutation",
            None,
            format!("topology mutation batch {} {}", batch.id, result_status),
            serde_json::json!({
                "batch_id": batch.id,
                "graph_revision_before": batch.base_revision,
                "graph_revision_after": graph_revision_after,
                "result": batch.result,
            }),
        )
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
                    "control": outcome.control.iter().map(|item| format!("{:?}", item)).collect::<Vec<_>>(),
                    "graph_mutations": outcome.graph_mutations.len(),
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

fn topology_validation_error(code: &str, message: &str) -> TaskGraphError {
    TaskGraphError::ValidationFailed {
        count: 1,
        errors: vec![TaskGraphValidationError {
            path: "topology_mutation".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }],
    }
}

fn rejected_compile_batch(mut batch: GraphMutationBatch, message: String) -> GraphMutationBatch {
    batch.result = GraphMutationBatchResult::Rejected {
        conflicts: vec![GraphMutationConflict {
            code: "topology_mutation_compile_failed".to_string(),
            message,
            request_ids: batch
                .requests
                .iter()
                .map(|request| request.id.clone())
                .collect(),
            node_id: None,
            edge_id: None,
        }],
    };
    batch
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::task_graph::definition::types::{
        EdgeKind, NodeType, TaskGraphDefinition, TaskGraphEdge, TaskGraphNode, TaskGraphScope,
    };
    use crate::task_graph::pregel::{prepare_next_tasks, PregelTask, PregelTaskKind};
    use crate::task_graph::run_state::GraphRef;
    use crate::task_graph::topology::{mutation_batch_id, GraphMutationOp};

    #[test]
    fn topology_mutation_adds_graph_revision_and_schedules_new_node() {
        let tmp = TempDir::new().unwrap();
        let graph = mutation_base_graph();
        let graph_ref = GraphRef {
            scope: TaskGraphScope::Project,
            id: graph.id.clone(),
            version: graph.version,
        };
        let run = run_state::create_run(
            tmp.path(),
            "test-project",
            graph_ref,
            &graph,
            json!({ "dynamic": "created" }),
        )
        .unwrap();
        let opts = runner_options(tmp.path().to_path_buf(), run.id.clone());
        let mut coordinator = GraphCoordinator::load(&opts).unwrap();
        let mut checkpoint = coordinator.run.pregel_checkpoint.clone().unwrap();
        checkpoint
            .channel_values
            .insert("branch:to:end".to_string(), json!("mutator"));
        checkpoint
            .channel_versions
            .insert("branch:to:end".to_string(), 2);
        checkpoint.updated_channels = vec!["branch:to:end".to_string()];

        coordinator.pending_graph_mutations = vec![
            GraphMutationRequest {
                id: "add-dynamic-node".to_string(),
                source_task_id: "task-mutator".to_string(),
                source_node_id: "mutator".to_string(),
                op: GraphMutationOp::AddNode {
                    node: TaskGraphNode {
                        id: "dynamic".to_string(),
                        node_type: NodeType::InputVar,
                        label: "Dynamic".to_string(),
                        description: None,
                        position: None,
                        config: json!({ "input_id": "dynamic" }),
                        pins: vec![],
                    },
                },
                reason: None,
            },
            GraphMutationRequest {
                id: "add-mutator-dynamic-edge".to_string(),
                source_task_id: "task-mutator".to_string(),
                source_node_id: "mutator".to_string(),
                op: GraphMutationOp::AddEdge {
                    edge: TaskGraphEdge {
                        id: "mutator__dynamic".to_string(),
                        from: "mutator".to_string(),
                        to: "dynamic".to_string(),
                        kind: EdgeKind::Exec,
                        label: None,
                        source_handle: None,
                        target_handle: None,
                        from_pin: None,
                        to_pin: None,
                    },
                },
                reason: None,
            },
        ];

        let batch_id = coordinator
            .apply_topology_mutations_after_commit(
                1,
                &mut checkpoint,
                &[PregelTask {
                    id: "task-mutator".to_string(),
                    node_id: "mutator".to_string(),
                    kind: PregelTaskKind::Pull,
                    triggers: vec!["branch:to:mutator".to_string()],
                    path: vec![],
                    input: json!(null),
                }],
            )
            .unwrap()
            .unwrap();

        assert_eq!(batch_id, mutation_batch_id(1));
        assert_eq!(coordinator.run.current_graph_revision, 1);
        assert_eq!(checkpoint.graph_revision, 1);
        assert!(coordinator.compiled.nodes.contains_key("dynamic"));
        assert!(checkpoint
            .updated_channels
            .contains(&"branch:to:dynamic".to_string()));

        let next = prepare_next_tasks(&coordinator.compiled, &checkpoint, 2);
        assert!(next.tasks.iter().any(|task| task.node_id == "dynamic"));
        assert!(next.tasks.iter().any(|task| task.node_id == "end"));

        let revision = run_state::read_graph_revision(tmp.path(), "test-project", &run.id, 1)
            .unwrap()
            .unwrap();
        assert_eq!(revision.revision, 1);
        assert!(revision.graph.nodes.iter().any(|node| node.id == "dynamic"));

        let batch_path = tmp
            .path()
            .join("runtime/task_graph_runs/test-project")
            .join(&run.id)
            .join("mutation_batches")
            .join(format!("000001-{}.json", mutation_batch_id(1)));
        assert!(batch_path.exists());

        let node_state_path = tmp
            .path()
            .join("runtime/task_graph_runs/test-project")
            .join(&run.id)
            .join("nodes/dynamic.json");
        assert!(node_state_path.exists());
    }

    fn mutation_base_graph() -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "mutation-test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Mutation Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![
                TaskGraphNode {
                    id: "start".to_string(),
                    node_type: NodeType::Start,
                    label: "Start".to_string(),
                    description: None,
                    position: None,
                    config: json!({}),
                    pins: vec![],
                },
                TaskGraphNode {
                    id: "mutator".to_string(),
                    node_type: NodeType::InputVar,
                    label: "Mutator".to_string(),
                    description: None,
                    position: None,
                    config: json!({ "input_id": "mutator" }),
                    pins: vec![],
                },
                TaskGraphNode {
                    id: "end".to_string(),
                    node_type: NodeType::End,
                    label: "End".to_string(),
                    description: None,
                    position: None,
                    config: json!({ "result": "succeeded" }),
                    pins: vec![],
                },
            ],
            edges: vec![exec_edge("start", "mutator"), exec_edge("mutator", "end")],
            layout: None,
        }
    }

    fn exec_edge(from: &str, to: &str) -> TaskGraphEdge {
        TaskGraphEdge {
            id: format!("{from}__{to}"),
            from: from.to_string(),
            to: to.to_string(),
            kind: EdgeKind::Exec,
            label: None,
            source_handle: None,
            target_handle: None,
            from_pin: None,
            to_pin: None,
        }
    }

    fn runner_options(workspace_root: std::path::PathBuf, run_id: String) -> RunnerOptions {
        RunnerOptions {
            workspace_root,
            scripts_dir: std::path::PathBuf::from("scripts"),
            project: "test-project".to_string(),
            run_id,
            codex_path: "codex".to_string(),
            codebuddy_path: "codebuddy".to_string(),
            opencode_path: "opencode".to_string(),
            opencode_config_content: None,
            model: None,
            node_timeout: std::time::Duration::from_secs(1),
            run_timeout: std::time::Duration::from_secs(10),
            dry_run: true,
            custom_env: Default::default(),
            custom_args: Vec::new(),
            mcp_servers: Vec::new(),
            skills: Vec::new(),
        }
    }
}
