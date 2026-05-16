use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bb_core::agent_session::{
    self, AgentEvent, AgentEventType, AgentSession, AgentSessionError, AgentSessionStatus,
    AgentTurnRequest, CreateAgentSession,
};
use bb_core::task_graph::resolve_scripts_dir;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use std::time::Duration;

use super::task_graph::runner_config::{
    build_opencode_task_graph_config, resolve_overrides_from_profile, RunnerOverrides,
};
use super::AppState;

const DIRECT_CHAT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Deserialize)]
pub(crate) struct AgentSessionEventsQuery {
    since: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateAgentSessionBody {
    prompt: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    runtime: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    provider_session_id: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentSessionResponse {
    session: AgentSession,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentSessionEventsResponse {
    events: Vec<AgentEvent>,
}

#[derive(Debug)]
pub(crate) struct AgentSessionApiError(AgentSessionError);

impl From<AgentSessionError> for AgentSessionApiError {
    fn from(err: AgentSessionError) -> Self {
        Self(err)
    }
}

#[derive(Debug, Serialize)]
struct AgentSessionErrorResponse {
    error: AgentSessionErrorDetail,
}

#[derive(Debug, Serialize)]
struct AgentSessionErrorDetail {
    code: &'static str,
    message: String,
}

impl IntoResponse for AgentSessionApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            AgentSessionError::NotFound { .. } => {
                (StatusCode::NOT_FOUND, "agent_session_not_found")
            }
            AgentSessionError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_input"),
            AgentSessionError::Io { .. }
            | AgentSessionError::IoMessage { .. }
            | AgentSessionError::Parse { .. }
            | AgentSessionError::Spawn { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };
        (
            status,
            Json(AgentSessionErrorResponse {
                error: AgentSessionErrorDetail {
                    code,
                    message: self.0.to_string(),
                },
            }),
        )
            .into_response()
    }
}

pub(crate) async fn create_agent_session(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(body): Json<CreateAgentSessionBody>,
) -> Result<(StatusCode, Json<AgentSessionResponse>), AgentSessionApiError> {
    let prompt = body.prompt.trim();
    if prompt.is_empty() {
        return Err(AgentSessionApiError(AgentSessionError::InvalidInput(
            "prompt cannot be empty".to_string(),
        )));
    }

    let root = state.workspace_root().map_err(|err| {
        AgentSessionApiError(AgentSessionError::InvalidInput(format!(
            "workspace unavailable: {err}"
        )))
    })?;
    // Validate the project through the workspace registry before writing runtime state.
    let workspace = state.workspace().map_err(|err| {
        AgentSessionApiError(AgentSessionError::InvalidInput(format!(
            "workspace unavailable: {err}"
        )))
    })?;
    let board = workspace
        .open_project(&project)
        .map_err(|err| AgentSessionApiError(AgentSessionError::InvalidInput(err.to_string())))?;
    let project_meta = board
        .read_project_meta()
        .map_err(|err| AgentSessionApiError(AgentSessionError::InvalidInput(err.to_string())))?;
    let execution_root = resolve_direct_chat_execution_root(&root, board.root(), &project_meta);

    let agent = clean_optional(body.agent).unwrap_or_else(|| "opencode".to_string());
    let runtime = clean_optional(body.runtime)
        .or_else(|| registered_runtime_for_agent(&root, &agent))
        .unwrap_or_else(|| "opencode".to_string());
    if runtime != "opencode" {
        return Err(AgentSessionApiError(AgentSessionError::InvalidInput(
            format!("direct project chat currently supports opencode runtime, got {runtime}"),
        )));
    }

    let cli_overrides = RunnerOverrides {
        agent: Some(agent.clone()),
        model: clean_optional(body.model),
        variant: clean_optional(body.variant),
        ..Default::default()
    };
    let overrides = resolve_overrides_from_profile(&root, &cli_overrides);
    bb_core::skills::inject_skills_for_runtime(&root, &runtime, &overrides.skills, &root).map_err(
        |source| {
            AgentSessionApiError(AgentSessionError::Io {
                path: root.join("skills"),
                source,
            })
        },
    )?;

    let model = overrides.model.clone();
    let variant = overrides.variant.clone();
    let session = agent_session::create_session(
        &root,
        CreateAgentSession {
            project: project.clone(),
            title: clean_optional(body.title).or_else(|| Some(chat_title(prompt))),
            runtime: runtime.clone(),
            agent: agent.clone(),
            model: model.clone(),
            variant: variant.clone(),
            parent: None,
        },
    )?;

    let session_id = session.id.clone();
    let opencode_config_content =
        build_opencode_task_graph_config(&root, &agent, &overrides.mcp_servers);
    let request = AgentTurnRequest {
        workspace_root: root.clone(),
        scripts_dir: resolve_scripts_dir(&root),
        execution_root: execution_root.clone(),
        project: project.clone(),
        session_id: session_id.clone(),
        runtime,
        agent,
        model,
        variant,
        continue_provider_session_id: clean_optional(body.provider_session_id),
        prompt: build_direct_chat_prompt(&project, &execution_root, prompt),
        codex_path: "codex".to_string(),
        codex_config_args: Vec::new(),
        codebuddy_path: "codebuddy".to_string(),
        codebuddy_mcp_config_content: None,
        codebuddy_settings_json: None,
        opencode_path: "opencode".to_string(),
        opencode_config_content,
        custom_env: overrides.custom_env.clone(),
        custom_args: overrides.custom_args.clone(),
        timeout: Duration::from_secs(body.timeout_secs.unwrap_or(DIRECT_CHAT_TIMEOUT_SECS)),
    };

    tokio::task::spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            agent_session::run_turn(request, || false)
        }));
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                let _ = mark_agent_session_failed(&root, &project, &session_id, err.to_string());
            }
            Err(panic_info) => {
                let message = if let Some(value) = panic_info.downcast_ref::<&str>() {
                    value.to_string()
                } else if let Some(value) = panic_info.downcast_ref::<String>() {
                    value.clone()
                } else {
                    "unknown panic".to_string()
                };
                let _ = mark_agent_session_failed(
                    &root,
                    &project,
                    &session_id,
                    format!("agent session panicked: {message}"),
                );
            }
        }
    });

    Ok((StatusCode::CREATED, Json(AgentSessionResponse { session })))
}

pub(crate) async fn read_agent_session(
    State(state): State<AppState>,
    Path((project, session_id)): Path<(String, String)>,
) -> Result<Json<AgentSessionResponse>, AgentSessionApiError> {
    let root = state.workspace_root().map_err(|err| {
        AgentSessionApiError(AgentSessionError::InvalidInput(format!(
            "workspace unavailable: {err}"
        )))
    })?;
    let session = agent_session::read_session(&root, &project, &session_id)?;
    Ok(Json(AgentSessionResponse { session }))
}

pub(crate) async fn read_agent_session_events(
    State(state): State<AppState>,
    Path((project, session_id)): Path<(String, String)>,
    Query(query): Query<AgentSessionEventsQuery>,
) -> Result<Json<AgentSessionEventsResponse>, AgentSessionApiError> {
    let root = state.workspace_root().map_err(|err| {
        AgentSessionApiError(AgentSessionError::InvalidInput(format!(
            "workspace unavailable: {err}"
        )))
    })?;
    agent_session::read_session(&root, &project, &session_id)?;
    let events = agent_session::read_events(&root, &project, &session_id, query.since)?;
    Ok(Json(AgentSessionEventsResponse { events }))
}

pub(crate) async fn stream_agent_session_events(
    State(state): State<AppState>,
    Path((project, session_id)): Path<(String, String)>,
    Query(query): Query<AgentSessionEventsQuery>,
) -> Result<Response, AgentSessionApiError> {
    let root = state.workspace_root().map_err(|err| {
        AgentSessionApiError(AgentSessionError::InvalidInput(format!(
            "workspace unavailable: {err}"
        )))
    })?;
    agent_session::read_session(&root, &project, &session_id)?;
    let session_dir = agent_session::session_dir(&root, &project, &session_id);

    let stream = async_stream::stream! {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("agent_session_error").data(agent_session_error_payload(err.to_string())),
                );
                return;
            }
        };
        if let Err(err) = watcher.watch(&session_dir, RecursiveMode::Recursive) {
            yield Ok::<Event, std::convert::Infallible>(
                Event::default().event("agent_session_error").data(agent_session_error_payload(err.to_string())),
            );
            return;
        }

        let mut last_seq = query.since.unwrap_or(0);
        match agent_session::read_events(&root, &project, &session_id, Some(last_seq)) {
            Ok(events) => {
                if !events.is_empty() {
                    if let Some(last) = events.last() {
                        last_seq = last.seq;
                    }
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("agent_session_events").data(agent_session_events_payload(events)),
                    );
                }
            }
            Err(err) => {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("agent_session_error").data(agent_session_error_payload(err.to_string())),
                );
                return;
            }
        }

        while let Some(result) = rx.recv().await {
            if let Err(err) = result {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("agent_session_error").data(agent_session_error_payload(err.to_string())),
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            match agent_session::read_events(&root, &project, &session_id, Some(last_seq)) {
                Ok(events) => {
                    if events.is_empty() {
                        continue;
                    }
                    if let Some(last) = events.last() {
                        last_seq = last.seq;
                    }
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("agent_session_events").data(agent_session_events_payload(events)),
                    );
                }
                Err(err) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("agent_session_error").data(agent_session_error_payload(err.to_string())),
                    );
                    return;
                }
            }
        }
    };

    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response())
}

fn agent_session_events_payload(events: Vec<AgentEvent>) -> String {
    serde_json::json!({ "events": events }).to_string()
}

fn agent_session_error_payload(message: String) -> String {
    serde_json::json!({ "error": { "message": message } }).to_string()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn registered_runtime_for_agent(root: &std::path::Path, agent: &str) -> Option<String> {
    bb_core::agents_registry::list_agents(root)
        .ok()?
        .agents
        .into_iter()
        .find(|profile| profile.id == agent)
        .and_then(|profile| profile.runtime)
}

fn chat_title(prompt: &str) -> String {
    let mut title = prompt
        .split_whitespace()
        .take(10)
        .collect::<Vec<_>>()
        .join(" ");
    if title.chars().count() > 80 {
        title = title.chars().take(80).collect();
    }
    if title.is_empty() {
        "Project chat".to_string()
    } else {
        title
    }
}

fn build_direct_chat_prompt(project: &str, execution_root: &FsPath, prompt: &str) -> String {
    format!(
        "You are OpenCode running from Blackboard's direct project chat.\nProject: {project}\nWorking directory: {}\nAnswer the user directly, and when you use tools, keep the final answer concise.\n\nUser request:\n{prompt}",
        execution_root.display()
    )
}

fn resolve_direct_chat_execution_root(
    workspace_root: &FsPath,
    project_data_root: &FsPath,
    meta: &bb_core::ProjectMeta,
) -> PathBuf {
    for repo in &meta.repos {
        let repo = repo.trim();
        if repo.is_empty() {
            continue;
        }
        let candidate = FsPath::new(repo);
        let candidate = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            project_data_root.join(candidate)
        };
        if let Ok(root) = bb_core::canonicalize_existing_dir(&candidate) {
            return root;
        }
    }

    if project_data_root.file_name().and_then(|name| name.to_str()) == Some(".bb") {
        if let Some(parent) = project_data_root.parent() {
            if let Ok(root) = bb_core::canonicalize_existing_dir(parent) {
                return root;
            }
        }
    }

    if workspace_root.file_name().and_then(|name| name.to_str()) == Some(".bb_template") {
        if let Some(parent) = workspace_root.parent() {
            if let Ok(root) = bb_core::canonicalize_existing_dir(parent) {
                return root;
            }
        }
    }

    bb_core::canonicalize_existing_dir(project_data_root)
        .unwrap_or_else(|_| project_data_root.to_path_buf())
}

fn mark_agent_session_failed(
    root: &std::path::Path,
    project: &str,
    session_id: &str,
    message: String,
) -> Result<(), AgentSessionError> {
    let next_seq = agent_session::read_events(root, project, session_id, None)?
        .last()
        .map(|event| event.seq + 1)
        .unwrap_or(1);
    let event = AgentEvent {
        seq: next_seq,
        timestamp: Utc::now().to_rfc3339(),
        event_type: AgentEventType::Error,
        content: Some(message),
        tool: None,
        call_id: None,
        input: None,
        output: None,
        status: None,
        level: None,
        session_id: None,
        usage: BTreeMap::new(),
    };
    agent_session::append_event(root, project, session_id, &event)?;
    let completed_at = Utc::now().to_rfc3339();
    agent_session::update_session(root, project, session_id, |session| {
        session.status = AgentSessionStatus::Failed;
        session.completed_at = Some(completed_at);
        session.event_count = next_seq;
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_meta(repos: Vec<String>) -> bb_core::ProjectMeta {
        bb_core::ProjectMeta {
            name: "Demo".to_string(),
            kind: "tool".to_string(),
            repos,
            description: None,
            data_root: None,
            local_host: None,
            lanes: Vec::new(),
            board_view: bb_core::ProjectBoardViewSettings::default(),
        }
    }

    #[test]
    fn direct_chat_execution_root_prefers_project_repo() {
        let temp = tempfile::tempdir().unwrap();
        let workspace_root = temp.path().join(".bb_template");
        let project_data_root = workspace_root.join("projects/devkit");
        let repo_root = temp.path().join("Devkit");
        std::fs::create_dir_all(&project_data_root).unwrap();
        std::fs::create_dir_all(&repo_root).unwrap();

        let root = resolve_direct_chat_execution_root(
            &workspace_root,
            &project_data_root,
            &project_meta(vec![repo_root.to_string_lossy().to_string()]),
        );

        assert_eq!(
            root,
            bb_core::canonicalize_existing_dir(&repo_root).unwrap()
        );
    }

    #[test]
    fn direct_chat_execution_root_falls_back_to_capsule_parent() {
        let temp = tempfile::tempdir().unwrap();
        let workspace_root = temp.path().join(".bb");
        let repo_root = temp.path().join("Code Root");
        let project_data_root = repo_root.join(".bb");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&project_data_root).unwrap();

        let root = resolve_direct_chat_execution_root(
            &workspace_root,
            &project_data_root,
            &project_meta(Vec::new()),
        );

        assert_eq!(
            root,
            bb_core::canonicalize_existing_dir(&repo_root).unwrap()
        );
    }

    #[test]
    fn direct_chat_execution_root_falls_back_to_template_parent() {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("blackboard");
        let workspace_root = repo_root.join(".bb_template");
        let project_data_root = workspace_root.join("projects/blackboard");
        std::fs::create_dir_all(&project_data_root).unwrap();

        let root = resolve_direct_chat_execution_root(
            &workspace_root,
            &project_data_root,
            &project_meta(Vec::new()),
        );

        assert_eq!(
            root,
            bb_core::canonicalize_existing_dir(&repo_root).unwrap()
        );
    }
}
