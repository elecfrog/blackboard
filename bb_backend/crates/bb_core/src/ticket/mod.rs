//! Ticket CRUD, indexing, search, and maintenance logic for a single project.

use crate::{
    agents_registry, validate_lane_id, validate_ticket_lane, AppendTicketSectionsInput, Blackboard,
    BoardSummary, CreateTicketInput, DeprecateTicketInput, DeprecateTicketResult, InboxError,
    Ticket, TicketById, TicketEntry, TicketFrontmatterPatch, TicketList, TicketMetadataDiagnostic,
    TicketSearchMatch, TicketSearchResult, TicketWriteResult, TicketWriteTicket, UpdateTicketInput,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

mod entry;
mod frontmatter;
mod index;
mod render;
mod spec;

use crate::common::search::{matching_lines, validate_search_query};
pub(crate) use entry::{is_six_digit_id, ticket_entry_from_content};
use frontmatter::{
    extract_attachments_field, extract_extra_fields, reject_core_frontmatter_key,
    reject_extra_frontmatter_key, render_frontmatter, sanitize_extra_for_write,
    split_ticket_frontmatter,
};
use render::{append_ticket_body_sections, render_ticket, TicketRenderInput};
pub(crate) use spec::validate_ticket_attachments;
use spec::{
    normalize_ticket_spec, parse_ticket_json_document, parse_ticket_spec_field,
    render_ticket_json_document, render_ticket_spec_body, serialize_ticket_json_document,
    serialize_ticket_spec, TicketJsonDocument, TICKET_SPEC_FRONTMATTER_KEY,
};

pub const TICKET_STATUSES: [&str; 6] = [
    "todo",
    "in_progress",
    "blocked",
    "review",
    "done",
    "archived",
];

#[must_use]
pub const fn default_ticket_status() -> &'static str {
    "todo"
}

#[must_use]
pub fn is_open_ticket_status(status: &str) -> bool {
    matches!(status, "todo" | "in_progress" | "blocked" | "review")
}

#[cfg(feature = "schema")]
pub(crate) fn ticket_json_document_schema() -> serde_json::Value {
    spec::ticket_json_document_schema()
}
pub(crate) const CONSISTENCY_CHECKS: [&str; 6] = [
    "id",
    "lane",
    "filename",
    "duplicates",
    "counter",
    "lane_catalog",
];
pub const REQUIRED_METADATA_FIELDS: [&str; 6] = [
    "id",
    "lane",
    "title",
    "status",
    "updated_at",
    TICKET_SPEC_FRONTMATTER_KEY,
];

/// Core frontmatter fields that the system always owns and writes.
pub const CORE_FRONTMATTER_FIELDS: [&str; 8] = [
    "id",
    "lane",
    "family",
    "title",
    "status",
    "created_at",
    "updated_at",
    TICKET_SPEC_FRONTMATTER_KEY,
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
                Err(source) => return Err(InboxError::Io { path, source }),
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

    fn deprecated_tickets_dir(&self) -> Result<PathBuf, InboxError> {
        let dir = self.tickets_dir()?.join("_deprecated");
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
                "tickets/_deprecated directory is a symlink".to_string(),
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
        let attachments = input.attachments;
        validate_ticket_attachments(&attachments)?;
        let spec = normalize_ticket_spec(input.spec)?;
        let spec_json = serialize_ticket_spec(&spec)?;

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
        let file_name = format!("{id}-{slug}.json");
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
            spec_json: &spec_json,
            attachments: &attachments,
            extra: &extra,
            sections: &sections,
        });
        write_new_file(&path, &content)?;

        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id,
                lane,
                title: title.to_string(),
                status: workflow_status.to_string(),
                created_at: date.clone(),
                updated_at: date,
                file_name: file_name.clone(),
                path: format!("tickets/{file_name}"),
                attachments,
                spec: Some(spec),
                extra,
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    pub fn update_ticket(&self, input: UpdateTicketInput) -> Result<TicketWriteResult, InboxError> {
        let id = validate_ticket_id(&input.id)?;
        let patch = input.frontmatter.ok_or_else(|| {
            InboxError::InvalidInput("update_ticket requires frontmatter".to_string())
        })?;

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
        if entry.name.ends_with(".json") {
            return self.update_json_ticket(id, &entry, &canonical_source, &original, patch);
        }

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

        let mut replacement_body = None;
        let mut migrate_markdown_to_json = false;
        {
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
            if let Some(spec) = patch.spec {
                let spec = normalize_ticket_spec(spec)?;
                let spec_json = serialize_ticket_spec(&spec)?;
                fields.insert(TICKET_SPEC_FRONTMATTER_KEY.to_string(), spec_json);
                replacement_body = Some(format!("\n{}", render_ticket_spec_body(&spec)));
                migrate_markdown_to_json = true;
            }
            if let Some(attachments) = patch.attachments {
                validate_ticket_attachments(&attachments)?;
                if attachments.is_empty() {
                    fields.remove("attachments");
                } else {
                    let serialized = serde_json::to_string(&attachments).map_err(|err| {
                        InboxError::InvalidInput(format!(
                            "attachments could not be serialized: {err}"
                        ))
                    })?;
                    fields.insert("attachments".to_string(), serialized);
                }
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

        if migrate_markdown_to_json {
            let spec = parse_ticket_spec_field(&fields).map_err(InboxError::InvalidInput)?;
            let final_status = fields
                .get("status")
                .cloned()
                .unwrap_or_else(|| default_ticket_status().to_string());
            let final_lane = fields.get("lane").cloned().unwrap_or_default();
            let title = fields.get("title").cloned().unwrap_or_default();
            let created_at = fields.get("created_at").cloned().unwrap_or_default();
            let extra = extract_extra_fields(&fields);
            let attachments = extract_attachments_field(&fields);
            let json_name = format!("{}.json", entry.name.trim_end_matches(".md"));
            validate_ticket_name(&json_name)?;
            let json_path = dir.join(&json_name);
            if json_path.exists() {
                return Err(InboxError::TicketWriteConflict(format!(
                    "ticket JSON file already exists: tickets/{json_name}"
                )));
            }
            let content = render_ticket_json_document(
                id,
                &final_lane,
                &title,
                &created_at,
                &date,
                &final_status,
                spec.clone(),
                attachments.clone(),
                extra.clone(),
            )?;
            write_new_file(&json_path, &content)?;
            fs::remove_file(&canonical_source).map_err(|source| InboxError::Io {
                path: canonical_source.clone(),
                source,
            })?;

            return Ok(TicketWriteResult {
                ticket: TicketWriteTicket {
                    id: id.to_string(),
                    lane: final_lane,
                    title,
                    status: final_status,
                    created_at,
                    updated_at: date,
                    file_name: json_name.clone(),
                    path: format!("tickets/{json_name}"),
                    attachments,
                    spec: Some(spec),
                    extra,
                },
                maintenance: self.ticket_maintenance(),
            });
        }

        let body = replacement_body.as_deref().unwrap_or(body);
        let content = render_frontmatter(&fields) + body;
        crate::write_file_atomic(&canonical_source, &content)?;

        let final_status = fields
            .get("status")
            .cloned()
            .unwrap_or_else(|| default_ticket_status().to_string());
        let final_lane = fields.get("lane").cloned().unwrap_or_default();
        let spec = parse_ticket_spec_field(&fields).ok();
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
                attachments: extract_attachments_field(&fields),
                spec,
                extra: extract_extra_fields(&fields),
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    fn update_json_ticket(
        &self,
        id: &str,
        entry: &TicketEntry,
        path: &Path,
        original: &str,
        frontmatter_patch: TicketFrontmatterPatch,
    ) -> Result<TicketWriteResult, InboxError> {
        let mut document = match parse_ticket_json_document(original) {
            Ok(document) => document,
            Err(message) => {
                return self.recover_invalid_json_ticket(
                    id,
                    entry,
                    path,
                    frontmatter_patch,
                    message,
                );
            }
        };
        if document.id != id {
            return Err(InboxError::InvalidInput(format!(
                "ticket JSON id `{}` does not match requested id `{id}`",
                document.id
            )));
        }

        if let Some(title) = frontmatter_patch.title {
            document.title = validate_required_string("title", &title)?.to_string();
        }
        if let Some(status) = frontmatter_patch.status {
            document.status = validate_ticket_status(&status)?.to_string();
        }
        if let Some(new_lane) = frontmatter_patch.lane {
            let meta = self.read_project_meta()?;
            let entry = validate_ticket_lane(&new_lane, &meta.lanes)?;
            document.lane = entry.id.clone();
        }
        if let Some(spec) = frontmatter_patch.spec {
            let spec = normalize_ticket_spec(spec)?;
            document.set_spec(spec);
        }
        if let Some(attachments) = frontmatter_patch.attachments {
            validate_ticket_attachments(&attachments)?;
            document.attachments = attachments;
            document.extra.remove("attachments");
        }
        for (key, value) in frontmatter_patch.extra {
            reject_extra_frontmatter_key(&key)?;
            validate_required_string("extra key", &key)?;
            if key == "assignee" {
                agents_registry::validate_assignee_for_project(
                    &self.workspace_root()?,
                    self.name(),
                    &value,
                )?;
            }
            document.extra.insert(key, value);
        }
        for key in frontmatter_patch.remove {
            reject_core_frontmatter_key(&key)?;
            if key == "attachments" {
                document.attachments.clear();
            } else {
                document.extra.remove(&key);
            }
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        document.updated_at = date.clone();
        validate_lane_id(&document.lane)?;
        let spec = document.spec();
        let content = serialize_ticket_json_document(&document)?;
        crate::write_file_atomic(path, &content)?;

        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id: id.to_string(),
                lane: document.lane,
                title: document.title,
                status: document.status,
                created_at: document.created_at,
                updated_at: date,
                file_name: entry.name.clone(),
                path: format!("tickets/{}", entry.name),
                attachments: document.attachments.clone(),
                spec: Some(spec),
                extra: document.extra,
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    fn recover_invalid_json_ticket(
        &self,
        id: &str,
        entry: &TicketEntry,
        path: &Path,
        frontmatter_patch: TicketFrontmatterPatch,
        parse_error: String,
    ) -> Result<TicketWriteResult, InboxError> {
        let TicketFrontmatterPatch {
            title,
            status,
            lane,
            spec,
            attachments,
            extra,
            remove,
        } = frontmatter_patch;

        if !remove.is_empty() {
            return Err(InboxError::InvalidInput(
                "remove is not supported when recovering an invalid JSON ticket".to_string(),
            ));
        }

        let Some(spec) = spec else {
            return Err(InboxError::InvalidInput(parse_error));
        };
        let Some(title) = title else {
            return Err(InboxError::InvalidInput(format!(
                "{parse_error}; recovering an invalid JSON ticket requires frontmatter.title"
            )));
        };
        let Some(lane) = lane else {
            return Err(InboxError::InvalidInput(format!(
                "{parse_error}; recovering an invalid JSON ticket requires frontmatter.lane"
            )));
        };

        let meta = self.read_project_meta()?;
        let lane = validate_ticket_lane(&lane, &meta.lanes)?.id.clone();
        let title = validate_required_string("title", &title)?.to_string();
        let status = match status {
            Some(status) => validate_ticket_status(&status)?.to_string(),
            None => default_ticket_status().to_string(),
        };
        let spec = normalize_ticket_spec(spec)?;
        let attachments = attachments.unwrap_or_default();
        validate_ticket_attachments(&attachments)?;
        for (key, value) in &extra {
            reject_extra_frontmatter_key(key)?;
            validate_required_string("extra key", key)?;
            if key == "assignee" {
                agents_registry::validate_assignee_for_project(
                    &self.workspace_root()?,
                    self.name(),
                    value,
                )?;
            }
        }

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let created_at = entry.created_at.clone().unwrap_or_else(|| date.clone());
        let document = TicketJsonDocument {
            schema_version: 1,
            id: id.to_string(),
            lane,
            title,
            summary: spec.summary.clone(),
            stories: spec.stories.clone(),
            risks: spec.risks.clone(),
            progress_record: spec.progress_record.clone(),
            attachments,
            status,
            created_at,
            updated_at: date.clone(),
            extra,
        };
        let spec = document.spec();
        let content = serialize_ticket_json_document(&document)?;
        crate::write_file_atomic(path, &content)?;

        Ok(TicketWriteResult {
            ticket: TicketWriteTicket {
                id: id.to_string(),
                lane: document.lane,
                title: document.title,
                status: document.status,
                created_at: document.created_at,
                updated_at: date,
                file_name: entry.name.clone(),
                path: format!("tickets/{}", entry.name),
                attachments: document.attachments,
                spec: Some(spec),
                extra: document.extra,
            },
            maintenance: self.ticket_maintenance(),
        })
    }

    pub fn deprecate_ticket(
        &self,
        input: DeprecateTicketInput,
    ) -> Result<DeprecateTicketResult, InboxError> {
        let id = validate_ticket_id(&input.id)?;
        let list = self.list_tickets()?;
        let matches: Vec<TicketEntry> = list
            .tickets
            .iter()
            .filter(|entry| entry.id.as_deref() == Some(id))
            .cloned()
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
        let deprecated_dir = self.deprecated_tickets_dir()?;
        let source_path = dir.join(&entry.name);
        let canonical_source = crate::canonicalize(&source_path)?;
        if !canonical_source.starts_with(&dir) {
            return Err(InboxError::InvalidName(entry.name));
        }

        let deprecated_path = deprecated_dir.join(&entry.name);
        if deprecated_path.exists() {
            return Err(InboxError::TicketWriteConflict(format!(
                "deprecated ticket file already exists: tickets/_deprecated/{}",
                entry.name
            )));
        }

        let cleanup = self.resolve_active_ticket_relationships(id, &entry.name)?;

        fs::rename(&canonical_source, &deprecated_path).map_err(|source| InboxError::Io {
            path: deprecated_path.clone(),
            source,
        })?;

        Ok(DeprecateTicketResult {
            ticket: TicketWriteTicket {
                id: id.to_string(),
                lane: entry.lane.unwrap_or_default(),
                title: entry.title.unwrap_or_default(),
                status: entry
                    .status
                    .unwrap_or_else(|| default_ticket_status().to_string()),
                created_at: entry.created_at.unwrap_or_default(),
                updated_at: entry.updated_at.unwrap_or_default(),
                file_name: entry.name.clone(),
                path: format!("tickets/_deprecated/{}", entry.name),
                attachments: entry.attachments,
                spec: entry.spec,
                extra: entry.extra,
            },
            removed_dependency_refs: cleanup.removed_dependency_refs,
            removed_attachment_refs: cleanup.removed_attachment_refs,
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
        if entry.name.ends_with(".json") {
            let mut document =
                parse_ticket_json_document(&original).map_err(InboxError::InvalidInput)?;
            if document.id != id {
                return Err(InboxError::InvalidInput(format!(
                    "ticket JSON id `{}` does not match requested id `{id}`",
                    document.id
                )));
            }
            let date = Utc::now().format("%Y-%m-%d").to_string();
            document.updated_at = date.clone();
            for line in input.progress {
                document.progress_record.push(crate::TicketProgressRecord {
                    at: Some(date.clone()),
                    summary: line.trim().to_string(),
                    evidence: Vec::new(),
                });
            }
            for line in input.record {
                document.progress_record.push(crate::TicketProgressRecord {
                    at: Some(date.clone()),
                    summary: format!("Record: {}", line.trim()),
                    evidence: Vec::new(),
                });
            }
            for line in input.next_step {
                document.progress_record.push(crate::TicketProgressRecord {
                    at: Some(date.clone()),
                    summary: format!("Next step: {}", line.trim()),
                    evidence: Vec::new(),
                });
            }
            let spec = document.spec();
            let content = serialize_ticket_json_document(&document)?;
            crate::write_file_atomic(&canonical, &content)?;

            return Ok(TicketWriteResult {
                ticket: TicketWriteTicket {
                    id: id.to_string(),
                    lane: document.lane,
                    title: document.title,
                    status: document.status,
                    created_at: document.created_at,
                    updated_at: date,
                    file_name: entry.name.clone(),
                    path: format!("tickets/{}", entry.name),
                    attachments: document.attachments.clone(),
                    spec: Some(spec),
                    extra: document.extra,
                },
                maintenance: self.ticket_maintenance(),
            });
        }

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
        let spec = parse_ticket_spec_field(&fields).ok();

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
                attachments: extract_attachments_field(&fields),
                spec,
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
            if !(name.ends_with(".md") || name.ends_with(".json")) {
                continue;
            }
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
                    attachments: entry.attachments.clone(),
                    spec: entry.spec.clone(),
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

    fn resolve_active_ticket_relationships(
        &self,
        deprecated_id: &str,
        deprecated_name: &str,
    ) -> Result<TicketRelationshipCleanup, InboxError> {
        let mut cleanup = TicketRelationshipCleanup::default();
        let dir = self.tickets_dir()?;

        for entry in self.list_tickets()?.tickets {
            if entry.name == deprecated_name || entry.metadata_error.is_some() {
                continue;
            }

            let path = dir.join(&entry.name);
            let canonical = crate::canonicalize(&path)?;
            if !canonical.starts_with(&dir) {
                return Err(InboxError::InvalidName(entry.name));
            }

            let original = fs::read_to_string(&canonical).map_err(|source| InboxError::Io {
                path: canonical.clone(),
                source,
            })?;
            if entry.name.ends_with(".json") {
                let mut document =
                    parse_ticket_json_document(&original).map_err(InboxError::InvalidInput)?;

                let mut changed = false;
                let mut dependency_removed = false;
                for key in ["depends_on", "dependencies"] {
                    if remove_ticket_id_from_relation_field(&mut document.extra, key, deprecated_id)
                    {
                        changed = true;
                        dependency_removed = true;
                    }
                }

                let attachment_removed = remove_ticket_attachment_refs_from_json(
                    &mut document.attachments,
                    deprecated_id,
                );
                changed |= attachment_removed;

                if changed {
                    document.updated_at = Utc::now().format("%Y-%m-%d").to_string();
                    let content = serialize_ticket_json_document(&document)?;
                    crate::write_file_atomic(&canonical, &content)?;

                    if let Some(id) = entry.id.clone() {
                        if dependency_removed {
                            cleanup.removed_dependency_refs.push(id.clone());
                        }
                        if attachment_removed {
                            cleanup.removed_attachment_refs.push(id);
                        }
                    }
                }
                continue;
            }

            let Ok((mut fields, body)) = split_ticket_frontmatter(&original) else {
                continue;
            };

            let mut changed = false;
            let mut dependency_removed = false;
            for key in ["depends_on", "dependencies"] {
                if remove_ticket_id_from_relation_field(&mut fields, key, deprecated_id) {
                    changed = true;
                    dependency_removed = true;
                }
            }

            let attachment_removed = remove_ticket_attachment_refs(&mut fields, deprecated_id)?;
            changed |= attachment_removed;

            if changed {
                fields.insert(
                    "updated_at".to_string(),
                    Utc::now().format("%Y-%m-%d").to_string(),
                );
                let content = render_frontmatter(&fields) + body;
                crate::write_file_atomic(&canonical, &content)?;

                if let Some(id) = entry.id.clone() {
                    if dependency_removed {
                        cleanup.removed_dependency_refs.push(id.clone());
                    }
                    if attachment_removed {
                        cleanup.removed_attachment_refs.push(id);
                    }
                }
            }
        }

        Ok(cleanup)
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

#[derive(Debug, Default)]
struct TicketRelationshipCleanup {
    removed_dependency_refs: Vec<String>,
    removed_attachment_refs: Vec<String>,
}

fn remove_ticket_id_from_relation_field(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    deprecated_id: &str,
) -> bool {
    let Some(value) = fields.get(key).cloned() else {
        return false;
    };

    let tokens: Vec<String> = value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect();
    if !tokens.iter().any(|token| token == deprecated_id) {
        return false;
    }

    let mut kept = Vec::new();
    for token in tokens {
        if token != deprecated_id && !kept.iter().any(|item| item == &token) {
            kept.push(token);
        }
    }
    if kept.is_empty() {
        fields.remove(key);
    } else {
        fields.insert(key.to_string(), kept.join(" "));
    }
    true
}

fn remove_ticket_attachment_refs(
    fields: &mut BTreeMap<String, String>,
    deprecated_id: &str,
) -> Result<bool, InboxError> {
    let Some(value) = fields.get("attachments").cloned() else {
        return Ok(false);
    };

    let Ok(mut attachments) = serde_json::from_str::<Vec<serde_json::Value>>(&value) else {
        return Ok(false);
    };
    let original_len = attachments.len();
    attachments.retain(|attachment| {
        let kind = attachment
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let target = attachment
            .get("target")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        !(kind.eq_ignore_ascii_case("ticket") && target == deprecated_id)
    });

    if attachments.len() == original_len {
        return Ok(false);
    }
    if attachments.is_empty() {
        fields.remove("attachments");
    } else {
        let serialized = serde_json::to_string(&attachments).map_err(|err| {
            InboxError::InvalidInput(format!("attachments could not be serialized: {err}"))
        })?;
        fields.insert("attachments".to_string(), serialized);
    }
    Ok(true)
}

fn remove_ticket_attachment_refs_from_json(
    attachments: &mut Vec<crate::TicketAttachment>,
    deprecated_id: &str,
) -> bool {
    let original_len = attachments.len();
    attachments.retain(|attachment| {
        !(attachment.kind.eq_ignore_ascii_case("ticket") && attachment.target == deprecated_id)
    });
    attachments.len() != original_len
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
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name {
        return Err(InboxError::InvalidName(name.to_string()));
    }
    if !(trimmed.ends_with(".md") || trimmed.ends_with(".json")) {
        return Err(InboxError::InvalidName(name.to_string()));
    }

    let path = Path::new(trimmed);
    if path.is_absolute() || path.components().count() != 1 {
        return Err(InboxError::InvalidName(name.to_string()));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) | Component::CurDir
        )
    }) {
        return Err(InboxError::InvalidName(name.to_string()));
    }

    let validated = trimmed.to_string();
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
