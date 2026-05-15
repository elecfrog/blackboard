use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use bb_core::task_graph::{
    self, TaskGraphError, TaskSchedule, TaskScheduleCreate, TaskSchedulePatch,
};
use bb_core::Workspace;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

use super::dto::{is_active_run_status, TaskGraphApiError, TgGraphRef, TgRunResponse};
use super::{create_task_graph_run, current_workspace_root, spawn_task_graph_run};
use crate::http::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct TgSchedulesResponse {
    pub(crate) schedules: Vec<TaskSchedule>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgScheduleResponse {
    pub(crate) schedule: TaskSchedule,
}

pub(crate) async fn tg_list_schedules(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<TgSchedulesResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    refresh_project_schedule_statuses(&root, &project)?;
    let schedules = task_graph::list_schedules(&root, &project)?;
    Ok(Json(TgSchedulesResponse { schedules }))
}

pub(crate) async fn tg_create_schedule(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(body): Json<TaskScheduleCreate>,
) -> Result<(StatusCode, Json<TgScheduleResponse>), TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let schedule = task_graph::create_schedule(&root, &project, body)?;
    Ok((StatusCode::CREATED, Json(TgScheduleResponse { schedule })))
}

pub(crate) async fn tg_patch_schedule(
    State(state): State<AppState>,
    Path((project, schedule_id)): Path<(String, String)>,
    Json(body): Json<TaskSchedulePatch>,
) -> Result<Json<TgScheduleResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let schedule = task_graph::patch_schedule(&root, &project, &schedule_id, body)?;
    Ok(Json(TgScheduleResponse { schedule }))
}

pub(crate) async fn tg_delete_schedule(
    State(state): State<AppState>,
    Path((project, schedule_id)): Path<(String, String)>,
) -> Result<StatusCode, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::delete_schedule(&root, &project, &schedule_id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn tg_run_schedule_now(
    State(state): State<AppState>,
    Path((project, schedule_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<TgRunResponse>), TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let schedule = task_graph::read_schedule(&root, &project, &schedule_id)?;
    let planned_fire_at = Utc::now();
    let input = schedule_run_input(&schedule, planned_fire_at, "manual");
    let graph = graph_ref_for_schedule(&schedule);
    let created = create_task_graph_run(&root, &project, &graph, input, false)?;
    task_graph::mark_schedule_triggered(
        &root,
        &project,
        &schedule.id,
        planned_fire_at,
        &created.run.id,
        &run_status_label(created.run.status),
        Utc::now(),
    )?;
    if created.created {
        spawn_task_graph_run(root, project, created.run.id.clone());
    }
    Ok((created.status, Json(TgRunResponse { run: created.run })))
}

pub(crate) fn spawn_schedule_dispatcher(workspace: Workspace) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let workspace = workspace.clone();
            match tokio::task::spawn_blocking(move || dispatch_due_schedules(&workspace)).await {
                Ok(Ok(count)) if count > 0 => {
                    eprintln!("bb schedule dispatcher: dispatched={count}");
                }
                Ok(Ok(_)) => {}
                Ok(Err(err)) => eprintln!("bb schedule dispatcher error: {}", err.0),
                Err(err) => eprintln!("bb schedule dispatcher join error: {err}"),
            }
        }
    });
}

pub(crate) fn dispatch_due_schedules(workspace: &Workspace) -> Result<usize, TaskGraphApiError> {
    let root = workspace.root().to_path_buf();
    let now = Utc::now();
    let mut dispatched = 0;
    for project in workspace.list_projects().map_err(|err| {
        TaskGraphApiError(TaskGraphError::InvalidSchedule(format!(
            "list projects failed: {err}"
        )))
    })? {
        dispatched += dispatch_project_due_schedules(&root, &project.name, now)?;
    }
    Ok(dispatched)
}

fn dispatch_project_due_schedules(
    root: &std::path::Path,
    project: &str,
    now: DateTime<Utc>,
) -> Result<usize, TaskGraphApiError> {
    refresh_project_schedule_statuses(root, project)?;
    let schedules = task_graph::list_schedules(root, project)?;
    let mut dispatched = 0;

    for schedule in schedules {
        let Some(planned_fire_at) = task_graph::due_planned_fire_at(&schedule, now)? else {
            continue;
        };
        if !task_graph::claim_schedule_fire(root, project, &schedule.id, planned_fire_at)? {
            continue;
        }

        if let Some(run_id) = schedule.state.last_run_id.as_deref() {
            if let Ok(run) = task_graph::read_run(root, project, run_id) {
                if is_active_run_status(run.status) {
                    task_graph::mark_schedule_skipped(
                        root,
                        project,
                        &schedule.id,
                        planned_fire_at,
                        "previous schedule run is still active",
                        now,
                    )?;
                    continue;
                }
            }
        }

        let graph = graph_ref_for_schedule(&schedule);
        let input = schedule_run_input(&schedule, planned_fire_at, "schedule");
        match create_task_graph_run(root, project, &graph, input, false) {
            Ok(created) => {
                task_graph::mark_schedule_triggered(
                    root,
                    project,
                    &schedule.id,
                    planned_fire_at,
                    &created.run.id,
                    &run_status_label(created.run.status),
                    Utc::now(),
                )?;
                if created.created {
                    spawn_task_graph_run(
                        root.to_path_buf(),
                        project.to_string(),
                        created.run.id.clone(),
                    );
                }
                dispatched += 1;
            }
            Err(err) => {
                let message = err.0.to_string();
                task_graph::mark_schedule_failed(
                    root,
                    project,
                    &schedule.id,
                    planned_fire_at,
                    &message,
                    Utc::now(),
                )?;
                return Err(err);
            }
        }
    }

    Ok(dispatched)
}

fn refresh_project_schedule_statuses(
    root: &std::path::Path,
    project: &str,
) -> Result<(), TaskGraphApiError> {
    for schedule in task_graph::list_schedules(root, project)? {
        let Some(run_id) = schedule.state.last_run_id.as_deref() else {
            continue;
        };
        let Ok(run) = task_graph::read_run(root, project, run_id) else {
            continue;
        };
        let status = run_status_label(run.status);
        if schedule.state.last_status.as_deref() != Some(status.as_str()) {
            task_graph::refresh_schedule_last_status(root, project, &schedule.id, &status)?;
        }
    }
    Ok(())
}

fn graph_ref_for_schedule(schedule: &TaskSchedule) -> TgGraphRef {
    TgGraphRef {
        scope: match schedule.graph_ref.scope {
            task_graph::TaskGraphScope::System => "system".to_string(),
            task_graph::TaskGraphScope::Project => "project".to_string(),
        },
        id: schedule.graph_ref.id.clone(),
    }
}

fn schedule_run_input(
    schedule: &TaskSchedule,
    planned_fire_at: DateTime<Utc>,
    source: &str,
) -> Value {
    let trigger = json!({
        "type": "schedule",
        "source": source,
        "schedule_id": schedule.id,
        "schedule_name": schedule.name,
        "planned_fire_at": planned_fire_at.to_rfc3339(),
    });
    match schedule.input.clone() {
        Value::Object(mut map) => {
            map.insert("_trigger".to_string(), trigger);
            Value::Object(map)
        }
        Value::Null => json!({ "_trigger": trigger }),
        value => json!({ "value": value, "_trigger": trigger }),
    }
}

fn run_status_label(status: task_graph::RunStatus) -> String {
    match status {
        task_graph::RunStatus::Pending => "pending",
        task_graph::RunStatus::Running => "running",
        task_graph::RunStatus::Paused => "paused",
        task_graph::RunStatus::Succeeded => "succeeded",
        task_graph::RunStatus::Failed => "failed",
        task_graph::RunStatus::Cancelled => "cancelled",
    }
    .to_string()
}
