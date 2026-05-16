//! Task Graph run state storage — run creation, snapshot, node state, logs, artifacts.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::TaskGraphError;

mod context;
mod fork_join;
mod lifecycle;
mod model;
mod node_io;
mod superstep;

pub use context::{pop_loop_frame, push_loop_frame, record_branch_decision, record_loop_iteration};
pub use fork_join::{
    count_exec_in_edges, detect_join_nodes, is_join_ready, record_branch_completion,
};
pub use lifecycle::{
    cancel_run_cascade, create_queued_run, create_run, list_runs, read_run, read_run_detail,
    resolve_graph_input, set_run_paused, update_run_status, write_run_json,
};
pub use model::*;
pub use node_io::{
    append_node_log, get_node_output, load_all_node_outputs, set_node_output, update_node_log_tail,
    update_node_state, write_artifact,
};
pub use superstep::{
    append_run_event, clear_pending_pregel_writes, list_run_events, list_superstep_checkpoints,
    read_graph_revision, read_latest_superstep_checkpoint, read_pending_pregel_writes,
    read_pregel_checkpoint_tuple, write_graph_revision, write_mutation_batch,
    write_pending_pregel_writes, write_superstep_checkpoint,
};

// ─── Path helpers ────────────────────────────────────────────────────────────

fn runs_root(workspace_root: &Path, project: &str) -> PathBuf {
    workspace_root
        .join("runtime")
        .join("task_graph_runs")
        .join(project)
}

fn run_dir(workspace_root: &Path, project: &str, run_id: &str) -> PathBuf {
    runs_root(workspace_root, project).join(run_id)
}

fn run_json_path(dir: &Path) -> PathBuf {
    dir.join("run.json")
}

fn snapshot_path(dir: &Path) -> PathBuf {
    dir.join("graph.snapshot.json")
}

fn compiled_snapshot_path(dir: &Path) -> PathBuf {
    dir.join("graph.compiled.json")
}

fn node_state_path(dir: &Path, node_id: &str) -> PathBuf {
    dir.join("nodes").join(format!("{}.json", node_id))
}

fn node_log_path(dir: &Path, node_id: &str) -> PathBuf {
    dir.join("logs").join(format!("{}.log", node_id))
}

fn artifacts_dir(dir: &Path) -> PathBuf {
    dir.join("artifacts")
}

fn checkpoints_dir(dir: &Path) -> PathBuf {
    dir.join("checkpoints")
}

fn run_events_path(dir: &Path) -> PathBuf {
    dir.join("events.jsonl")
}

fn pending_pregel_writes_path(dir: &Path) -> PathBuf {
    dir.join("pending_pregel_writes.json")
}

fn graph_revisions_dir(dir: &Path) -> PathBuf {
    dir.join("graph_revisions")
}

fn graph_revision_path(dir: &Path, revision: u64) -> PathBuf {
    graph_revisions_dir(dir).join(format!("{revision:06}.json"))
}

fn mutation_batches_dir(dir: &Path) -> PathBuf {
    dir.join("mutation_batches")
}

fn mutation_batch_path(dir: &Path, superstep: u64, batch_id: &str) -> PathBuf {
    mutation_batches_dir(dir).join(format!("{superstep:06}-{batch_id}.json"))
}

fn node_output_path(dir: &Path, node_id: &str) -> PathBuf {
    dir.join("node_outputs").join(format!("{}.json", node_id))
}

// ─── Run ID generation ───────────────────────────────────────────────────────

fn generate_run_id() -> String {
    let now = Utc::now();
    let ts = now.format("%Y%m%d-%H%M%S").to_string();
    let suffix = &Uuid::new_v4().to_string()[..8];
    format!("run-{}-{}", ts, suffix)
}

// ─── File helpers ────────────────────────────────────────────────────────────

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskGraphError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(value).expect("serialization should not fail");
    fs::write(path, json.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TaskGraphError> {
    let contents = fs::read_to_string(path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| TaskGraphError::Parse {
        path: path.to_path_buf(),
        source,
    })
}
