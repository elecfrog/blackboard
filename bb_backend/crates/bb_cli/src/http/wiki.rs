use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::Json;
use bb_core::{InboxError, WikiAsset, WikiContentResponse, WikiTreeResponse};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState};

#[derive(Debug, Deserialize)]
pub(super) struct WikiPathQuery {
    path: String,
}

pub(super) async fn wiki_tree_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<WikiTreeResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.list_wiki_tree()?))
}

/// Read a wiki Markdown document. The relative path comes in as a query
/// parameter (`?path=...`) instead of a URL wildcard so nested paths like
/// `modules/foo/index.md` survive unambiguous routing and URL encoding.
pub(super) async fn wiki_file_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<WikiPathQuery>,
) -> Result<Json<WikiContentResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.read_wiki_content(&query.path)?))
}

/// Serve a relative wiki asset (images, svg, pdf, ...) as a binary
/// response. Same path safety rules as `wiki_file_handler`; the allowed
/// extension set lives in `bb_core::wiki`.
pub(super) async fn wiki_asset_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<WikiPathQuery>,
) -> Result<Response, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    let WikiAsset {
        bytes,
        content_type,
    } = board.read_wiki_asset(&query.path)?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .map_err(|e| {
            ApiError(InboxError::InvalidInput(format!(
                "failed to build asset response: {}",
                e
            )))
        })?;
    Ok(response)
}

#[derive(Debug, Serialize)]
pub(super) struct WikiUploadResponse {
    uploaded: Vec<String>,
    errors: Vec<String>,
}

/// Multipart upload into `projects/<project>/wiki/`. Creates the wiki
/// directory on first use (so the UI doesn't 404 before any content
/// exists) and routes each field through the same `validate_wiki_path`
/// guard as the read endpoints — the only place that may *create* new
/// subdirectories along the way.
pub(super) async fn wiki_upload_handler(
    State(state): State<AppState>,
    Path(project): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<WikiUploadResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    // Ensure `wiki/` exists so the very first upload succeeds on a
    // freshly-migrated project that has no wiki yet.
    let wiki_root = board.ensure_wiki_root()?;

    let mut uploaded = Vec::new();
    let mut errors = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError(InboxError::InvalidInput(format!(
            "failed to read multipart field: {}",
            e
        )))
    })? {
        let field_name = field.name().unwrap_or("file").to_string();

        let raw_name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("unnamed-{}", &field_name));

        // Normalize separators before validation; skip obvious dotfiles so
        // `.DS_Store` and friends never hit disk.
        let relative_path = raw_name.replace('\\', "/");
        let last_segment = relative_path.rsplit('/').next().unwrap_or("");
        if last_segment.is_empty() || last_segment.starts_with('.') {
            continue;
        }

        // Reuse the core validator so upload, read, and asset all enforce
        // the same path-escape and extension rules.
        let target_path = match board.validate_wiki_path_for_upload(&relative_path) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("{}: {}", relative_path, e));
                continue;
            }
        };

        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    errors.push(format!(
                        "failed to create directory for {}: {}",
                        relative_path, e
                    ));
                    continue;
                }
            }
        }

        let content = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                errors.push(format!("failed to read file {}: {}", relative_path, e));
                continue;
            }
        };

        if let Err(e) = std::fs::write(&target_path, &content) {
            errors.push(format!("failed to write {}: {}", relative_path, e));
            continue;
        }

        uploaded.push(relative_path);
    }

    // Silence unused warning if the caller didn't actually touch the root
    // (e.g. empty upload payload). The directory now definitely exists.
    let _ = wiki_root;

    Ok(Json(WikiUploadResponse { uploaded, errors }))
}
