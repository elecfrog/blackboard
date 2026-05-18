use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use bb_core::task_graph::{self, TaskGraphError};
use serde::Deserialize;
use std::collections::HashMap;

use super::super::AppState;
use super::current_workspace_root;
use super::dto::TaskGraphApiError;

// ─── Sub-Pipeline: Get graph inputs ──────────────────────────────────────────

/// GET /api/projects/:project/task-graphs/:scope/:graph_id/inputs
/// Returns the inputs definition of a specific graph (for sub-pipeline parameter binding).
pub async fn tg_get_graph_inputs(
    State(state): State<AppState>,
    Path((project, scope, graph_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;

    let graph = match scope.as_str() {
        "system" => task_graph::read_system_graph(&root, &graph_id)?,
        "project" => task_graph::read_project_graph(&root, &project, &graph_id)?,
        _ => {
            return Err(TaskGraphApiError(TaskGraphError::NotFound {
                scope: scope.clone(),
                id: graph_id,
            }));
        }
    };

    let inputs = graph.inputs.unwrap_or_default();
    Ok(Json(serde_json::json!({
        "graph_id": graph_id,
        "scope": scope,
        "inputs": inputs,
    })))
}

/// GET /api/projects/:project/task-graphs/:scope/:graph_id/prompt-file
/// Read a prompt file content (for LLM node file mode editing).
pub async fn tg_read_prompt_file(
    State(state): State<AppState>,
    Path((_project, _scope, _graph_id)): Path<(String, String, String)>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;

    let file_path = params.get("path").cloned().unwrap_or_default();
    if file_path.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing 'path' query parameter"})),
        )
            .into_response());
    }

    let full_path = root.join(&file_path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => Ok(Json(serde_json::json!({
            "path": file_path,
            "content": content,
        }))
        .into_response()),
        Err(e) => Ok((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Failed to read file: {}", e)})),
        )
            .into_response()),
    }
}

/// PUT /api/projects/:project/task-graphs/:scope/:graph_id/prompt-file
/// Write prompt file content back (for LLM node file mode editing).
pub async fn tg_write_prompt_file(
    State(state): State<AppState>,
    Path((_project, _scope, _graph_id)): Path<(String, String, String)>,
    Json(body): Json<WritePromptFileBody>,
) -> Result<impl IntoResponse, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;

    if body.path.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing 'path' field"})),
        )
            .into_response());
    }

    let full_path = root.join(&body.path);

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(&full_path, &body.content) {
        Ok(()) => Ok(Json(serde_json::json!({
            "path": body.path,
            "written": true,
        }))
        .into_response()),
        Err(e) => Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write file: {}", e)})),
        )
            .into_response()),
    }
}

#[derive(Debug, Deserialize)]
pub struct WritePromptFileBody {
    pub path: String,
    pub content: String,
}
