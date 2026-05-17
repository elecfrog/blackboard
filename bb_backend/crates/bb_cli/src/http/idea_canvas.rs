use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use bb_core::{
    CreateIdeaCanvasInput, CreateStickyNoteInput, IdeaCanvasDeleteResult, IdeaCanvasDetail,
    IdeaCanvasIndex, IdeaCanvasWriteResult, InboxError, StickyNoteDeleteResult,
    StickyNoteWriteResult, UpdateIdeaCanvasInput, UpdateStickyNoteInput,
};
use serde::Deserialize;

use super::{ApiError, AppState};

pub(super) async fn list_canvases(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<IdeaCanvasIndex>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.read_idea_canvas_index()?))
}

pub(super) async fn read_canvas(
    State(state): State<AppState>,
    Path((project, canvas_id)): Path<(String, String)>,
) -> Result<Json<IdeaCanvasDetail>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.read_idea_canvas(&canvas_id)?))
}

pub(super) async fn create_canvas(
    State(state): State<AppState>,
    Path(project): Path<String>,
    input: Result<Json<CreateIdeaCanvasInput>, JsonRejection>,
) -> Result<(StatusCode, Json<IdeaCanvasWriteResult>), ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid create idea canvas body: {err}"
        )))
    })?;
    let board = state.workspace()?.open_project(&project)?;
    Ok((StatusCode::CREATED, Json(board.create_idea_canvas(input)?)))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatchCanvasBody {
    #[serde(default)]
    title: Option<String>,
}

pub(super) async fn patch_canvas(
    State(state): State<AppState>,
    Path((project, canvas_id)): Path<(String, String)>,
    input: Result<Json<PatchCanvasBody>, JsonRejection>,
) -> Result<Json<IdeaCanvasWriteResult>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid patch idea canvas body: {err}"
        )))
    })?;
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.update_idea_canvas(UpdateIdeaCanvasInput {
        id: canvas_id,
        title: input.title,
    })?))
}

pub(super) async fn delete_canvas(
    State(state): State<AppState>,
    Path((project, canvas_id)): Path<(String, String)>,
) -> Result<Json<IdeaCanvasDeleteResult>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.delete_idea_canvas(&canvas_id)?))
}

pub(super) async fn create_note(
    State(state): State<AppState>,
    Path((project, canvas_id)): Path<(String, String)>,
    input: Result<Json<CreateStickyNoteInput>, JsonRejection>,
) -> Result<(StatusCode, Json<StickyNoteWriteResult>), ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid create sticky note body: {err}"
        )))
    })?;
    let board = state.workspace()?.open_project(&project)?;
    Ok((
        StatusCode::CREATED,
        Json(board.create_sticky_note(&canvas_id, input)?),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatchNoteBody {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    color: Option<String>,
}

pub(super) async fn patch_note(
    State(state): State<AppState>,
    Path((project, canvas_id, note_id)): Path<(String, String, String)>,
    input: Result<Json<PatchNoteBody>, JsonRejection>,
) -> Result<Json<StickyNoteWriteResult>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid patch sticky note body: {err}"
        )))
    })?;
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.update_sticky_note(UpdateStickyNoteInput {
        canvas_id,
        note_id,
        text: input.text,
        x: input.x,
        y: input.y,
        color: input.color,
    })?))
}

pub(super) async fn delete_note(
    State(state): State<AppState>,
    Path((project, canvas_id, note_id)): Path<(String, String, String)>,
) -> Result<Json<StickyNoteDeleteResult>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.delete_sticky_note(&canvas_id, &note_id)?))
}
