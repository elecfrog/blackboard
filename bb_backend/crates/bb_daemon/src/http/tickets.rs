use std::collections::{BTreeMap, BTreeSet};

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::Json;
use bb_core::{
    DeprecateTicketInput, DeprecateTicketResult, InboxError, ProjectBoard, ProjectMeta,
    TicketAttachment, TicketFrontmatterPatch, TicketWriteResult, UpdateTicketInput,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState};

#[derive(Debug, Serialize)]
pub(super) struct TicketsResponse {
    generated_at: String,
    project: String,
    meta: ProjectMeta,
    current_counter: String,
    tickets: Vec<HttpTicket>,
}

#[derive(Debug, Serialize)]
pub(super) struct HttpTicket {
    id: String,
    lane: String,
    title: String,
    status: String,
    created_at: String,
    updated_at: String,
    file_name: String,
    file_path: String,
    dependencies: Vec<String>,
    attachments: Vec<TicketAttachment>,
    spec: Option<bb_core::TicketSpec>,
    extra: bb_core::FrontmatterExtra,
}

/// Runtime ticket payload for the web Dashboard. The web app now treats
/// bb as its C/S data source instead of falling back to exported JSON.
pub(super) async fn list_tickets(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<TicketsResponse>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(build_tickets_response(&board)?))
}

/// Single-ticket structured endpoint. Unlike the index endpoint, this reads the
/// ticket source file so JSON tickets expose their BDD spec without bloating the
/// persistent `__tickets__.json` index.
pub(super) async fn ticket_detail(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<HttpTicket>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    let ticket = board.read_ticket_by_id(&id)?;
    let id = ticket.id.unwrap_or_default();
    let dependencies = extract_dependencies(&ticket.extra, &id);
    let attachments = ticket.attachments;
    let title = ticket.title.unwrap_or_else(|| {
        ticket
            .name
            .trim_end_matches(".md")
            .trim_end_matches(".json")
            .to_string()
    });

    Ok(Json(HttpTicket {
        id,
        lane: ticket.lane.unwrap_or_else(|| "unknown".to_string()),
        title,
        status: ticket
            .status
            .unwrap_or_else(|| bb_core::ticket::default_ticket_status().to_string()),
        created_at: ticket.created_at.unwrap_or_default(),
        updated_at: ticket.updated_at.unwrap_or_default(),
        file_name: ticket.name,
        file_path: ticket.path,
        dependencies,
        attachments,
        spec: ticket.spec,
        extra: ticket.extra,
    }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct PatchTicketInput {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    lane: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    depends_on: Option<Vec<String>>,
    #[serde(default)]
    attachments: Option<Vec<TicketAttachment>>,
    #[serde(default)]
    spec: Option<bb_core::TicketSpec>,
}

/// Narrow ticket mutation endpoint used by the kanban UI.
///
/// The route intentionally delegates to bb-core's structured `update_ticket`
/// path and only accepts status/lane plus the dedicated assignee field.
/// It does not accept raw Markdown patches or arbitrary frontmatter writes.
pub(super) async fn patch_ticket(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    input: Result<Json<PatchTicketInput>, JsonRejection>,
) -> Result<Json<TicketWriteResult>, ApiError> {
    let Json(input) = input.map_err(|err| {
        ApiError(InboxError::InvalidInput(format!(
            "invalid patch ticket body: {err}"
        )))
    })?;

    if input.status.is_none()
        && input.lane.is_none()
        && input.assignee.is_none()
        && input.depends_on.is_none()
        && input.attachments.is_none()
        && input.spec.is_none()
    {
        return Err(ApiError(InboxError::InvalidInput(
            "patch ticket requires status, lane, assignee, depends_on, attachments, or spec"
                .to_string(),
        )));
    }

    let board = state.workspace()?.open_project(&project)?;
    let workspace_root = state.workspace_root()?;
    let dependency_patch = match input.depends_on {
        Some(deps) => Some(validate_dependency_patch(&board, &id, deps)?),
        None => None,
    };
    let attachment_patch = match input.attachments {
        Some(attachments) => Some(validate_attachment_patch(attachments)?),
        None => None,
    };

    let frontmatter = if input.status.is_some()
        || input.lane.is_some()
        || input.assignee.is_some()
        || dependency_patch.is_some()
        || attachment_patch.is_some()
        || input.spec.is_some()
    {
        let mut patch = TicketFrontmatterPatch {
            status: input.status,
            lane: input.lane,
            spec: input.spec,
            ..TicketFrontmatterPatch::default()
        };
        if let Some(assignee) = input.assignee {
            let trimmed = assignee.trim().to_string();
            if trimmed.is_empty() {
                patch.remove.push("assignee".to_string());
            } else {
                bb_core::agents_registry::validate_assignee_for_project(
                    &workspace_root,
                    &project,
                    &trimmed,
                )?;
                patch.extra.insert("assignee".to_string(), trimmed);
            }
        }
        if let Some(deps) = dependency_patch {
            patch.remove.push("dependencies".to_string());
            if deps.is_empty() {
                patch.remove.push("depends_on".to_string());
            } else {
                patch.extra.insert("depends_on".to_string(), deps.join(" "));
            }
        }
        if let Some(attachments) = attachment_patch {
            patch.attachments = Some(attachments);
        }
        Some(patch)
    } else {
        None
    };

    Ok(Json(
        board.update_ticket(UpdateTicketInput { id, frontmatter })?,
    ))
}

pub(super) async fn deprecate_ticket(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<DeprecateTicketResult>, ApiError> {
    let board = state.workspace()?.open_project(&project)?;
    Ok(Json(board.deprecate_ticket(DeprecateTicketInput { id })?))
}

fn validate_dependency_patch(
    board: &ProjectBoard,
    ticket_id: &str,
    deps: Vec<String>,
) -> Result<Vec<String>, InboxError> {
    let mut normalized = Vec::new();
    for dep in deps {
        let dep = dep.trim();
        if dep.is_empty() {
            continue;
        }
        if dep.len() != 6 || !dep.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(InboxError::InvalidInput(format!(
                "dependency `{dep}` must be a six-digit ticket id"
            )));
        }
        if dep == ticket_id {
            return Err(InboxError::InvalidInput(
                "ticket cannot depend on itself".to_string(),
            ));
        }
        if !normalized.iter().any(|item| item == dep) {
            normalized.push(dep.to_string());
        }
    }

    let mut existing = BTreeMap::new();
    for entry in board.list_tickets()?.tickets {
        let Some(id) = entry.id.clone() else {
            continue;
        };
        existing.insert(id.clone(), extract_dependencies(&entry.extra, &id));
    }

    if !existing.contains_key(ticket_id) {
        return Err(InboxError::TicketIdNotFound(ticket_id.to_string()));
    }
    for dep in &normalized {
        if !existing.contains_key(dep) {
            return Err(InboxError::InvalidInput(format!(
                "dependency `{dep}` does not exist"
            )));
        }
    }

    existing.insert(ticket_id.to_string(), normalized.clone());
    if let Some(cycle) = find_dependency_cycle(&existing) {
        return Err(InboxError::InvalidInput(format!(
            "dependency graph must stay acyclic: {}",
            cycle.join(" -> ")
        )));
    }

    Ok(normalized)
}

fn validate_attachment_patch(
    attachments: Vec<TicketAttachment>,
) -> Result<Vec<TicketAttachment>, InboxError> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();

    for attachment in attachments {
        let kind = normalize_attachment_kind(&attachment.kind)?;
        let target = normalize_attachment_target(&kind, &attachment.target)?;
        let label = normalize_optional_attachment_text("attachment label", attachment.label, 160)?;
        let description = normalize_optional_attachment_text(
            "attachment description",
            attachment.description,
            500,
        )?;
        let key = format!("{kind}\u{0}{target}");
        if !seen.insert(key) {
            continue;
        }
        normalized.push(TicketAttachment {
            kind,
            target,
            label,
            description,
        });
    }

    Ok(normalized)
}

fn normalize_attachment_kind(kind: &str) -> Result<String, InboxError> {
    let normalized = kind.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(InboxError::InvalidInput(
            "attachment kind cannot be empty".to_string(),
        ));
    }
    if normalized.len() > 48
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(InboxError::InvalidInput(
            "attachment kind must be ascii lower-case, digits, hyphen, or underscore".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_attachment_target(kind: &str, target: &str) -> Result<String, InboxError> {
    let normalized = target.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err(InboxError::InvalidInput(
            "attachment target cannot be empty".to_string(),
        ));
    }
    if normalized.len() > 1024 || normalized.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err(InboxError::InvalidInput(
            "attachment target is too long or contains control characters".to_string(),
        ));
    }
    if matches!(kind, "wiki" | "file" | "artifact")
        && (normalized.starts_with('/') || normalized.split('/').any(|part| part == ".."))
    {
        return Err(InboxError::InvalidInput(format!(
            "{kind} attachment target must stay relative"
        )));
    }
    Ok(normalized)
}

fn normalize_optional_attachment_text(
    field: &str,
    value: Option<String>,
    max_len: usize,
) -> Result<Option<String>, InboxError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.len() > max_len || normalized.chars().any(|ch| ch == '\0') {
        return Err(InboxError::InvalidInput(format!("{field} is invalid")));
    }
    Ok(Some(normalized))
}

fn find_dependency_cycle(dependencies: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    fn visit(
        id: &str,
        dependencies: &BTreeMap<String, Vec<String>>,
        state: &mut BTreeMap<String, u8>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match state.get(id).copied().unwrap_or(0) {
            2 => return None,
            1 => {
                let start = stack.iter().position(|item| item == id).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(id.to_string());
                return Some(cycle);
            }
            _ => {}
        }
        state.insert(id.to_string(), 1);
        stack.push(id.to_string());
        for dep in dependencies.get(id).into_iter().flatten() {
            if let Some(cycle) = visit(dep, dependencies, state, stack) {
                return Some(cycle);
            }
        }
        stack.pop();
        state.insert(id.to_string(), 2);
        None
    }

    let mut state = BTreeMap::new();
    let mut stack = Vec::new();
    for id in dependencies.keys() {
        if let Some(cycle) = visit(id, dependencies, &mut state, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn build_tickets_response(board: &ProjectBoard) -> Result<TicketsResponse, InboxError> {
    let meta = board.read_project_meta()?;

    // Read from the persisted JSON index (projects/<project>/__tickets__.json).
    // If the index does not exist yet, read_ticket_index() will rebuild it
    // from Markdown sources on first access.
    let index = board.read_ticket_index()?;
    let current_counter = index.current_counter.clone();
    let mut tickets = Vec::new();

    for entry in index.tickets {
        let id = entry.id.unwrap_or_default();
        let dependencies = extract_dependencies(&entry.extra, &id);
        let attachments = entry.attachments;

        tickets.push(HttpTicket {
            id,
            lane: entry.lane.unwrap_or_else(|| "unknown".to_string()),
            title: entry.title.unwrap_or_else(|| {
                entry
                    .name
                    .trim_end_matches(".md")
                    .trim_end_matches(".json")
                    .to_string()
            }),
            status: entry
                .status
                .unwrap_or_else(|| bb_core::ticket::default_ticket_status().to_string()),
            created_at: entry.created_at.unwrap_or_default(),
            updated_at: entry.updated_at.unwrap_or_default(),
            file_name: entry.name,
            file_path: entry.path,
            dependencies,
            attachments,
            spec: entry.spec,
            extra: entry.extra,
        });
    }

    tickets.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(TicketsResponse {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        project: board.name().to_string(),
        meta,
        current_counter,
        tickets,
    })
}

fn extract_dependencies(extra: &bb_core::FrontmatterExtra, self_id: &str) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for key in ["depends_on", "dependencies"] {
        let Some(value) = extra.get(key) else {
            continue;
        };
        for token in value.split(|ch: char| ch == ',' || ch.is_whitespace()) {
            if token.len() == 6 && token.chars().all(|ch| ch.is_ascii_digit()) && token != self_id {
                ids.insert(token.to_string());
            }
        }
    }
    ids.into_iter().collect()
}
