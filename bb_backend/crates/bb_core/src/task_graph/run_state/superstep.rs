use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use chrono::Utc;

use super::lifecycle::update_run_checkpoint_pointer;
use super::model::{RunEvent, SuperstepCheckpoint, TaskGraphRun};
use super::{
    checkpoints_dir, pending_pregel_writes_path, read_json, run_dir, run_events_path, write_json,
};
use crate::task_graph::definition::types::TaskGraphError;
use crate::task_graph::pregel::{
    checkpoint_config, checkpoint_metadata, checkpoint_tuple, PregelCheckpointConfig,
    PregelCheckpointTuple, PregelWrite,
};

pub fn write_superstep_checkpoint(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    checkpoint: &SuperstepCheckpoint,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let checkpoints = checkpoints_dir(&dir);
    fs::create_dir_all(&checkpoints).map_err(|source| TaskGraphError::Io {
        path: checkpoints.clone(),
        source,
    })?;

    let file_name = format!("{:06}-{}.json", checkpoint.superstep, checkpoint.id);
    let path = checkpoints.join(file_name);
    write_json(&path, checkpoint)?;

    update_run_checkpoint_pointer(
        workspace_root,
        project,
        run_id,
        checkpoint.superstep,
        checkpoint.id.clone(),
    )?;
    Ok(())
}

pub fn list_superstep_checkpoints(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<Vec<SuperstepCheckpoint>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let checkpoints = checkpoints_dir(&dir);
    if !checkpoints.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for entry in fs::read_dir(&checkpoints).map_err(|source| TaskGraphError::Io {
        path: checkpoints.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| TaskGraphError::Io {
            path: checkpoints.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            results.push(read_json::<SuperstepCheckpoint>(&path)?);
        }
    }
    results.sort_by(|a, b| a.superstep.cmp(&b.superstep).then(a.id.cmp(&b.id)));
    Ok(results)
}

pub fn read_latest_superstep_checkpoint(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<Option<SuperstepCheckpoint>, TaskGraphError> {
    Ok(list_superstep_checkpoints(workspace_root, project, run_id)?
        .into_iter()
        .filter(|checkpoint| checkpoint.pregel_checkpoint.is_some())
        .last())
}

pub fn read_pregel_checkpoint_tuple(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    checkpoint_ns: &str,
) -> Result<Option<PregelCheckpointTuple>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run: TaskGraphRun = read_json(&super::run_json_path(&dir))?;
    let pending_writes = read_pending_pregel_writes(workspace_root, project, run_id)?;
    let checkpoints = list_superstep_checkpoints(workspace_root, project, run_id)?;
    let latest_index = checkpoints
        .iter()
        .rposition(|checkpoint| checkpoint.pregel_checkpoint.is_some());

    if let Some(index) = latest_index {
        let saved = &checkpoints[index];
        let saved_checkpoint = saved
            .pregel_checkpoint
            .clone()
            .expect("filtered checkpoint should contain Pregel state");
        if let Some(run_checkpoint) = run.pregel_checkpoint.clone() {
            if run_checkpoint_is_at_least_as_fresh(&run_checkpoint, &saved_checkpoint) {
                let parent_config = Some(checkpoint_config(
                    run_id,
                    checkpoint_ns,
                    saved_checkpoint.id.clone(),
                ));
                return Ok(Some(checkpoint_tuple(
                    run_id,
                    checkpoint_ns,
                    run_checkpoint,
                    checkpoint_metadata(
                        "update",
                        run.current_superstep as i64,
                        parent_config.as_ref(),
                    ),
                    parent_config,
                    pending_writes,
                )));
            }
        }
        let checkpoint = saved_checkpoint;
        let parent_config = saved
            .pregel_parent_config
            .clone()
            .or_else(|| inferred_parent_config(run_id, checkpoint_ns, &checkpoints, index));
        let metadata = saved.pregel_checkpoint_metadata.clone().unwrap_or_else(|| {
            checkpoint_metadata("loop", checkpoint.superstep as i64, parent_config.as_ref())
        });
        return Ok(Some(checkpoint_tuple(
            run_id,
            checkpoint_ns,
            checkpoint,
            metadata,
            parent_config,
            pending_writes,
        )));
    }

    let Some(checkpoint) = run.pregel_checkpoint else {
        return Ok(None);
    };
    Ok(Some(checkpoint_tuple(
        run_id,
        checkpoint_ns,
        checkpoint,
        checkpoint_metadata("input", run.current_superstep as i64, None),
        None,
        pending_writes,
    )))
}

fn run_checkpoint_is_at_least_as_fresh(
    run_checkpoint: &crate::task_graph::pregel::PregelCheckpoint,
    saved_checkpoint: &crate::task_graph::pregel::PregelCheckpoint,
) -> bool {
    run_checkpoint.superstep > saved_checkpoint.superstep
        || (run_checkpoint.superstep == saved_checkpoint.superstep
            && (run_checkpoint.channel_versions != saved_checkpoint.channel_versions
                || run_checkpoint.versions_seen != saved_checkpoint.versions_seen
                || run_checkpoint.channel_values != saved_checkpoint.channel_values))
}

fn inferred_parent_config(
    run_id: &str,
    checkpoint_ns: &str,
    checkpoints: &[SuperstepCheckpoint],
    index: usize,
) -> Option<PregelCheckpointConfig> {
    checkpoints
        .iter()
        .take(index)
        .rev()
        .find_map(|checkpoint| checkpoint.pregel_checkpoint.as_ref())
        .map(|checkpoint| checkpoint_config(run_id, checkpoint_ns, checkpoint.id.clone()))
}

pub fn write_pending_pregel_writes(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    writes: &[PregelWrite],
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let path = pending_pregel_writes_path(&dir);
    write_json(&path, &writes)
}

pub fn read_pending_pregel_writes(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<Vec<PregelWrite>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let path = pending_pregel_writes_path(&dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    read_json::<Vec<PregelWrite>>(&path)
}

pub fn clear_pending_pregel_writes(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let path = pending_pregel_writes_path(&dir);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|source| TaskGraphError::Io { path, source })
}

pub fn append_run_event(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    superstep: u64,
    kind: impl Into<String>,
    node_id: Option<String>,
    message: impl Into<String>,
    payload: serde_json::Value,
) -> Result<RunEvent, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    fs::create_dir_all(&dir).map_err(|source| TaskGraphError::Io {
        path: dir.clone(),
        source,
    })?;

    let path = run_events_path(&dir);
    let seq = count_event_lines(&path)? + 1;
    let event = RunEvent {
        id: format!("evt-{:06}", seq),
        seq,
        run_id: run_id.to_string(),
        superstep,
        kind: kind.into(),
        node_id,
        message: message.into(),
        payload,
        created_at: Utc::now().to_rfc3339(),
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| TaskGraphError::Io {
            path: path.clone(),
            source,
        })?;
    let line = serde_json::to_string(&event).expect("run event serialization should not fail");
    writeln!(file, "{line}").map_err(|source| TaskGraphError::Io { path, source })?;

    Ok(event)
}

pub fn list_run_events(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
) -> Result<Vec<RunEvent>, TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let path = run_events_path(&dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path).map_err(|source| TaskGraphError::Io {
        path: path.clone(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|source| TaskGraphError::Io {
            path: path.clone(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let event =
            serde_json::from_str::<RunEvent>(&line).map_err(|source| TaskGraphError::Parse {
                path: path.clone(),
                source,
            })?;
        events.push(event);
    }
    Ok(events)
}

fn count_event_lines(path: &Path) -> Result<u64, TaskGraphError> {
    if !path.exists() {
        return Ok(0);
    }
    let file = fs::File::open(path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut count = 0;
    for line in reader.lines() {
        let line = line.map_err(|source| TaskGraphError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if !line.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}
