//! Interrupt and resume write helpers.

use serde_json::{json, Value};

use super::model::{PregelCheckpoint, PregelTask, PregelWrite};
use super::runtime_channels::{INTERRUPT_CHANNEL, RESUME_CHANNEL};

#[must_use]
pub fn should_interrupt(
    checkpoint: &PregelCheckpoint,
    interrupt_nodes: &[String],
    tasks: &[PregelTask],
) -> bool {
    if interrupt_nodes.is_empty() || tasks.is_empty() {
        return false;
    }

    let seen = checkpoint
        .versions_seen
        .get(INTERRUPT_CHANNEL)
        .cloned()
        .unwrap_or_default();
    let any_channel_updated = checkpoint
        .channel_versions
        .iter()
        .any(|(channel, version)| *version > seen.get(channel).copied().unwrap_or(0));
    if !any_channel_updated {
        return false;
    }

    let interrupts_all = interrupt_nodes.iter().any(|node| node == "*");
    tasks
        .iter()
        .any(|task| interrupts_all || interrupt_nodes.iter().any(|node| node == &task.node_id))
}
pub fn mark_interrupt_seen(checkpoint: &mut PregelCheckpoint) {
    checkpoint.versions_seen.insert(
        INTERRUPT_CHANNEL.to_string(),
        checkpoint.channel_versions.clone(),
    );
}
pub fn interrupt_write(task: &PregelTask, reason: impl Into<String>) -> PregelWrite {
    PregelWrite {
        task_id: task.id.clone(),
        source_node_id: task.node_id.clone(),
        channel: INTERRUPT_CHANNEL.to_string(),
        value: json!({
            "node_id": task.node_id.clone(),
            "reason": reason.into(),
        }),
    }
}
#[must_use]
pub fn resume_write(task: &PregelTask, value: Value) -> PregelWrite {
    PregelWrite {
        task_id: task.id.clone(),
        source_node_id: task.node_id.clone(),
        channel: RESUME_CHANNEL.to_string(),
        value,
    }
}
