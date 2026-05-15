//! LangGraph-shaped checkpoint namespace helpers.

pub const DEFAULT_CHECKPOINT_NAMESPACE: &str = "";
pub const CHECKPOINT_NAMESPACE_SEPARATOR: &str = "|";
pub const CHECKPOINT_NAMESPACE_END: &str = ":";

pub fn child_checkpoint_namespace(parent_ns: &str, child_name: &str) -> String {
    if parent_ns.is_empty() {
        child_name.to_string()
    } else {
        format!("{parent_ns}{CHECKPOINT_NAMESPACE_SEPARATOR}{child_name}")
    }
}
pub fn task_checkpoint_namespace(checkpoint_ns: &str, task_id: &str) -> String {
    format!("{checkpoint_ns}{CHECKPOINT_NAMESPACE_END}{task_id}")
}
