//! Task Graph HTTP API handlers.
//!
//! Extracted from the monolithic `http.rs` to improve maintainability.
//! Contains: catalog, CRUD, fork, run management, SSE events.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use bb_core::task_graph::{
    self, upgrade_graph, TaskGraphDefinition, TaskGraphError, TaskGraphScope,
};
use chrono::Utc;
use std::path::{Path as FsPath, PathBuf};

use super::AppState;

mod dto;
mod events;
mod prompt_files;
pub(crate) mod runner_config;
mod schedules;

use dto::{
    decode_create_body, decode_patch_body, is_active_run_status, TaskGraphApiError,
    TgCatalogResponse, TgCreateRunBody, TgForkBody, TgGraphRef, TgGraphResponse, TgResumeGateBody,
    TgRunCheckpointsResponse, TgRunDetailResponse, TgRunEventsResponse, TgRunResponse,
    TgRunStatusResponse, TgRunsListResponse, TgValidationStatus, TgWriteResponse,
};
use runner_config::build_runner_opts;

pub(super) use events::tg_run_events;
pub(super) use prompt_files::{tg_get_graph_inputs, tg_read_prompt_file, tg_write_prompt_file};
pub(crate) use schedules::{dispatch_due_schedules, spawn_schedule_dispatcher};
pub(super) use schedules::{
    tg_create_schedule, tg_delete_schedule, tg_list_schedules, tg_patch_schedule,
    tg_run_schedule_now,
};

// ─── Handlers ────────────────────────────────────────────────────────────────

pub(super) fn current_workspace_root(state: &AppState) -> Result<PathBuf, TaskGraphApiError> {
    state.workspace_root().map_err(|err| {
        TaskGraphApiError(TaskGraphError::InvalidGraphId(format!(
            "workspace unavailable: {err}"
        )))
    })
}

/// GET /api/projects/{project}/task-graphs — merged catalog
pub(super) async fn tg_list_catalog(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<TgCatalogResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let mut graphs = task_graph::list_system_graphs(&root)?;
    let mut project_graphs = task_graph::list_project_graphs(&root, &project)?;
    graphs.append(&mut project_graphs);
    Ok(Json(TgCatalogResponse { graphs }))
}

/// GET /api/projects/{project}/task-graphs/system/{graph_id} — read system graph
pub(super) async fn tg_read_system_graph(
    State(state): State<AppState>,
    Path((_project, graph_id)): Path<(String, String)>,
) -> Result<Json<TgGraphResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let graph = task_graph::read_system_graph(&root, &graph_id)?;
    Ok(Json(TgGraphResponse { graph }))
}

/// PATCH /api/projects/{project}/task-graphs/system/{graph_id} — update system graph (blackboard only)
pub(super) async fn tg_patch_system_graph(
    State(state): State<AppState>,
    Path((project, graph_id)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<TgWriteResponse>, TaskGraphApiError> {
    if project != "blackboard" {
        return Err(TaskGraphApiError(TaskGraphError::ReadonlyGraph(graph_id)));
    }
    let root = current_workspace_root(&state)?;
    let mut body = decode_patch_body(&body)?;

    // 自动填充 pins（前端可能未发送）
    upgrade_graph(&mut body.graph);

    let errors = task_graph::validate_graph(&body.graph);
    if !errors.is_empty() {
        return Err(TaskGraphApiError(TaskGraphError::ValidationFailed {
            count: errors.len(),
            errors,
        }));
    }

    let mut graph = body.graph;
    graph.id = graph_id;

    let saved = task_graph::save_system_graph(&root, graph, Some(body.expected_version))?;
    Ok(Json(TgWriteResponse {
        graph: saved,
        validation: TgValidationStatus {
            status: "passed",
            errors: Vec::new(),
        },
    }))
}

/// GET /api/projects/{project}/task-graphs/project/{graph_id} — read project graph
pub(super) async fn tg_read_project_graph(
    State(state): State<AppState>,
    Path((project, graph_id)): Path<(String, String)>,
) -> Result<Json<TgGraphResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let graph = task_graph::read_project_graph(&root, &project, &graph_id)?;
    Ok(Json(TgGraphResponse { graph }))
}

/// POST /api/projects/{project}/task-graphs — create project graph
pub(super) async fn tg_create_graph(
    State(state): State<AppState>,
    Path(project): Path<String>,
    body: Bytes,
) -> Result<Json<TgWriteResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let mut body = decode_create_body(&body)?;

    // 自动填充 pins（前端可能未发送）
    upgrade_graph(&mut body.graph);

    let errors = task_graph::validate_graph(&body.graph);
    if !errors.is_empty() {
        return Err(TaskGraphApiError(TaskGraphError::ValidationFailed {
            count: errors.len(),
            errors,
        }));
    }

    let saved = task_graph::save_project_graph(&root, &project, body.graph, None)?;
    Ok(Json(TgWriteResponse {
        graph: saved,
        validation: TgValidationStatus {
            status: "passed",
            errors: Vec::new(),
        },
    }))
}

/// PATCH /api/projects/{project}/task-graphs/project/{graph_id} — update
pub(super) async fn tg_patch_graph(
    State(state): State<AppState>,
    Path((project, graph_id)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<TgWriteResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let mut body = decode_patch_body(&body)?;

    // 自动填充 pins（前端可能未发送）
    upgrade_graph(&mut body.graph);

    let errors = task_graph::validate_graph(&body.graph);
    if !errors.is_empty() {
        return Err(TaskGraphApiError(TaskGraphError::ValidationFailed {
            count: errors.len(),
            errors,
        }));
    }

    let mut graph = body.graph;
    graph.id = graph_id;

    let saved =
        task_graph::save_project_graph(&root, &project, graph, Some(body.expected_version))?;
    Ok(Json(TgWriteResponse {
        graph: saved,
        validation: TgValidationStatus {
            status: "passed",
            errors: Vec::new(),
        },
    }))
}

/// DELETE /api/projects/{project}/task-graphs/project/{graph_id}
pub(super) async fn tg_delete_graph(
    State(state): State<AppState>,
    Path((project, graph_id)): Path<(String, String)>,
) -> Result<StatusCode, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::delete_project_graph(&root, &project, &graph_id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/projects/{project}/task-graphs/system/{graph_id}/fork
pub(super) async fn tg_fork_graph(
    State(state): State<AppState>,
    Path((project, graph_id)): Path<(String, String)>,
    Json(body): Json<TgForkBody>,
) -> Result<Json<TgGraphResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;

    let source = task_graph::read_system_graph(&root, &graph_id)?;

    let forked = TaskGraphDefinition {
        schema_version: source.schema_version,
        id: body.target_id,
        scope: TaskGraphScope::Project,
        title: body.title.unwrap_or_else(|| source.title.clone()),
        description: source.description,
        version: 1,
        readonly: false,
        origin: Some(bb_core::task_graph::types::GraphOrigin {
            scope: TaskGraphScope::System,
            id: graph_id,
            version: source.version,
            forked_at: Utc::now().to_rfc3339(),
        }),
        metadata: source.metadata,
        inputs: source.inputs,
        nodes: source.nodes,
        edges: source.edges,
        layout: source.layout,
    };

    let saved = task_graph::save_project_graph(&root, &project, forked, None)?;
    Ok(Json(TgGraphResponse { graph: saved }))
}

// ─── Task Graph Run Handlers ─────────────────────────────────────────────────

/// POST /api/projects/{project}/task-graph-runs — create and optionally start a run
pub(super) async fn tg_create_run(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(body): Json<TgCreateRunBody>,
) -> Result<(StatusCode, Json<TgRunResponse>), TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let run = create_task_graph_run(&root, &project, &body.graph, body.input, true)?;

    if run.created && !body.dry_run {
        spawn_task_graph_run(root, project.clone(), run.run.id.clone());
    }

    Ok((run.status, Json(TgRunResponse { run: run.run })))
}

pub(crate) struct CreatedTaskGraphRun {
    pub(crate) run: task_graph::TaskGraphRunSummary,
    pub(crate) created: bool,
    pub(crate) status: StatusCode,
}

pub(crate) fn create_task_graph_run(
    root: &FsPath,
    project: &str,
    graph_ref_input: &TgGraphRef,
    input: serde_json::Value,
    reuse_active: bool,
) -> Result<CreatedTaskGraphRun, TaskGraphApiError> {
    let graph = match graph_ref_input.scope.as_str() {
        "system" => task_graph::read_system_graph(root, &graph_ref_input.id)?,
        "project" => task_graph::read_project_graph(root, project, &graph_ref_input.id)?,
        _ => {
            return Err(TaskGraphApiError(TaskGraphError::InvalidGraphId(format!(
                "invalid scope: {}",
                graph_ref_input.scope
            ))));
        }
    };

    let graph_ref = task_graph::GraphRef {
        scope: graph.scope,
        id: graph.id.clone(),
        version: graph.version,
    };

    if reuse_active {
        if let Some(active_run) = task_graph::list_runs(root, project)?
            .into_iter()
            .find(|run| {
                is_active_run_status(run.status)
                    && run.graph_ref.scope == graph_ref.scope
                    && run.graph_ref.id == graph_ref.id
            })
        {
            return Ok(CreatedTaskGraphRun {
                run: active_run,
                created: false,
                status: StatusCode::OK,
            });
        }
    }

    let input = if input.is_null() {
        serde_json::json!({})
    } else {
        input
    };

    // ── 运行前完整性校验 ──
    let pre_run_errors = task_graph::validate_pre_run(&graph, root, project);
    if !pre_run_errors.is_empty() {
        return Err(TaskGraphApiError(TaskGraphError::ValidationFailed {
            count: pre_run_errors.len(),
            errors: pre_run_errors,
        }));
    }

    let run = task_graph::create_run(root, project, graph_ref, &graph, input)?;

    let summary = task_graph::TaskGraphRunSummary {
        id: run.id,
        project: run.project,
        graph_ref: run.graph_ref,
        status: run.status,
        created_at: run.created_at,
        started_at: run.started_at,
        updated_at: run.updated_at,
        completed_at: run.completed_at,
        current_superstep: run.current_superstep,
        last_checkpoint_id: run.last_checkpoint_id,
        checkpoint_ns: run.checkpoint_ns,
    };

    Ok(CreatedTaskGraphRun {
        run: summary,
        created: true,
        status: StatusCode::CREATED,
    })
}

pub(crate) fn spawn_task_graph_run(root: PathBuf, project: String, run_id: String) {
    let opts = build_runner_opts(&root, project, run_id, None);
    tokio::task::spawn_blocking(move || {
        // Fast-fail: 使用 catch_unwind 捕获 panic，确保 run 不会变成僵尸
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            task_graph::execute_run(&opts)
        }));

        match result {
            Ok(Ok(outcome)) => {
                eprintln!("bb task-graph run completed: {:?}", outcome);
            }
            Ok(Err(e)) => {
                // execute_run 内部已经会尝试标记 failed，这里做兜底
                eprintln!("bb task-graph run error: {}", e);
                let _ = task_graph::update_run_status(
                    &opts.workspace_root,
                    &opts.project,
                    &opts.run_id,
                    task_graph::RunStatus::Failed,
                );
            }
            Err(panic_info) => {
                // panic 兜底：确保 run 被标记为 failed
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                eprintln!("bb task-graph run panicked: {}", msg);
                let _ = task_graph::update_run_status(
                    &opts.workspace_root,
                    &opts.project,
                    &opts.run_id,
                    task_graph::RunStatus::Failed,
                );
            }
        }
    });
}

/// GET /api/projects/{project}/task-graph-runs — list runs
pub(super) async fn tg_list_runs(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<TgRunsListResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let runs = task_graph::list_runs(&root, &project)?;
    Ok(Json(TgRunsListResponse { runs }))
}

/// GET /api/projects/{project}/task-graph-runs/{run_id} — read run detail
pub(super) async fn tg_read_run(
    State(state): State<AppState>,
    Path((project, run_id)): Path<(String, String)>,
) -> Result<Json<TgRunDetailResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    let detail = task_graph::read_run_detail(&root, &project, &run_id)?;
    Ok(Json(TgRunDetailResponse { run: detail }))
}

/// GET /api/projects/{project}/task-graph-runs/{run_id}/event-log
pub(super) async fn tg_run_event_log(
    State(state): State<AppState>,
    Path((project, run_id)): Path<(String, String)>,
) -> Result<Json<TgRunEventsResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::read_run(&root, &project, &run_id)?;
    let events = task_graph::list_run_events(&root, &project, &run_id)?;
    Ok(Json(TgRunEventsResponse { events }))
}

/// GET /api/projects/{project}/task-graph-runs/{run_id}/checkpoints
pub(super) async fn tg_run_checkpoints(
    State(state): State<AppState>,
    Path((project, run_id)): Path<(String, String)>,
) -> Result<Json<TgRunCheckpointsResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::read_run(&root, &project, &run_id)?;
    let checkpoints = task_graph::list_superstep_checkpoints(&root, &project, &run_id)?;
    Ok(Json(TgRunCheckpointsResponse { checkpoints }))
}

/// POST /api/projects/{project}/task-graph-runs/{run_id}/gates/{node_id}/resume
pub(super) async fn tg_resume_gate(
    State(state): State<AppState>,
    Path((project, run_id, node_id)): Path<(String, String, String)>,
    Json(body): Json<TgResumeGateBody>,
) -> Result<Json<TgRunStatusResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;

    let run = task_graph::read_run(&root, &project, &run_id)?;
    if run.status != task_graph::RunStatus::Paused {
        return Err(TaskGraphApiError(TaskGraphError::InvalidRunTransition {
            from: format!("{:?}", run.status).to_lowercase(),
            to: "running".to_string(),
        }));
    }
    if let Some(ref paused) = run.paused {
        if paused.node_id != node_id {
            return Err(TaskGraphApiError(TaskGraphError::InvalidRunTransition {
                from: format!("paused at {}", paused.node_id),
                to: format!("resume at {}", node_id),
            }));
        }
    }

    let project_clone = project.clone();
    let run_id_clone = run_id.clone();
    let action = body.action.clone();
    let opts = build_runner_opts(&root, project_clone, run_id_clone, None);

    tokio::task::spawn_blocking(move || {
        // Fast-fail: 使用 catch_unwind 捕获 panic，确保 run 不会变成僵尸
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            task_graph::resume_run(&opts, &action)
        }));

        match result {
            Ok(Ok(outcome)) => {
                eprintln!("bb task-graph run resumed: {:?}", outcome);
            }
            Ok(Err(e)) => {
                // resume_run 内部已经会尝试标记 failed，这里做兜底
                eprintln!("bb task-graph resume error: {}", e);
                let _ = task_graph::update_run_status(
                    &opts.workspace_root,
                    &opts.project,
                    &opts.run_id,
                    task_graph::RunStatus::Failed,
                );
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                eprintln!("bb task-graph resume panicked: {}", msg);
                let _ = task_graph::update_run_status(
                    &opts.workspace_root,
                    &opts.project,
                    &opts.run_id,
                    task_graph::RunStatus::Failed,
                );
            }
        }
    });

    Ok(Json(TgRunStatusResponse {
        run_id,
        status: "running".to_string(),
    }))
}

/// POST /api/projects/{project}/task-graph-runs/{run_id}/cancel
///
/// 级联取消：同时取消该 run 及其所有活跃的子 run。
pub(super) async fn tg_cancel_run(
    State(state): State<AppState>,
    Path((project, run_id)): Path<(String, String)>,
) -> Result<Json<TgRunStatusResponse>, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::cancel_run_cascade(&root, &project, &run_id)?;
    Ok(Json(TgRunStatusResponse {
        run_id,
        status: "cancelled".to_string(),
    }))
}
