use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use bb_core::task_graph::{
    self, TaskGraphDefinition, TaskGraphError, TaskGraphSummary, TaskGraphValidationError,
};
use serde::{Deserialize, Serialize};

// ─── Task Graph Error ────────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct TaskGraphApiError(pub(crate) TaskGraphError);

impl From<TaskGraphError> for TaskGraphApiError {
    fn from(e: TaskGraphError) -> Self {
        Self(e)
    }
}

#[derive(Debug, Serialize)]
struct TgErrorResponse {
    error: TgErrorDetail,
}

#[derive(Debug, Serialize)]
struct TgErrorDetail {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<TaskGraphValidationError>>,
}

impl IntoResponse for TaskGraphApiError {
    fn into_response(self) -> Response {
        let (status, code, details) = match &self.0 {
            TaskGraphError::NotFound { .. } => (StatusCode::NOT_FOUND, "graph_not_found", None),
            TaskGraphError::RunNotFound { .. } => (StatusCode::NOT_FOUND, "run_not_found", None),
            TaskGraphError::ReadonlyGraph(_) => (StatusCode::FORBIDDEN, "readonly_graph", None),
            TaskGraphError::DuplicateGraphId(_) => {
                (StatusCode::CONFLICT, "duplicate_graph_id", None)
            }
            TaskGraphError::StaleVersion { .. } => (StatusCode::CONFLICT, "stale_version", None),
            TaskGraphError::ValidationFailed { errors, .. } => (
                StatusCode::BAD_REQUEST,
                "validation_failed",
                Some(errors.clone()),
            ),
            TaskGraphError::InvalidGraphId(_) => {
                (StatusCode::BAD_REQUEST, "validation_failed", None)
            }
            TaskGraphError::InvalidRunTransition { .. } => {
                (StatusCode::CONFLICT, "run_not_resumable", None)
            }
            TaskGraphError::Io { .. } | TaskGraphError::Parse { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", None)
            }
        };

        let body = TgErrorResponse {
            error: TgErrorDetail {
                code,
                message: self.0.to_string(),
                details,
            },
        };

        (status, Json(body)).into_response()
    }
}

// ─── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct TgCatalogResponse {
    pub(crate) graphs: Vec<TaskGraphSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgGraphResponse {
    pub(crate) graph: TaskGraphDefinition,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgWriteResponse {
    pub(crate) graph: TaskGraphDefinition,
    pub(crate) validation: TgValidationStatus,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgValidationStatus {
    pub(crate) status: &'static str,
    pub(crate) errors: Vec<TaskGraphValidationError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgCreateBody {
    pub(crate) graph: TaskGraphDefinition,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgPatchBody {
    pub(crate) graph: TaskGraphDefinition,
    pub(crate) expected_version: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgForkBody {
    pub(crate) target_id: String,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

// ─── Run DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct TgCreateRunBody {
    pub(crate) graph: TgGraphRef,
    #[serde(default)]
    pub(crate) input: serde_json::Value,
    #[serde(default)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgGraphRef {
    pub(crate) scope: String,
    pub(crate) id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgResumeGateBody {
    pub(crate) action: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgRunResponse {
    pub(crate) run: task_graph::TaskGraphRunSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgRunDetailResponse {
    pub(crate) run: task_graph::TaskGraphRunDetail,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgRunsListResponse {
    pub(crate) runs: Vec<task_graph::TaskGraphRunSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgRunStatusResponse {
    pub(crate) run_id: String,
    pub(crate) status: String,
}

pub(crate) fn is_active_run_status(status: task_graph::RunStatus) -> bool {
    matches!(
        status,
        task_graph::RunStatus::Pending
            | task_graph::RunStatus::Running
            | task_graph::RunStatus::Paused
    )
}
