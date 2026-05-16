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
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
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
    let run = create_task_graph_run(
        &root,
        &project,
        &body.graph,
        body.input,
        body.intent.as_deref(),
        true,
    )?;

    if run.created && !body.dry_run && run.run.status != task_graph::RunStatus::Queued {
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
    intent: Option<&str>,
    _reuse_active: bool,
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
    let policy = graph_run_policy(&graph);
    let matching_runs: Vec<_> = task_graph::list_runs(root, project)?
        .into_iter()
        .filter(|run| run.graph_ref.scope == graph_ref.scope && run.graph_ref.id == graph_ref.id)
        .collect();
    let active_slot_count = matching_runs
        .iter()
        .filter(|run| occupies_concurrency_slot(run.status))
        .count() as u32;
    let has_queued_run = matching_runs
        .iter()
        .any(|run| run.status == task_graph::RunStatus::Queued);
    let capacity_full = active_slot_count >= policy.effective_max_concurrent_runs();

    if capacity_full && !policy.queue_enabled {
        if let Some(active_run) = matching_runs
            .iter()
            .find(|run| is_active_run_status(run.status))
            .cloned()
        {
            return Ok(CreatedTaskGraphRun {
                run: active_run,
                created: false,
                status: StatusCode::OK,
            });
        }
    }

    let input = prepare_pre_start_input(&graph, input, intent);

    // ── 运行前完整性校验 ──
    let pre_run_errors = task_graph::validate_pre_run(&graph, root, project);
    if !pre_run_errors.is_empty() {
        return Err(TaskGraphApiError(TaskGraphError::ValidationFailed {
            count: pre_run_errors.len(),
            errors: pre_run_errors,
        }));
    }

    let should_queue = policy.queue_enabled && (capacity_full || has_queued_run);
    let (run, status) = if should_queue {
        let queue_deadline_at = queue_deadline_at(&policy);
        let run = task_graph::create_queued_run(
            root,
            project,
            graph_ref,
            &graph,
            input,
            queue_deadline_at.clone(),
        )?;
        task_graph::append_run_event(
            root,
            project,
            &run.id,
            0,
            "run_queued",
            None,
            "Run queued because graph concurrency capacity is full",
            json!({
                "graph_scope": run.graph_ref.scope,
                "graph_id": run.graph_ref.id,
                "active_slot_count": active_slot_count,
                "max_concurrent_runs": policy.effective_max_concurrent_runs(),
                "queued_at": run.queued_at,
                "queue_deadline_at": queue_deadline_at,
            }),
        )?;
        (run, StatusCode::ACCEPTED)
    } else {
        (
            task_graph::create_run(root, project, graph_ref, &graph, input)?,
            StatusCode::CREATED,
        )
    };
    let summary = summarize_run(run);

    Ok(CreatedTaskGraphRun {
        run: summary,
        created: true,
        status,
    })
}

fn prepare_pre_start_input(
    graph: &TaskGraphDefinition,
    explicit_input: Value,
    intent: Option<&str>,
) -> Value {
    let mut input = serde_json::Map::new();
    if let Some(params) = graph.inputs.as_ref() {
        for param in params {
            input.insert(param.id.clone(), param.default_value.clone());
        }
    }

    if let Some(intent) = intent.map(str::trim).filter(|value| !value.is_empty()) {
        let inferred = infer_input_from_intent(graph.inputs.as_deref().unwrap_or(&[]), intent);
        for (key, value) in inferred {
            if meaningful_input_value(&value) {
                input.insert(key, value);
            }
        }
    }

    if let Some(map) = explicit_input.as_object() {
        for (key, value) in map {
            if meaningful_input_value(value) {
                input.insert(key.clone(), value.clone());
            }
        }
    }

    Value::Object(input)
}

fn infer_input_from_intent(
    params: &[task_graph::TaskGraphInputParam],
    intent: &str,
) -> serde_json::Map<String, Value> {
    let mut input = serde_json::Map::new();
    let paths = extract_windows_paths(intent);
    let output_path = path_after_marker(intent, "输出到")
        .or_else(|| path_after_marker(intent, "输出至"))
        .or_else(|| path_after_marker(intent, "写到"))
        .or_else(|| path_after_marker(intent, "保存到"))
        .or_else(|| path_after_marker(intent, "Output to"))
        .or_else(|| path_after_marker(intent, "output to"))
        .or_else(|| {
            if paths.len() >= 2 {
                paths.last().cloned()
            } else {
                None
            }
        });
    let primary_path = paths
        .iter()
        .find(|path| Some(path.as_str()) != output_path.as_deref())
        .cloned();
    let extra_paths = paths
        .iter()
        .filter(|path| Some(path.as_str()) != primary_path.as_deref())
        .filter(|path| Some(path.as_str()) != output_path.as_deref())
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();

    for param in params {
        let id = param.id.as_str();
        let normalized = normalize_input_id(id);
        if matches!(
            normalized.as_str(),
            "request" | "prompt" | "task" | "description"
        ) {
            input.insert(id.to_string(), Value::String(intent.to_string()));
        } else if matches!(
            normalized.as_str(),
            "modulename" | "module" | "name" | "subject"
        ) {
            if let Some(name) = infer_module_name(intent) {
                input.insert(id.to_string(), Value::String(name));
            }
        } else if matches!(
            normalized.as_str(),
            "moduleroot" | "sourceroot" | "root" | "sourcepath" | "source"
        ) {
            if let Some(path) = primary_path.clone() {
                input.insert(id.to_string(), Value::String(path));
            }
        } else if matches!(
            normalized.as_str(),
            "kboutputdir" | "outputdir" | "outputpath" | "targetdir" | "targetpath"
        ) {
            if let Some(path) = output_path.clone() {
                input.insert(id.to_string(), Value::String(path));
            }
        } else if normalized.contains("extrasource") || normalized.contains("sourcedirs") {
            if !extra_paths.is_empty() {
                input.insert(id.to_string(), Value::Array(extra_paths.clone()));
            }
        } else if normalized == "language" || normalized == "lang" {
            input.insert(id.to_string(), Value::String(infer_language(intent)));
        }
    }

    input
}

fn meaningful_input_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        _ => true,
    }
}

fn normalize_input_id(id: &str) -> String {
    id.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn infer_language(intent: &str) -> String {
    let lower = intent.to_ascii_lowercase();
    if intent.contains("中文") || lower.contains("zh-cn") || lower.contains("chinese") {
        "zh-CN".to_string()
    } else if lower.contains("english") || lower.contains("en-us") {
        "en-US".to_string()
    } else {
        "zh-CN".to_string()
    }
}

fn infer_module_name(intent: &str) -> Option<String> {
    let lower = intent.to_ascii_lowercase();
    if lower.contains("topology mutation") {
        return Some("topology mutation".to_string());
    }
    for marker in ["蒸馏", "构建", "整理", "生成", "编写"] {
        if let Some((_, tail)) = intent.split_once(marker) {
            if let Some(candidate) = module_phrase_from_tail(tail) {
                return Some(candidate);
            }
        }
    }
    for marker in ["模块", "主题", "module", "topic"] {
        if let Some((_, tail)) = lower.split_once(marker) {
            if let Some(candidate) = module_phrase_from_tail(tail) {
                return Some(candidate);
            }
        }
    }
    None
}

fn module_phrase_from_tail(tail: &str) -> Option<String> {
    let mut end = tail.len();
    for (idx, ch) in tail.char_indices() {
        if ch.is_whitespace() && tail[..idx].trim().len() >= 2 {
            end = idx;
            break;
        }
        if matches!(ch, '，' | '。' | '；' | ';' | ',' | '\n' | '\r') {
            end = idx;
            break;
        }
    }
    let candidate = tail[..end]
        .trim()
        .trim_start_matches("一个")
        .trim_start_matches("一份")
        .trim_start_matches("的")
        .trim_end_matches("模块")
        .trim_end_matches("主题")
        .trim();
    if candidate.is_empty()
        || candidate.contains(":\\")
        || candidate.contains("源码")
        || candidate.contains("输出")
    {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn path_after_marker(text: &str, marker: &str) -> Option<String> {
    let (_, tail) = text.split_once(marker)?;
    extract_windows_paths(tail).into_iter().next()
}

fn extract_windows_paths(text: &str) -> Vec<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut paths = Vec::new();
    let mut i = 0;
    while i + 2 < chars.len() {
        let (_, drive) = chars[i];
        let (_, colon) = chars[i + 1];
        let (_, slash) = chars[i + 2];
        if drive.is_ascii_alphabetic() && colon == ':' && (slash == '\\' || slash == '/') {
            let start = chars[i].0;
            let mut end = text.len();
            let mut j = i + 3;
            while j < chars.len() {
                let (idx, ch) = chars[j];
                if is_path_delimiter(ch) {
                    end = idx;
                    break;
                }
                j += 1;
            }
            let path = trim_path_tail(&text[start..end]);
            if !path.is_empty() && !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_string());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    paths
}

fn is_path_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '，' | '。'
                | '；'
                | ';'
                | '、'
                | '"'
                | '\''
                | '`'
                | '?'
                | '？'
                | '<'
                | '>'
                | '|'
                | '\r'
                | '\n'
        )
}

fn trim_path_tail(path: &str) -> &str {
    path.trim_end_matches(|ch| matches!(ch, '.' | ',' | ';' | ':' | ')' | ']' | '}'))
}

pub(super) fn graph_run_policy(graph: &TaskGraphDefinition) -> task_graph::TaskGraphRunPolicy {
    graph
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.run_policy.clone())
        .unwrap_or_default()
}

fn occupies_concurrency_slot(status: task_graph::RunStatus) -> bool {
    matches!(
        status,
        task_graph::RunStatus::Pending
            | task_graph::RunStatus::Running
            | task_graph::RunStatus::Paused
    )
}

fn same_run_admission_key(lhs: &task_graph::GraphRef, rhs: &task_graph::GraphRef) -> bool {
    lhs.scope == rhs.scope && lhs.id == rhs.id
}

fn queue_deadline_at(policy: &task_graph::TaskGraphRunPolicy) -> String {
    (Utc::now() + ChronoDuration::milliseconds(policy.max_queue_wait_ms as i64)).to_rfc3339()
}

fn summarize_run(run: task_graph::TaskGraphRun) -> task_graph::TaskGraphRunSummary {
    task_graph::TaskGraphRunSummary {
        id: run.id,
        project: run.project,
        graph_ref: run.graph_ref,
        status: run.status,
        created_at: run.created_at,
        queued_at: run.queued_at,
        queue_deadline_at: run.queue_deadline_at,
        started_at: run.started_at,
        updated_at: run.updated_at,
        completed_at: run.completed_at,
        current_superstep: run.current_superstep,
        last_checkpoint_id: run.last_checkpoint_id,
        current_graph_revision: run.current_graph_revision,
        active_nodes: run.active_nodes,
        checkpoint_ns: run.checkpoint_ns,
    }
}

pub(crate) fn dispatch_queued_task_graph_runs(
    root: &FsPath,
    project: &str,
) -> Result<usize, TaskGraphApiError> {
    let now = Utc::now();
    let mut dispatched = 0;

    loop {
        let mut queued_runs: Vec<_> = task_graph::list_runs(root, project)?
            .into_iter()
            .filter(|run| run.status == task_graph::RunStatus::Queued)
            .collect();
        if queued_runs.is_empty() {
            break;
        }
        queued_runs.sort_by(|a, b| {
            a.queued_at
                .cmp(&b.queued_at)
                .then_with(|| a.created_at.cmp(&b.created_at))
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut progressed = false;
        for queued in queued_runs {
            if queued_run_expired(&queued, now) {
                expire_queued_run(root, project, &queued, now)?;
                progressed = true;
                continue;
            }

            let graph = read_current_graph_for_run(root, project, &queued)?;
            let policy = graph_run_policy(&graph);
            let active_slots = task_graph::list_runs(root, project)?
                .into_iter()
                .filter(|run| same_run_admission_key(&run.graph_ref, &queued.graph_ref))
                .filter(|run| occupies_concurrency_slot(run.status))
                .count() as u32;
            if active_slots >= policy.effective_max_concurrent_runs() {
                continue;
            }

            match task_graph::update_run_status(
                root,
                project,
                &queued.id,
                task_graph::RunStatus::Pending,
            ) {
                Ok(run) => {
                    task_graph::append_run_event(
                        root,
                        project,
                        &queued.id,
                        0,
                        "run_dequeued",
                        None,
                        "Queued run admitted to graph execution",
                        json!({
                            "graph_scope": queued.graph_ref.scope,
                            "graph_id": queued.graph_ref.id,
                            "active_slot_count": active_slots,
                            "max_concurrent_runs": policy.effective_max_concurrent_runs(),
                            "queued_at": queued.queued_at,
                            "dequeued_at": run.updated_at,
                        }),
                    )?;
                    spawn_task_graph_run(root.to_path_buf(), project.to_string(), queued.id);
                    dispatched += 1;
                    progressed = true;
                }
                Err(TaskGraphError::InvalidRunTransition { .. }) => {}
                Err(err) => return Err(TaskGraphApiError(err)),
            }
        }

        if !progressed {
            break;
        }
    }

    Ok(dispatched)
}

fn queued_run_expired(run: &task_graph::TaskGraphRunSummary, now: DateTime<Utc>) -> bool {
    run.queue_deadline_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|deadline| deadline.with_timezone(&Utc) <= now)
        .unwrap_or(false)
}

fn expire_queued_run(
    root: &FsPath,
    project: &str,
    run: &task_graph::TaskGraphRunSummary,
    now: DateTime<Utc>,
) -> Result<(), TaskGraphApiError> {
    match task_graph::update_run_status(root, project, &run.id, task_graph::RunStatus::Failed) {
        Ok(_) => {
            task_graph::append_run_event(
                root,
                project,
                &run.id,
                0,
                "run_queue_timeout",
                None,
                "Queued run exceeded max queue wait and was failed",
                json!({
                    "graph_scope": run.graph_ref.scope,
                    "graph_id": run.graph_ref.id,
                    "queued_at": run.queued_at,
                    "queue_deadline_at": run.queue_deadline_at,
                    "timed_out_at": now.to_rfc3339(),
                }),
            )?;
            Ok(())
        }
        Err(TaskGraphError::InvalidRunTransition { .. }) => Ok(()),
        Err(err) => Err(TaskGraphApiError(err)),
    }
}

fn read_current_graph_for_run(
    root: &FsPath,
    project: &str,
    run: &task_graph::TaskGraphRunSummary,
) -> Result<TaskGraphDefinition, TaskGraphApiError> {
    let current = match run.graph_ref.scope {
        TaskGraphScope::System => task_graph::read_system_graph(root, &run.graph_ref.id),
        TaskGraphScope::Project => task_graph::read_project_graph(root, project, &run.graph_ref.id),
    };
    match current {
        Ok(graph) => Ok(graph),
        Err(_) => Ok(task_graph::read_run_detail(root, project, &run.id)?.graph_snapshot),
    }
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
        let _ = dispatch_queued_task_graph_runs(&opts.workspace_root, &opts.project);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bb_core::task_graph::{TaskGraphInputParam, TaskGraphScope};

    #[test]
    fn pre_start_intent_resolver_projects_chat_prompt_into_kb_inputs() {
        let graph = TaskGraphDefinition {
            schema_version: 1,
            id: "kb-wiki-build-workflow".to_string(),
            scope: TaskGraphScope::System,
            title: "KB Wiki Build Workflow".to_string(),
            description: None,
            version: 17,
            readonly: true,
            origin: None,
            metadata: None,
            inputs: Some(vec![
                input("request", "string", json!("")),
                input("module-name", "string", json!("")),
                input("module-root", "string", json!("")),
                input("kb-output-dir", "string", json!("")),
                input("extra-source-dirs", "array<string>", json!([])),
                input("language", "string", json!("zh-CN")),
            ]),
            nodes: vec![],
            edges: vec![],
            layout: None,
        };
        let intent = "请蒸馏 topology mutation 模块，源码在 D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph，输出到 D:\\Dev\\blackboard\\.bb_template\\projects\\blackboard\\wiki\\topology，中文";

        let resolved = prepare_pre_start_input(&graph, json!({}), Some(intent));

        assert_eq!(resolved["request"], intent);
        assert_eq!(resolved["module-name"], "topology mutation");
        assert_eq!(
            resolved["module-root"],
            "D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph"
        );
        assert_eq!(
            resolved["kb-output-dir"],
            "D:\\Dev\\blackboard\\.bb_template\\projects\\blackboard\\wiki\\topology"
        );
        assert_eq!(resolved["language"], "zh-CN");
    }

    fn input(id: &str, value_type: &str, default_value: Value) -> TaskGraphInputParam {
        TaskGraphInputParam {
            id: id.to_string(),
            label: None,
            value_type: value_type.to_string(),
            reducer: None,
            channel_class: None,
            default_value,
            description: None,
            min: None,
            max: None,
        }
    }
}
