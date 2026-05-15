use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use bb_core::task_graph::{
    self, TaskGraphDefinition, TaskGraphError, TaskGraphSummary, TaskGraphValidationError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
            TaskGraphError::ScheduleNotFound { .. } => {
                (StatusCode::NOT_FOUND, "schedule_not_found", None)
            }
            TaskGraphError::ReadonlyGraph(_) => (StatusCode::FORBIDDEN, "readonly_graph", None),
            TaskGraphError::DuplicateGraphId(_) => {
                (StatusCode::CONFLICT, "duplicate_graph_id", None)
            }
            TaskGraphError::DuplicateScheduleId(_) => {
                (StatusCode::CONFLICT, "duplicate_schedule_id", None)
            }
            TaskGraphError::StaleVersion { .. } => (StatusCode::CONFLICT, "stale_version", None),
            TaskGraphError::ValidationFailed { errors, .. } => (
                StatusCode::BAD_REQUEST,
                "validation_failed",
                Some(errors.clone()),
            ),
            TaskGraphError::InvalidGraphId(_) | TaskGraphError::InvalidSchedule(_) => {
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

#[derive(Debug)]
pub(crate) struct TgDecodedCreateBody {
    pub(crate) graph: TaskGraphDefinition,
}

#[derive(Debug)]
pub(crate) struct TgDecodedPatchBody {
    pub(crate) graph: TaskGraphDefinition,
    pub(crate) expected_version: u32,
}

pub(crate) fn decode_create_body(bytes: &[u8]) -> Result<TgDecodedCreateBody, TaskGraphApiError> {
    let value = parse_body_json(bytes)?;
    let graph = decode_graph_field(&value)?;
    Ok(TgDecodedCreateBody { graph })
}

pub(crate) fn decode_patch_body(bytes: &[u8]) -> Result<TgDecodedPatchBody, TaskGraphApiError> {
    let value = parse_body_json(bytes)?;
    let graph = decode_graph_field(&value)?;
    let expected_version = decode_expected_version(&value)?;
    Ok(TgDecodedPatchBody {
        graph,
        expected_version,
    })
}

fn parse_body_json(bytes: &[u8]) -> Result<Value, TaskGraphApiError> {
    task_graph::parse_json_source(bytes, "$").map_err(validation_failed)
}

fn decode_graph_field(value: &Value) -> Result<TaskGraphDefinition, TaskGraphApiError> {
    let Some(object) = value.as_object() else {
        return Err(validation_failed(vec![validation_error(
            "$",
            "invalid_type",
            "Expected request body to be an object",
        )]));
    };
    let Some(graph) = object.get("graph") else {
        return Err(validation_failed(vec![validation_error(
            "graph",
            "missing_field",
            "Missing required field 'graph'",
        )]));
    };

    task_graph::validate_graph_value_at(graph.clone(), "graph").map_err(validation_failed)
}

fn decode_expected_version(value: &Value) -> Result<u32, TaskGraphApiError> {
    let Some(object) = value.as_object() else {
        return Err(validation_failed(vec![validation_error(
            "$",
            "invalid_type",
            "Expected request body to be an object",
        )]));
    };
    let Some(value) = object.get("expected_version") else {
        return Err(validation_failed(vec![validation_error(
            "expected_version",
            "missing_field",
            "Missing required field 'expected_version'",
        )]));
    };
    let Some(version) = value.as_u64().and_then(|v| u32::try_from(v).ok()) else {
        return Err(validation_failed(vec![validation_error(
            "expected_version",
            "invalid_type",
            "Expected field 'expected_version' to be an unsigned 32-bit integer",
        )]));
    };
    Ok(version)
}

fn validation_failed(errors: Vec<TaskGraphValidationError>) -> TaskGraphApiError {
    TaskGraphApiError(TaskGraphError::ValidationFailed {
        count: errors.len(),
        errors,
    })
}

fn validation_error(path: &str, code: &str, message: &str) -> TaskGraphValidationError {
    TaskGraphValidationError {
        path: path.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
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
pub(crate) struct TgRunEventsResponse {
    pub(crate) events: Vec<task_graph::RunEvent>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TgRunCheckpointsResponse {
    pub(crate) checkpoints: Vec<task_graph::SuperstepCheckpoint>,
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
