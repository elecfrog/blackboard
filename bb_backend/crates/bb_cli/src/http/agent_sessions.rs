use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bb_core::agent_session::{self, AgentEvent, AgentSession, AgentSessionError};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct AgentSessionEventsQuery {
    since: Option<u64>,
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
