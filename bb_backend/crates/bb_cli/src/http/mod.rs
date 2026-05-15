mod agent_sessions;
mod project_picker;
pub(crate) mod task_graph;
mod tickets;
mod wiki;

use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use crate::mcp_handler::BbMcpHandler;
use bb_core::{
    agents_config, AgentProfile, AgentRegistryList, ArchiveLaneResult, BoardSummary,
    CreatedInboxNote, InboxError, InboxNote, InboxNoteEntry, InboxNoteInput, LaneDef,
    ProjectAgentList, ProjectAgentRegistration, ProjectBoardViewSettings, ProjectDirectoryCreate,
    ProjectDirectoryOpen, ProjectEntry, RemovedProjectAgentRegistration, Workspace,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct AppState {
    workspace: Arc<RwLock<Workspace>>,
    static_dir: Option<PathBuf>,
    mcp_url: Option<String>,
}

impl AppState {
    pub fn workspace(&self) -> Result<Workspace, InboxError> {
        self.workspace
            .read()
            .map_err(|_| InboxError::InvalidInput("workspace state lock is poisoned".to_string()))
            .map(|workspace| workspace.clone())
    }

    pub fn workspace_root(&self) -> Result<PathBuf, InboxError> {
        Ok(self.workspace()?.root().to_path_buf())
    }
}

#[derive(Debug, Serialize)]
struct NotesResponse {
    notes: Vec<InboxNoteEntry>,
}

#[derive(Debug, Serialize)]
struct ProjectsResponse {
    generated_at: String,
    projects: Vec<ProjectEntry>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

#[derive(Debug, Clone, Default)]
pub struct HttpServeOptions {
    pub static_dir: Option<PathBuf>,
    pub mcp_url: Option<String>,
}

#[allow(dead_code)]
pub fn app(workspace: Workspace) -> Router {
    app_with_static(workspace, None)
}

pub fn app_with_static(workspace: Workspace, static_dir: Option<PathBuf>) -> Router {
    app_with_options(
        workspace,
        HttpServeOptions {
            static_dir,
            mcp_url: None,
        },
    )
}

pub fn app_with_options(workspace: Workspace, options: HttpServeOptions) -> Router {
    let state = AppState {
        workspace: Arc::new(RwLock::new(workspace)),
        static_dir: options.static_dir,
        mcp_url: options.mcp_url,
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/projects", get(list_projects))
        .route(
            "/api/workspace/folder/select",
            post(project_picker::select_workspace_folder),
        )
        .route("/api/workspace/folder/open", post(open_workspace_folder))
        .route("/api/projects/create", post(create_project_directory))
        .route("/api/projects/open", post(open_project_directory))
        .route(
            "/api/projects/select-create-directory",
            post(project_picker::select_create_project_directory),
        )
        .route(
            "/api/projects/select-open-directory",
            post(project_picker::select_open_project_directory),
        )
        .route_service("/mcp", build_mcp_service(state.workspace.clone()))
        .route(
            "/api/projects/{project}/inbox/notes",
            get(list_notes).post(create_note),
        )
        .route("/api/projects/{project}/inbox/notes/{name}", get(read_note))
        .route("/api/projects/{project}/board/summary", get(board_summary))
        .route(
            "/api/projects/{project}/board/view",
            get(project_board_view).patch(patch_project_board_view),
        )
        .route(
            "/api/projects/{project}/tickets",
            get(tickets::list_tickets),
        )
        .route(
            "/api/projects/{project}/tickets/{id}",
            patch(tickets::patch_ticket),
        )
        .route(
            "/api/projects/{project}/tickets/{id}/content",
            get(tickets::ticket_content),
        )
        // Wiki: read-only file tree, Markdown content, and relative static
        // assets for project-level wikis, plus a multipart upload endpoint
        // used by the web UI to drop files/folders into the wiki root.
        //
        // All three read endpoints are explicit literal paths — we avoid
        // `/{*path}` wildcards here so `tree` / `file` / `asset` / `upload`
        // never collide with each other or with user-created wiki paths.
        .route(
            "/api/projects/{project}/wiki/tree",
            get(wiki::wiki_tree_handler),
        )
        .route(
            "/api/projects/{project}/wiki/file",
            get(wiki::wiki_file_handler),
        )
        .route(
            "/api/projects/{project}/wiki/asset",
            get(wiki::wiki_asset_handler),
        )
        .route(
            "/api/projects/{project}/wiki/upload",
            post(wiki::wiki_upload_handler),
        )
        // Lane catalog: part of the runtime HTTP API used by the web app.
        // Blackboard now treats bb as the C/S data source instead of
        // relying on exported JSON artifacts at runtime.
        .route(
            "/api/projects/{project}/lanes",
            get(list_lanes).post(upsert_lane_post),
        )
        .route(
            "/api/projects/{project}/lanes/{id}",
            axum::routing::patch(upsert_lane_patch),
        )
        .route(
            "/api/projects/{project}/lanes/{id}/archive",
            post(archive_lane_handler),
        )
        // Agent connector management. Workspace-scoped (no project arg) and
        // intentionally exposes write methods over HTTP, in the same spirit
        // as the lane endpoints above, so the Settings UI can drive them
        // directly without going through stdio.
        .route("/api/agents/connectors", get(list_agent_connectors_handler))
        .route(
            "/api/agents",
            get(list_agents_handler).post(upsert_agent_handler),
        )
        .route(
            "/api/projects/{project}/agents",
            get(list_project_agents_handler).post(upsert_project_agent_handler),
        )
        .route(
            "/api/projects/{project}/agents/{id}",
            delete(remove_project_agent_handler),
        )
        .route(
            "/api/agents/connectors/{id}/sync",
            post(sync_agent_connector_handler),
        )
        .route(
            "/api/agents/connectors/{id}",
            delete(disconnect_agent_connector_handler),
        )
        // AgentSession: shared LLM execution/session runtime used by Task Graph
        // LLM nodes first, and later by direct project chat.
        .route(
            "/api/projects/{project}/agent-sessions",
            post(agent_sessions::create_agent_session),
        )
        .route(
            "/api/projects/{project}/agent-sessions/{session_id}",
            get(agent_sessions::read_agent_session),
        )
        .route(
            "/api/projects/{project}/agent-sessions/{session_id}/events",
            get(agent_sessions::read_agent_session_events),
        )
        .route(
            "/api/projects/{project}/agent-sessions/{session_id}/stream",
            get(agent_sessions::stream_agent_session_events),
        )
        // Task Graph: catalog, detail, CRUD, fork
        .route(
            "/api/projects/{project}/task-graphs",
            get(task_graph::tg_list_catalog).post(task_graph::tg_create_graph),
        )
        .route(
            "/api/projects/{project}/task-graphs/system/{graph_id}",
            get(task_graph::tg_read_system_graph).patch(task_graph::tg_patch_system_graph),
        )
        .route(
            "/api/projects/{project}/task-graphs/system/{graph_id}/fork",
            post(task_graph::tg_fork_graph),
        )
        .route(
            "/api/projects/{project}/task-graphs/project/{graph_id}",
            get(task_graph::tg_read_project_graph)
                .patch(task_graph::tg_patch_graph)
                .delete(task_graph::tg_delete_graph),
        )
        // Task Graph Schedules: local stateless schedule triggers
        .route(
            "/api/projects/{project}/task-graph-schedules",
            get(task_graph::tg_list_schedules).post(task_graph::tg_create_schedule),
        )
        .route(
            "/api/projects/{project}/task-graph-schedules/{schedule_id}",
            patch(task_graph::tg_patch_schedule).delete(task_graph::tg_delete_schedule),
        )
        .route(
            "/api/projects/{project}/task-graph-schedules/{schedule_id}/run-now",
            post(task_graph::tg_run_schedule_now),
        )
        // Task Graph Runs: create, read, resume gate, cancel
        .route(
            "/api/projects/{project}/task-graph-runs",
            post(task_graph::tg_create_run).get(task_graph::tg_list_runs),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}",
            get(task_graph::tg_read_run),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}/events",
            get(task_graph::tg_run_events),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}/event-log",
            get(task_graph::tg_run_event_log),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}/checkpoints",
            get(task_graph::tg_run_checkpoints),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}/gates/{node_id}/resume",
            post(task_graph::tg_resume_gate),
        )
        .route(
            "/api/projects/{project}/task-graph-runs/{run_id}/cancel",
            post(task_graph::tg_cancel_run),
        )
        // Task Graph: sub-pipeline inputs & prompt file
        .route(
            "/api/projects/{project}/task-graphs/{scope}/{graph_id}/inputs",
            get(task_graph::tg_get_graph_inputs),
        )
        .route(
            "/api/projects/{project}/task-graphs/{scope}/{graph_id}/prompt-file",
            get(task_graph::tg_read_prompt_file).put(task_graph::tg_write_prompt_file),
        )
        .fallback(static_fallback)
        .with_state(state)
}

/// Build the rmcp Streamable HTTP MCP service for the `/mcp` endpoint.
fn build_mcp_service(
    workspace: Arc<RwLock<Workspace>>,
) -> StreamableHttpService<BbMcpHandler, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default();
    let session_manager = Arc::new(LocalSessionManager::default());
    let ws = workspace.clone();
    StreamableHttpService::new(
        move || Ok(BbMcpHandler::new(ws.clone())),
        session_manager,
        config,
    )
}

pub async fn serve(
    workspace: Workspace,
    addr: SocketAddr,
    options: HttpServeOptions,
) -> anyhow::Result<()> {
    let static_dir = match options.static_dir {
        Some(path) => {
            let path = bb_core::fs_util::canonicalize(&path)?;
            anyhow::ensure!(
                path.is_dir(),
                "static directory does not exist: {}",
                bb_core::path_to_string(&path)
            );
            Some(path)
        }
        None => None,
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("bb REST listening on {}", listener.local_addr()?);
    if let Some(static_dir) = &static_dir {
        eprintln!(
            "bb static frontend serving from {}",
            bb_core::path_to_string(static_dir)
        );
    }
    task_graph::spawn_schedule_dispatcher(workspace.clone());
    let router = app_with_options(
        workspace,
        HttpServeOptions {
            static_dir,
            mcp_url: options.mcp_url,
        },
    );
    axum::serve(listener, router).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok\n"
}

async fn static_fallback(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path();
    if path == "/api" || path.starts_with("/api/") || path == "/mcp" || path.starts_with("/mcp/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some(static_dir) = &state.static_dir else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let Some(candidate) = static_path_for_request(static_dir, path) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let file_path = if candidate.is_file() {
        candidate
    } else {
        static_dir.join("index.html")
    };

    if !file_path.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }

    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let mut response = Body::from(bytes).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_for_path(&file_path)),
            );
            response
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn static_path_for_request(static_dir: &FsPath, request_path: &str) -> Option<PathBuf> {
    let trimmed = request_path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Some(static_dir.join("index.html"));
    }

    let mut path = static_dir.to_path_buf();
    for component in FsPath::new(trimmed).components() {
        match component {
            Component::Normal(segment) => path.push(segment),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(path)
}

fn content_type_for_path(path: &FsPath) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Serialize)]
pub(super) struct WorkspaceFolderInspection {
    root: String,
    status: &'static str,
    message: Option<String>,
    suggested_project_name: String,
    suggested_display_name: String,
    active_project: Option<String>,
    projects: Vec<ProjectEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceFolderOpenInput {
    root: String,
    initialize: bool,
    #[serde(default, rename = "project_name")]
    _project_name: Option<String>,
    #[serde(default, rename = "display_name")]
    _display_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkspaceFolderOpenResponse {
    root: String,
    active_project: Option<String>,
    projects: Vec<ProjectEntry>,
}

pub(super) fn inspect_workspace_folder_root(
    state: &AppState,
    root: PathBuf,
) -> Result<WorkspaceFolderInspection, InboxError> {
    let root = bb_core::canonicalize_existing_dir(&root).map_err(|source| InboxError::Io {
        path: root.clone(),
        source,
    })?;
    let suggested_display_name = directory_display_name(&root);
    let suggested_project_name = suggest_project_name(&suggested_display_name);
    let capsule_root = root.join(".bb");

    if !capsule_root.exists() {
        return Ok(WorkspaceFolderInspection {
            root: bb_core::path_to_string(&root),
            status: "uninitialized",
            message: None,
            suggested_project_name,
            suggested_display_name,
            active_project: None,
            projects: Vec::new(),
        });
    }

    if let Some(project) = Workspace::migrate_legacy_folder_workspace_to_capsule(&root)? {
        return Ok(WorkspaceFolderInspection {
            root: bb_core::path_to_string(&root),
            status: "valid",
            message: None,
            suggested_project_name,
            suggested_display_name,
            active_project: Some(project.name.clone()),
            projects: vec![project],
        });
    }

    let global_workspace = state.workspace()?;
    let data_root = bb_core::path_to_string(&capsule_root);
    match global_workspace.inspect_project_directory(&data_root) {
        Ok(mut meta) => {
            meta.data_root = Some(data_root.clone());
            let active_project = global_workspace
                .registered_project_for_data_root(&data_root)?
                .unwrap_or_else(|| suggest_project_name(&meta.name));
            Ok(WorkspaceFolderInspection {
                root: bb_core::path_to_string(&root),
                status: "valid",
                message: None,
                suggested_project_name,
                suggested_display_name,
                active_project: Some(active_project.clone()),
                projects: vec![ProjectEntry {
                    name: active_project,
                    uuid: String::new(),
                    meta,
                }],
            })
        }
        Err(err) => Ok(WorkspaceFolderInspection {
            root: bb_core::path_to_string(&root),
            status: "invalid",
            message: Some(err.to_string()),
            suggested_project_name,
            suggested_display_name,
            active_project: None,
            projects: Vec::new(),
        }),
    }
}

async fn open_workspace_folder(
    State(state): State<AppState>,
    input: Result<Json<WorkspaceFolderOpenInput>, JsonRejection>,
) -> Result<Json<WorkspaceFolderOpenResponse>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid open workspace folder body: {err}"
        )))
    })?;
    let root = PathBuf::from(input.root.trim());
    let root = bb_core::canonicalize_existing_dir(&root).map_err(|source| {
        ApiError(InboxError::Io {
            path: root.clone(),
            source,
        })
    })?;

    let global_workspace = state.workspace()?;
    let capsule_root = root.join(".bb");
    let display_name = directory_display_name(&root);
    let folder_project_name = suggest_project_name(&display_name);
    let project_name = if capsule_root.exists() {
        if Workspace::migrate_legacy_folder_workspace_to_capsule(&root)?.is_none() {
            let data_root = bb_core::path_to_string(&capsule_root);
            global_workspace
                .inspect_project_directory(&data_root)
                .map_err(|err| {
                    ApiError(InboxError::InvalidInput(format!(
                        "invalid Blackboard project at {}: {err}",
                        capsule_root.display()
                    )))
                })?;
        }
        let data_root = bb_core::path_to_string(&capsule_root);
        global_workspace
            .registered_project_for_data_root(&data_root)?
            .unwrap_or_else(|| folder_project_name.clone())
    } else if input.initialize {
        let project_name = folder_project_name.clone();
        Workspace::init_project_capsule(&root, &project_name, Some(&display_name))?;
        project_name
    } else {
        return Err(ApiError(InboxError::InvalidInput(format!(
            "folder is not initialized as a Blackboard project: {}",
            root.display()
        ))));
    };

    let entry = global_workspace.open_project_directory(ProjectDirectoryOpen {
        name: project_name,
        data_root: bb_core::path_to_string(&capsule_root),
    })?;
    let active_project = Some(entry.name);
    let projects = global_workspace.list_projects()?;
    Ok(Json(WorkspaceFolderOpenResponse {
        root: bb_core::path_to_string(&root),
        active_project,
        projects,
    }))
}

fn directory_display_name(root: &FsPath) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Project")
        .to_string()
}

fn suggest_project_name(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    }
}

async fn list_projects(State(state): State<AppState>) -> Result<Json<ProjectsResponse>, ApiError> {
    let projects = state.workspace()?.list_projects()?;
    Ok(Json(ProjectsResponse {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        projects,
    }))
}

async fn create_project_directory(
    State(state): State<AppState>,
    input: Result<Json<ProjectDirectoryCreate>, JsonRejection>,
) -> Result<Json<ProjectEntry>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid create project body: {err}"
        )))
    })?;
    Ok(Json(state.workspace()?.create_project_directory(input)?))
}

async fn open_project_directory(
    State(state): State<AppState>,
    input: Result<Json<ProjectDirectoryOpen>, JsonRejection>,
) -> Result<Json<ProjectEntry>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid open project body: {err}"
        )))
    })?;
    Ok(Json(state.workspace()?.open_project_directory(input)?))
}

async fn list_notes(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<NotesResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    let notes = board.list_notes()?;
    Ok(Json(NotesResponse { notes }))
}

async fn read_note(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
) -> Result<Json<InboxNote>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.read_note(&name)?))
}

async fn create_note(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(mut input): Json<InboxNoteInput>,
) -> Result<(StatusCode, Json<CreatedInboxNote>), ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    // Path project wins over any `project` field inside the body so the
    // resulting note is consistent with the directory it lands in.
    input.project = Some(project);
    Ok((StatusCode::CREATED, Json(board.create_note(input)?)))
}

/// Read-only board summary endpoint.
///
/// This is the ONLY ticket-adjacent capability exposed via HTTP. Ticket
/// read/write operations remain stdio-MCP only (see blackboard/README.md and
/// bb/README.md). This endpoint intentionally returns the same
/// `BoardSummary` produced by the stdio `board_summary` tool so the web
/// BoardView can consume a single aggregated source of truth instead of
/// re-aggregating tickets client-side.
async fn board_summary(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<BoardSummary>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.board_summary()?))
}

async fn project_board_view(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<ProjectBoardViewSettings>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.read_project_meta()?.board_view))
}

async fn patch_project_board_view(
    State(state): State<AppState>,
    Path(project): Path<String>,
    input: Result<Json<ProjectBoardViewSettings>, JsonRejection>,
) -> Result<Json<ProjectBoardViewSettings>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid board view settings body: {err}"
        )))
    })?;
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.update_board_view(input)?))
}

#[derive(Debug, Serialize)]
struct LanesResponse {
    lanes: Vec<LaneDef>,
}

/// Runtime lane catalog. Mirrors the stdio `list_lanes` tool.
async fn list_lanes(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<LanesResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(LanesResponse {
        lanes: board.list_lanes()?,
    }))
}

/// Create or replace a lane via the collection endpoint. Body is a full
/// `LaneDef`. The web app uses this HTTP write path for normal C/S lane
/// management.
async fn upsert_lane_post(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(def): Json<LaneDef>,
) -> Result<(StatusCode, Json<LaneDef>), ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    let saved = board.upsert_lane(def)?;
    Ok((StatusCode::CREATED, Json(saved)))
}

/// Body for the `PATCH /api/projects/{project}/lanes/{id}` endpoint. All
/// fields are optional except `id`, which is taken from the URL.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PatchLaneInput {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// Patch an existing lane. Loads the current entry from `__project__.json` so
/// callers can omit fields they do not want to change. URL `id` always wins.
async fn upsert_lane_patch(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Json(patch): Json<PatchLaneInput>,
) -> Result<Json<LaneDef>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    let mut existing = board
        .list_lanes()?
        .into_iter()
        .find(|lane| lane.id == id)
        .ok_or_else(|| {
            ApiError(InboxError::InvalidInput(format!(
                "lane `{id}` is not defined on this project"
            )))
        })?;
    if let Some(label) = patch.label {
        existing.label = label;
    }
    if let Some(color) = patch.color {
        existing.color = color;
    }
    if let Some(description) = patch.description {
        existing.description = description;
    }
    if let Some(status) = patch.status {
        existing.status = status;
    }
    Ok(Json(board.upsert_lane(existing)?))
}

/// Archive a lane. Returns the updated definition plus the count of tickets
/// that still cite it so the UI can warn about orphans.
async fn archive_lane_handler(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<ArchiveLaneResult>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.archive_lane(&id)?))
}

/// List all supported Agent connectors and their per-target sync state.
async fn list_agent_connectors_handler(
    State(state): State<AppState>,
) -> Result<Json<agents_config::AgentConnectorList>, ApiError> {
    Ok(Json(agents_config::list_agent_connectors_with_mcp_url(
        &state.workspace_root()?,
        state.mcp_url.as_deref(),
    )?))
}

async fn list_agents_handler(
    State(state): State<AppState>,
) -> Result<Json<AgentRegistryList>, ApiError> {
    Ok(Json(state.workspace()?.list_agents()?))
}

async fn list_project_agents_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<ProjectAgentList>, ApiError> {
    Ok(Json(state.workspace()?.list_project_agents(&project)?))
}

async fn upsert_agent_handler(
    State(state): State<AppState>,
    input: Result<Json<AgentProfile>, JsonRejection>,
) -> Result<Json<AgentProfile>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid agent body: {err}"
        )))
    })?;
    let workspace_root = state.workspace_root()?;
    Ok(Json(bb_core::agents_registry::upsert_agent(
        &workspace_root,
        input,
    )?))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectAgentWriteInput {
    agent: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    lanes: Vec<String>,
}

async fn upsert_project_agent_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
    input: Result<Json<ProjectAgentWriteInput>, JsonRejection>,
) -> Result<Json<ProjectAgentRegistration>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid project agent body: {err}"
        )))
    })?;
    let workspace_root = state.workspace_root()?;
    Ok(Json(bb_core::agents_registry::upsert_project_agent(
        &workspace_root,
        ProjectAgentRegistration {
            project,
            agent: input.agent,
            role: input.role,
            lanes: input.lanes,
        },
    )?))
}

async fn remove_project_agent_handler(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<RemovedProjectAgentRegistration>, ApiError> {
    let workspace_root = state.workspace_root()?;
    Ok(Json(bb_core::agents_registry::remove_project_agent(
        &workspace_root,
        &project,
        &id,
    )?))
}

/// Sync the canonical AGENTS.md to a single connector target. Returns the
/// updated connector entry.
async fn sync_agent_connector_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<agents_config::AgentConnector>, ApiError> {
    let workspace_root = state.workspace_root()?;
    Ok(Json(agents_config::sync_agent_connector_with_mcp_url(
        &workspace_root,
        &id,
        state.mcp_url.as_deref(),
    )?))
}

/// Delete a connector's target file. Returns the updated connector entry
/// (state will typically flip back to `missing` or `unreachable`).
async fn disconnect_agent_connector_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<agents_config::AgentConnector>, ApiError> {
    let workspace_root = state.workspace_root()?;
    Ok(Json(agents_config::disconnect_agent_connector(
        &workspace_root,
        &id,
    )?))
}

#[derive(Debug)]
struct ApiError(InboxError);

impl From<InboxError> for ApiError {
    fn from(value: InboxError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            InboxError::InvalidName(_) | InboxError::InvalidInput(_) => {
                (StatusCode::BAD_REQUEST, "bad_request")
            }
            InboxError::InvalidTicketStatus(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            InboxError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            InboxError::TicketNotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
            InboxError::InvalidTicketId(_) | InboxError::DuplicateTicketId { .. } => {
                (StatusCode::BAD_REQUEST, "bad_request")
            }
            InboxError::InvalidTicketFamily(_) | InboxError::TicketWriteConflict(_) => {
                (StatusCode::BAD_REQUEST, "bad_request")
            }
            InboxError::TicketIdNotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            InboxError::InvalidProjectName(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            InboxError::ProjectNotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            InboxError::InvalidProjectMeta { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
            InboxError::RootNotFound(_)
            | InboxError::MissingInbox(_)
            | InboxError::ProjectsRootMissing(_)
            | InboxError::TicketIdLockTimeout(_)
            | InboxError::Io { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = ErrorResponse {
            error: ErrorDetail {
                code,
                message: self.0.to_string(),
            },
        };

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests;
