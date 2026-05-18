//! Reserved Pregel runtime channel names.

pub const TASKS_CHANNEL: &str = "__pregel_tasks";
pub const PUSH_TRIGGER: &str = "__pregel_push";
pub const PULL_TRIGGER: &str = "__pregel_pull";
pub const INTERRUPT_CHANNEL: &str = "__interrupt__";
pub const RESUME_CHANNEL: &str = "__resume__";
pub const ERROR_CHANNEL: &str = "__error__";
pub const RETURN_CHANNEL: &str = "__return__";
pub const PREVIOUS_CHANNEL: &str = "__previous__";

const RESERVED_RUNTIME_CHANNELS: &[&str] = &[
    TASKS_CHANNEL,
    PUSH_TRIGGER,
    PULL_TRIGGER,
    INTERRUPT_CHANNEL,
    RESUME_CHANNEL,
    ERROR_CHANNEL,
    RETURN_CHANNEL,
    PREVIOUS_CHANNEL,
];

pub(super) fn ignored_write_channel(channel: &str) -> bool {
    channel == ERROR_CHANNEL
}

pub(super) fn is_reserved_runtime_channel(channel: &str) -> bool {
    RESERVED_RUNTIME_CHANNELS.contains(&channel)
}

#[must_use]
pub fn reserved_runtime_channels() -> Vec<&'static str> {
    RESERVED_RUNTIME_CHANNELS.to_vec()
}
