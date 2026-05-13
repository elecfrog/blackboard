use std::path::Path;

use chrono::Utc;

use super::super::types::TaskGraphError;
use super::model::{BranchDecision, LoopFrame, LoopIterationState, TaskGraphRun};
use super::{read_json, run_dir, run_json_path, write_json};

/// Push a loop frame before entering the loop body.
pub fn push_loop_frame(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    frame: LoopFrame,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    let mut run: TaskGraphRun = read_json(&run_file)?;
    run.context.loop_stack.push(frame);
    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(())
}

/// Pop the active frame when execution returns to its loop controller.
pub fn pop_loop_frame(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    loop_node_id: &str,
) -> Result<Option<LoopFrame>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    let mut run: TaskGraphRun = read_json(&run_file)?;
    let popped = if run
        .context
        .loop_stack
        .last()
        .map(|frame| frame.loop_node_id.as_str())
        == Some(loop_node_id)
    {
        run.context.loop_stack.pop()
    } else {
        None
    };
    if popped.is_some() {
        run.updated_at = Utc::now().to_rfc3339();
        write_json(&run_file, &run)?;
    }
    Ok(popped)
}

/// Record a branch decision in the run context.
pub fn record_branch_decision(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    decision: BranchDecision,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    let mut run: TaskGraphRun = read_json(&run_file)?;
    run.context.branch_decisions.push(decision);
    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(())
}

/// Record or update a loop iteration in the run context.
pub fn record_loop_iteration(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    state: LoopIterationState,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    let mut run: TaskGraphRun = read_json(&run_file)?;

    // Upsert: replace existing entry for same loop_node_id or append
    if let Some(existing) = run
        .context
        .loop_iterations
        .iter_mut()
        .find(|l| l.loop_node_id == state.loop_node_id)
    {
        *existing = state;
    } else {
        run.context.loop_iterations.push(state);
    }

    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(())
}
