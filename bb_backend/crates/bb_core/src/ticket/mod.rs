//! Ticket CRUD, indexing, search, and maintenance logic for a single project.

use crate::{
    agents_registry, validate_lane_id, validate_ticket_lane, AppendTicketSectionsInput, Blackboard,
    BoardSummary, CreateTicketInput, InboxError, Ticket, TicketById, TicketEntry, TicketList,
    TicketMetadataDiagnostic, TicketSearchMatch, TicketSearchResult, TicketWriteResult,
    TicketWriteTicket, UpdateTicketInput,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

mod entry;
mod frontmatter;
mod index;
mod render;

use crate::common::search::{matching_lines, validate_search_query};
pub(crate) use entry::{is_six_digit_id, ticket_entry_from_content};
use frontmatter::{
    extract_extra_fields, reject_core_frontmatter_key, reject_extra_frontmatter_key,
    render_frontmatter, sanitize_extra_for_write, split_ticket_frontmatter,
};
use render::{append_ticket_body_sections, render_ticket, TicketRenderInput};

pub const TICKET_STATUSES: [&str; 6] = [
    "todo",
    "in_progress",
    "blocked",
    "review",
    "done",
    "archived",
];

pub fn default_ticket_status() -> &'static str {
    "todo"
}

pub fn is_open_ticket_status(status: &str) -> bool {
    matches!(status, "todo" | "in_progress" | "blocked" | "review")
}
pub(crate) const CONSISTENCY_CHECKS: [&str; 6] = [
    "id",
    "lane",
    "filename",
    "duplicates",
    "counter",
    "lane_catalog",
];
pub const REQUIRED_METADATA_FIELDS: [&str; 5] = ["id", "lane", "title", "status", "updated_at"];

/// Core frontmatter fields that the system always owns and writes.
pub const CORE_FRONTMATTER_FIELDS: [&str; 7] = [
    "id",
    "lane",
    "family",
    "title",
    "status",
    "created_at",
    "updated_at",
];

pub(crate) struct TicketIdLock {
    path: PathBuf,
}

impl TicketIdLock {
    pub fn acquire(path: PathBuf) -> Result<Self, InboxError> {
        for _ in 0..50 {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(source) => {
                    return Err(InboxError::Io {
                        path: path.clone(),
                        source,
                    })
                }
            }
        }
        Err(InboxError::TicketIdLockTimeout(path))
    }
}

impl Drop for TicketIdLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

// ─── impl Blackboard: ticket methods ─────────────────────────────────────────

impl Blackboard {
    /// Return the flat `tickets/` directory path, ensuring it exists and is not a symlink.
    fn tickets_dir(&self) -> Result<PathBuf, InboxError> {
        let dir = self.root().join("tickets");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|source| InboxError::Io {
                path: dir.clone(),
                source,
            })?;
        }
        let metadata = fs::symlink_metadata(&dir).map_err(|source| InboxError::Io {
            path: dir.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(InboxError::InvalidInput(
                "tickets directory is a symlink".to_string(),
            ));
        }
        Ok(dir)
    }

    pub fn create_ticket(&self, input: CreateTicketInput) -> Result<TicketWriteResult, InboxError> {
        let meta = self.read_project_meta()?;
        let lane_entry = validate_ticket_lane(&input.lane, &meta.lanes)?;
        let lane = lane_entry.id.clone();
        let title = validate_required_string("title", &input.title)?;
        let workflow_status = validate_ticket_status(&input.status)?;

        let extra = sanitize_extra_for_write(&input.extra)?;
        if let Some(assignee) = extra.get("assignee") {
            agents_registry::validate_assignee_for_project(
                &self.workspace_root()?,
                self.name(),
                assignee,
            )?;
        }

        let dir = self.tickets_dir()?;
        let id = self.allocate_ticket_id()?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let slug_source = input.slug.as_deref().unwrap_or(title);
        let slug = crate::slug_segment(slug_source, "ticket");
        let file_name = format!("{id}-{slug}.md");
        validate_ticket_name(&file_name)?;
        let path = dir.join(&file_name);
        if path.exists() {
            return Err(InboxError::TicketWriteConflict(format!(
                "ticket file already exists: tickets/{file_name}"
            )));
        }
        let sections = input.sections.unwrap_or_default();
        let content = render_ticket(TicketRenderInput {
            id: &id,
            lane: &lane,
            title,
            created_at: &date,
            updated_at: &date,
            status: workflow_status,
            extra: &extra,
            sections: &sections,
        });
        write_new_file(&path, &content)?;

        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id,
                lane: lane.clone(),
                title: title.to_string(),
                status: workflow_status.to_string(),
                created_at: date.clone(),
                updated_at: date,
                file_name: file_name.clone(),
                path: format!("tickets/{file_name}"),
                extra,
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    pub fn update_ticket(&self, input: UpdateTicketInput) -> Result<TicketWriteResult, InboxError> {
        let id = validate_ticket_id(&input.id)?;
        if input.frontmatter.is_none() {
            return Err(InboxError::InvalidInput(
                "update_ticket requires frontmatter".to_string(),
            ));
        }

        let list = self.list_tickets()?;
        let matches: Vec<TicketEntry> = list
            .tickets
            .into_iter()
            .filter(|entry| entry.id.as_deref() == Some(id))
            .collect();
        let entry = match matches.as_slice() {
            [] => return Err(InboxError::TicketIdNotFound(id.to_string())),
            [entry] => entry.clone(),
            many => {
                return Err(InboxError::DuplicateTicketId {
                    id: id.to_string(),
                    matches: many.iter().map(|entry| entry.path.clone()).collect(),
                })
            }
        };

        let dir = self.tickets_dir()?;
        let source_path = dir.join(&entry.name);
        let canonical_source = crate::canonicalize(&source_path)?;
        if !canonical_source.starts_with(&dir) {
            return Err(InboxError::InvalidName(entry.name));
        }

        let original = fs::read_to_string(&canonical_source).map_err(|source| InboxError::Io {
            path: canonical_source.clone(),
            source,
        })?;
        let (mut fields, body) = split_ticket_frontmatter(&original)?;
        let lane = match fields.get("lane").cloned() {
            Some(lane) => lane,
            None => match fields.remove("family") {
                Some(legacy) => {
                    fields.insert("lane".to_string(), legacy.clone());
                    legacy
                }
                None => {
                    return Err(InboxError::InvalidInput(
                        "ticket frontmatter missing lane".to_string(),
                    ));
                }
            },
        };
        validate_lane_id(&lane)?;
        let ticket_id = fields
            .get("id")
            .cloned()
            .ok_or_else(|| InboxError::InvalidInput("ticket frontmatter missing id".to_string()))?;
        if ticket_id != id {
            return Err(InboxError::InvalidInput(format!(
                "ticket frontmatter id `{ticket_id}` does not match requested id `{id}`"
            )));
        }

        if let Some(patch) = input.frontmatter {
            if let Some(title) = patch.title {
                fields.insert(
                    "title".to_string(),
                    validate_required_string("title", &title)?.to_string(),
                );
            }
            if let Some(status) = patch.status {
                fields.insert(
                    "status".to_string(),
                    validate_ticket_status(&status)?.to_string(),
                );
            }
            if let Some(new_lane) = patch.lane {
                let meta = self.read_project_meta()?;
                let entry = validate_ticket_lane(&new_lane, &meta.lanes)?;
                fields.insert("lane".to_string(), entry.id.clone());
            }
            for (key, value) in patch.extra {
                reject_extra_frontmatter_key(&key)?;
                validate_required_string("extra key", &key)?;
                if key == "assignee" {
                    agents_registry::validate_assignee_for_project(
                        &self.workspace_root()?,
                        self.name(),
                        &value,
                    )?;
                }
                fields.insert(key, value);
            }
            for key in patch.remove {
                reject_core_frontmatter_key(&key)?;
                fields.remove(&key);
            }
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        fields.insert("updated_at".to_string(), date.clone());

        let content = render_frontmatter(&fields) + body;
        crate::write_file_atomic(&canonical_source, &content)?;

        let final_status = fields
            .get("status")
            .cloned()
            .unwrap_or_else(|| default_ticket_status().to_string());
        let final_lane = fields.get("lane").cloned().unwrap_or_default();
        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id: id.to_string(),
                lane: final_lane,
                title: fields.get("title").cloned().unwrap_or_default(),
                status: final_status,
                created_at: fields.get("created_at").cloned().unwrap_or_default(),
                updated_at: date,
                file_name: entry.name.clone(),
                path: format!("tickets/{}", entry.name),
                extra: extract_extra_fields(&fields),
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    pub fn append_ticket_sections(
        &self,
        input: AppendTicketSectionsInput,
    ) -> Result<TicketWriteResult, InboxError> {
        let id = validate_ticket_id(&input.id)?;
        if input.progress.is_empty() && input.record.is_empty() && input.next_step.is_empty() {
            return Err(InboxError::InvalidInput(
                "append_ticket_sections requires progress, record, or next_step".to_string(),
            ));
        }

        let list = self.list_tickets()?;
        let matches: Vec<TicketEntry> = list
            .tickets
            .into_iter()
            .filter(|entry| entry.id.as_deref() == Some(id))
            .collect();
        let entry = match matches.as_slice() {
            [] => return Err(InboxError::TicketIdNotFound(id.to_string())),
            [entry] => entry.clone(),
            many => {
                return Err(InboxError::DuplicateTicketId {
                    id: id.to_string(),
                    matches: many.iter().map(|entry| entry.path.clone()).collect(),
                })
            }
        };

        let dir = self.tickets_dir()?;
        let path = dir.join(&entry.name);
        let canonical = crate::canonicalize(&path)?;
        if !canonical.starts_with(&dir) {
            return Err(InboxError::InvalidName(entry.name));
        }

        let original = fs::read_to_string(&canonical).map_err(|source| InboxError::Io {
            path: canonical.clone(),
            source,
        })?;
        let (mut fields, body) = split_ticket_frontmatter(&original)?;
        let lane = match fields.get("lane").cloned() {
            Some(lane) => lane,
            None => match fields.remove("family") {
                Some(legacy) => {
                    fields.insert("lane".to_string(), legacy.clone());
                    legacy
                }
                None => {
                    return Err(InboxError::InvalidInput(
                        "ticket frontmatter missing lane".to_string(),
                    ));
                }
            },
        };
        validate_lane_id(&lane)?;
        let ticket_id = fields
            .get("id")
            .cloned()
            .ok_or_else(|| InboxError::InvalidInput("ticket frontmatter missing id".to_string()))?;
        if ticket_id != id {
            return Err(InboxError::InvalidInput(format!(
                "ticket frontmatter id `{ticket_id}` does not match requested id `{id}`"
            )));
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        fields.insert("updated_at".to_string(), date.clone());
        let body =
            append_ticket_body_sections(body, &input.progress, &input.record, &input.next_step);
        let content = render_frontmatter(&fields) + &body;
        crate::write_file_atomic(&canonical, &content)?;

        let final_status = fields
            .get("status")
            .cloned()
            .unwrap_or_else(|| default_ticket_status().to_string());

        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id: id.to_string(),
                lane,
                title: fields.get("title").cloned().unwrap_or_default(),
                status: final_status,
                created_at: fields.get("created_at").cloned().unwrap_or_default(),
                updated_at: date,
                file_name: entry.name.clone(),
                path: format!("tickets/{}", entry.name),
                extra: extract_extra_fields(&fields),
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    pub fn list_tickets(&self) -> Result<TicketList, InboxError> {
        let dir = self.tickets_dir()?;
        let mut tickets = Vec::new();

        for entry in fs::read_dir(&dir).map_err(|source| InboxError::Io {
            path: dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| InboxError::Io {
                path: dir.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| InboxError::Io {
                path: entry.path(),
                source,
            })?;
            if !file_type.is_file() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            if validate_ticket_name(&name).is_ok() {
                let path = entry.path();
                let content = fs::read_to_string(&path).map_err(|source| InboxError::Io {
                    path: path.clone(),
                    source,
                })?;
                tickets.push(ticket_entry_from_content(name, &content));
            }
        }

        tickets.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(TicketList {
            current_counter: String::new(),
            tickets,
        })
    }

    pub fn read_ticket(&self, name: &str) -> Result<Ticket, InboxError> {
        let name = validate_ticket_name(name)?;
        let dir = self.tickets_dir()?;
        let path = dir.join(&name);

        let canonical = match crate::canonicalize(&path) {
            Ok(path) => path,
            Err(InboxError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(InboxError::TicketNotFound { name });
            }
            Err(err) => return Err(err),
        };

        if !canonical.starts_with(&dir) {
            return Err(InboxError::InvalidName(name));
        }

        let content = fs::read_to_string(&canonical).map_err(|source| InboxError::Io {
            path: canonical,
            source,
        })?;

        let entry = ticket_entry_from_content(name.clone(), &content);

        Ok(Ticket {
            status: entry
                .status
                .unwrap_or_else(|| default_ticket_status().to_string()),
            path: format!("tickets/{name}"),
            name,
            content,
        })
    }

    pub fn read_ticket_by_id(&self, id: &str) -> Result<TicketById, InboxError> {
        let id = validate_ticket_id(id)?;
        let list = self.list_tickets()?;
        let matches: Vec<TicketEntry> = list
            .tickets
            .into_iter()
            .filter(|entry| entry.id.as_deref() == Some(id))
            .collect();

        match matches.as_slice() {
            [] => Err(InboxError::TicketIdNotFound(id.to_string())),
            [entry] => {
                let ticket = self.read_ticket(&entry.name)?;
                Ok(TicketById {
                    name: entry.name.clone(),
                    path: entry.path.clone(),
                    content: ticket.content,
                    id: entry.id.clone(),
                    lane: entry.lane.clone(),
                    title: entry.title.clone(),
                    status: entry.status.clone(),
                    created_at: entry.created_at.clone(),
                    updated_at: entry.updated_at.clone(),
                    extra: entry.extra.clone(),
                    metadata_error: entry.metadata_error.clone(),
                    metadata_warnings: entry.metadata_warnings.clone(),
                })
            }
            many => Err(InboxError::DuplicateTicketId {
                id: id.to_string(),
                matches: many.iter().map(|entry| entry.path.clone()).collect(),
            }),
        }
    }

    pub fn board_summary(&self) -> Result<BoardSummary, InboxError> {
        let list = self.list_tickets()?;
        let mut by_status = BTreeMap::new();
        let mut by_lane = BTreeMap::new();
        for status in TICKET_STATUSES {
            by_status.insert(status.to_string(), 0);
        }
        let mut metadata_warnings = Vec::new();
        let mut metadata_errors = Vec::new();

        for entry in &list.tickets {
            let status = entry
                .status
                .clone()
                .unwrap_or_else(|| default_ticket_status().to_string());
            *by_status.entry(status).or_insert(0) += 1;
            let lane_key = entry
                .lane
                .clone()
                .unwrap_or_else(|| "__unknown".to_string());
            *by_lane.entry(lane_key).or_insert(0) += 1;
            for warning in &entry.metadata_warnings {
                metadata_warnings.push(TicketMetadataDiagnostic {
                    path: entry.path.clone(),
                    message: warning.clone(),
                });
            }
            if let Some(error) = &entry.metadata_error {
                metadata_errors.push(TicketMetadataDiagnostic {
                    path: entry.path.clone(),
                    message: error.clone(),
                });
            }
        }

        Ok(BoardSummary {
            total: list.tickets.len(),
            by_status,
            by_lane,
            metadata_warning_count: metadata_warnings.len(),
            metadata_error_count: metadata_errors.len(),
            metadata_warnings,
            metadata_errors,
        })
    }

    pub fn search_tickets(&self, query: &str) -> Result<TicketSearchResult, InboxError> {
        let query = validate_search_query(query)?;
        let needle = query.to_lowercase();
        let mut matches = Vec::new();

        for entry in self.list_tickets()?.tickets {
            let ticket = self.read_ticket(&entry.name)?;
            for (line, snippet) in matching_lines(&ticket.content, &needle) {
                matches.push(TicketSearchMatch {
                    filename: ticket.name.clone(),
                    path: ticket.path.clone(),
                    status: ticket.status.clone(),
                    line,
                    snippet,
                });
            }
        }

        Ok(TicketSearchResult { matches })
    }
}

pub fn validate_ticket_status(status: &str) -> Result<&'static str, InboxError> {
    let trimmed = status.trim();
    if trimmed != status {
        return Err(InboxError::InvalidTicketStatus(status.to_string()));
    }

    TICKET_STATUSES
        .iter()
        .copied()
        .find(|candidate| *candidate == trimmed)
        .ok_or_else(|| InboxError::InvalidTicketStatus(status.to_string()))
}

pub fn validate_ticket_id(id: &str) -> Result<&str, InboxError> {
    let trimmed = id.trim();
    if trimmed != id || trimmed.len() != 6 || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(InboxError::InvalidTicketId(id.to_string()));
    }
    Ok(trimmed)
}

pub fn validate_ticket_name(name: &str) -> Result<String, InboxError> {
    let validated = crate::validate_note_name(name)?;
    let lower = validated.to_ascii_lowercase();

    if validated.contains('\\')
        || lower.contains("%2f")
        || lower.contains("%5c")
        || lower.contains("%2e")
    {
        return Err(InboxError::InvalidName(name.to_string()));
    }

    Ok(validated)
}

pub fn validate_ticket_family(family: &str) -> Result<&'static str, InboxError> {
    let _ = family;
    Err(InboxError::InvalidInput(
        "ticket family has been replaced by lane; see validate_lane_id / validate_ticket_lane"
            .to_string(),
    ))
}

pub(crate) fn validate_required_string<'a>(
    field: &str,
    value: &'a str,
) -> Result<&'a str, InboxError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(InboxError::InvalidInput(format!("{field} is required")));
    }
    if trimmed != value {
        return Err(InboxError::InvalidInput(format!(
            "{field} must not have surrounding whitespace"
        )));
    }
    Ok(trimmed)
}

fn write_new_file(path: &Path, content: &str) -> Result<(), InboxError> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(InboxError::TicketWriteConflict(format!(
                "ticket file already exists: {}",
                path.display()
            )))
        }
        Err(source) => {
            return Err(InboxError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    file.write_all(content.as_bytes())
        .map_err(|source| InboxError::Io {
            path: path.to_path_buf(),
            source,
        })
}
