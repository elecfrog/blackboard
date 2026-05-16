//! PregelLoop state object for TaskGraph.
//!
//! This is the Blackboard-shaped counterpart of LangGraph's `PregelLoop`.
//! It deliberately starts small: the loop owns the active checkpoint,
//! recovered pending task writes, step preparation, and barrier commit. The
//! coordinator still owns runtime execution and durable run files.

use std::collections::{BTreeMap, BTreeSet};

use crate::task_graph::compile::compiler::CompiledGraph;
use crate::task_graph::definition::types::TaskGraphError;

use super::{
    apply_writes, mark_interrupt_seen, new_channel_versions,
    prepare_next_tasks_with_pending_writes, should_interrupt, PregelCheckpoint, PregelPreparedStep,
    PregelTask, PregelWrite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PregelLoopStatus {
    Pending,
    Done,
    InterruptBefore,
    InterruptAfter,
    OutOfSteps,
}

#[derive(Debug, Clone, Default)]
pub struct PregelLoopConfig {
    pub stop: Option<u64>,
    pub interrupt_before: Vec<String>,
    pub interrupt_after: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PregelLoopCommit {
    pub checkpoint: PregelCheckpoint,
    pub completed_tasks: Vec<PregelTask>,
    pub applied_writes: Vec<PregelWrite>,
    pub new_versions: BTreeMap<String, u64>,
    pub replayed_task_count: usize,
    pub executed_task_count: usize,
}

pub struct PregelLoop {
    compiled: CompiledGraph,
    checkpoint: PregelCheckpoint,
    checkpoint_previous_versions: BTreeMap<String, u64>,
    pending_writes: Vec<PregelWrite>,
    step: u64,
    stop: Option<u64>,
    interrupt_before: Vec<String>,
    interrupt_after: Vec<String>,
    status: PregelLoopStatus,
}

impl PregelLoop {
    pub fn new(
        compiled: CompiledGraph,
        checkpoint: PregelCheckpoint,
        pending_writes: Vec<PregelWrite>,
    ) -> Self {
        Self::with_stop(compiled, checkpoint, pending_writes, None)
    }

    pub fn with_stop(
        compiled: CompiledGraph,
        checkpoint: PregelCheckpoint,
        pending_writes: Vec<PregelWrite>,
        stop: Option<u64>,
    ) -> Self {
        Self::with_config(
            compiled,
            checkpoint,
            pending_writes,
            PregelLoopConfig {
                stop,
                ..PregelLoopConfig::default()
            },
        )
    }

    pub fn with_config(
        compiled: CompiledGraph,
        checkpoint: PregelCheckpoint,
        pending_writes: Vec<PregelWrite>,
        config: PregelLoopConfig,
    ) -> Self {
        let checkpoint_previous_versions = checkpoint.channel_versions.clone();
        Self {
            compiled,
            checkpoint,
            checkpoint_previous_versions,
            pending_writes,
            step: 0,
            stop: config.stop,
            interrupt_before: config.interrupt_before,
            interrupt_after: config.interrupt_after,
            status: PregelLoopStatus::Pending,
        }
    }

    pub fn compiled(&self) -> &CompiledGraph {
        &self.compiled
    }

    pub fn checkpoint(&self) -> &PregelCheckpoint {
        &self.checkpoint
    }

    pub fn pending_writes(&self) -> &[PregelWrite] {
        &self.pending_writes
    }

    pub fn replace_compiled_and_checkpoint(
        &mut self,
        compiled: CompiledGraph,
        checkpoint: PregelCheckpoint,
    ) {
        self.compiled = compiled;
        self.checkpoint_previous_versions = checkpoint.channel_versions.clone();
        self.checkpoint = checkpoint;
    }

    pub fn checkpoint_previous_versions(&self) -> &BTreeMap<String, u64> {
        &self.checkpoint_previous_versions
    }

    pub fn step(&self) -> u64 {
        self.step
    }

    pub fn stop(&self) -> Option<u64> {
        self.stop
    }

    pub fn status(&self) -> PregelLoopStatus {
        self.status
    }

    pub fn prepare_next(&mut self, next_superstep: u64) -> PregelPreparedStep {
        self.step = next_superstep;
        if self
            .stop
            .is_some_and(|stop_superstep| next_superstep > stop_superstep)
        {
            self.status = PregelLoopStatus::OutOfSteps;
            return PregelPreparedStep {
                superstep: next_superstep,
                tasks: Vec::new(),
                replayed_tasks: Vec::new(),
                replayed_writes: Vec::new(),
            };
        }

        let prepared = prepare_next_tasks_with_pending_writes(
            &self.compiled,
            &self.checkpoint,
            &self.pending_writes,
            next_superstep,
        );
        if prepared.tasks.is_empty() && prepared.replayed_tasks.is_empty() {
            self.status = PregelLoopStatus::Done;
        } else if should_interrupt(&self.checkpoint, &self.interrupt_before, &prepared.tasks) {
            mark_interrupt_seen(&mut self.checkpoint);
            self.status = PregelLoopStatus::InterruptBefore;
        } else {
            self.status = PregelLoopStatus::Pending;
        }
        prepared
    }

    pub fn put_writes(&mut self, task_id: &str, writes: Vec<PregelWrite>) {
        if writes.is_empty() {
            return;
        }
        self.pending_writes.retain(|write| write.task_id != task_id);
        self.pending_writes.extend(writes);
    }

    pub fn commit_step(
        &mut self,
        prepared: &PregelPreparedStep,
    ) -> Result<PregelLoopCommit, TaskGraphError> {
        let mut completed_tasks = prepared.replayed_tasks.clone();
        completed_tasks.extend(prepared.tasks.clone());

        let executed_task_ids = prepared
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<BTreeSet<_>>();
        let executed_writes = self
            .pending_writes
            .iter()
            .filter(|write| executed_task_ids.contains(write.task_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        let mut applied_writes = prepared.replayed_writes.clone();
        applied_writes.extend(executed_writes);

        let checkpoint = apply_writes(
            &self.compiled,
            &self.checkpoint,
            &completed_tasks,
            &applied_writes,
            prepared.superstep,
        )?;
        let new_versions = new_channel_versions(
            &self.checkpoint_previous_versions,
            &checkpoint.channel_versions,
        );

        self.step = prepared.superstep;
        self.checkpoint = checkpoint.clone();
        self.checkpoint_previous_versions = checkpoint.channel_versions.clone();
        self.clear_committed_pending_writes(&completed_tasks);
        if should_interrupt(&self.checkpoint, &self.interrupt_after, &completed_tasks) {
            mark_interrupt_seen(&mut self.checkpoint);
            self.status = PregelLoopStatus::InterruptAfter;
        } else {
            self.status = PregelLoopStatus::Pending;
        }

        Ok(PregelLoopCommit {
            checkpoint: self.checkpoint.clone(),
            completed_tasks,
            applied_writes,
            new_versions,
            replayed_task_count: prepared.replayed_tasks.len(),
            executed_task_count: prepared.tasks.len(),
        })
    }

    fn clear_committed_pending_writes(&mut self, completed_tasks: &[PregelTask]) {
        if self.pending_writes.is_empty() {
            return;
        }
        let task_ids = completed_tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<BTreeSet<_>>();
        self.pending_writes
            .retain(|write| !task_ids.contains(write.task_id.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::task_graph::compile::compiler::compile_graph;
    use crate::task_graph::definition::types::{
        EdgeKind, NodeType, TaskGraphDefinition, TaskGraphEdge, TaskGraphNode, TaskGraphScope,
    };
    use crate::task_graph::pregel::{initial_checkpoint, writes_from_node_outcome};

    #[test]
    fn pregel_loop_prepares_and_commits_a_superstep() {
        let compiled = compile_graph(&linear_graph()).unwrap();
        let checkpoint = initial_checkpoint(&compiled, json!({"value": "go"}));
        let mut loop_state = PregelLoop::new(compiled.clone(), checkpoint, Vec::new());

        let prepared = loop_state.prepare_next(1);
        assert_eq!(prepared.tasks.len(), 1);
        assert_eq!(prepared.tasks[0].node_id, "start");

        let writes =
            writes_from_node_outcome(&compiled, &prepared.tasks[0], None, &[], None, true).unwrap();
        loop_state.put_writes(&prepared.tasks[0].id, writes);
        let commit = loop_state.commit_step(&prepared).unwrap();

        assert_eq!(commit.executed_task_count, 1);
        assert_eq!(commit.replayed_task_count, 0);
        assert!(commit.new_versions.contains_key("branch:to:read"));
        assert!(commit
            .checkpoint
            .channel_values
            .contains_key("branch:to:read"));
        assert_eq!(loop_state.checkpoint().superstep, 1);
    }

    #[test]
    fn pregel_loop_replays_pending_writes_and_clears_committed_entries() {
        let compiled = compile_graph(&linear_graph()).unwrap();
        let checkpoint = initial_checkpoint(&compiled, json!({"value": "go"}));
        let prepared = prepare_next_tasks_with_pending_writes(&compiled, &checkpoint, &[], 1);
        let pending =
            writes_from_node_outcome(&compiled, &prepared.tasks[0], None, &[], None, true).unwrap();
        let mut loop_state = PregelLoop::new(compiled, checkpoint, pending);

        let recovered = loop_state.prepare_next(1);
        assert!(recovered.tasks.is_empty());
        assert_eq!(recovered.replayed_tasks.len(), 1);

        let commit = loop_state.commit_step(&recovered).unwrap();

        assert_eq!(commit.executed_task_count, 0);
        assert_eq!(commit.replayed_task_count, 1);
        assert!(loop_state.pending_writes().is_empty());
        assert!(loop_state
            .checkpoint()
            .channel_values
            .contains_key("branch:to:read"));
    }

    #[test]
    fn pregel_loop_reports_out_of_steps_before_preparing_tasks() {
        let compiled = compile_graph(&linear_graph()).unwrap();
        let checkpoint = initial_checkpoint(&compiled, json!({"value": "go"}));
        let mut loop_state = PregelLoop::with_stop(compiled, checkpoint, Vec::new(), Some(0));

        let prepared = loop_state.prepare_next(1);

        assert_eq!(loop_state.step(), 1);
        assert_eq!(loop_state.stop(), Some(0));
        assert_eq!(loop_state.status(), PregelLoopStatus::OutOfSteps);
        assert!(prepared.tasks.is_empty());
        assert!(prepared.replayed_tasks.is_empty());
        assert!(prepared.replayed_writes.is_empty());
    }

    #[test]
    fn pregel_loop_interrupts_before_matching_task_once() {
        let compiled = compile_graph(&linear_graph()).unwrap();
        let checkpoint = initial_checkpoint(&compiled, json!({"value": "go"}));
        let mut loop_state = PregelLoop::with_config(
            compiled,
            checkpoint,
            Vec::new(),
            PregelLoopConfig {
                interrupt_before: vec!["start".to_string()],
                ..PregelLoopConfig::default()
            },
        );

        let prepared = loop_state.prepare_next(1);

        assert_eq!(prepared.tasks.len(), 1);
        assert_eq!(loop_state.status(), PregelLoopStatus::InterruptBefore);
        assert!(loop_state
            .checkpoint()
            .versions_seen
            .contains_key(crate::task_graph::pregel::INTERRUPT_CHANNEL));

        let prepared = loop_state.prepare_next(1);

        assert_eq!(prepared.tasks.len(), 1);
        assert_eq!(loop_state.status(), PregelLoopStatus::Pending);
    }

    fn linear_graph() -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "pregel-loop-test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Pregel Loop Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![
                node("start", NodeType::Start),
                node("read", NodeType::InputVar),
                node("end", NodeType::End),
            ],
            edges: vec![edge("start", "read"), edge("read", "end")],
            layout: None,
        }
    }

    fn node(id: &str, node_type: NodeType) -> TaskGraphNode {
        TaskGraphNode {
            id: id.to_string(),
            node_type,
            label: id.to_string(),
            description: None,
            position: None,
            config: match node_type {
                NodeType::InputVar => json!({ "input_id": "value" }),
                NodeType::End => json!({ "result": "succeeded" }),
                _ => json!({}),
            },
            pins: vec![],
        }
    }

    fn edge(from: &str, to: &str) -> TaskGraphEdge {
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
}
