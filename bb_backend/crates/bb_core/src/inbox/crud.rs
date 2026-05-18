use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::{
    ArchivedInboxNote, Blackboard, CreatedInboxNote, DeletedInboxNote, InboxError, InboxNote,
    InboxNoteEntry, InboxNoteInput,
};

use super::input::validate_input;
use super::render::{
    document_from_input, inbox_excerpt, parse_inbox_json_document, render_note,
    serialize_inbox_json_document,
};

impl Blackboard {
    pub fn list_notes(&self) -> Result<Vec<InboxNoteEntry>, InboxError> {
        let mut notes = Vec::new();

        for entry in fs::read_dir(self.inbox()).map_err(|source| InboxError::Io {
            path: self.inbox().to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| InboxError::Io {
                path: self.inbox().to_path_buf(),
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
            if !crate::is_valid_note_name(&name) {
                continue;
            }
            let path = entry.path();
            let content = fs::read_to_string(&path).map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;
            let Ok(document) = parse_inbox_json_document(&content) else {
                continue;
            };
            let meta = fs::metadata(&path).map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;
            notes.push(InboxNoteEntry {
                name,
                size: meta.len(),
                modified_at: super::index::modified_at_rfc3339(&meta),
                excerpt: inbox_excerpt(&document),
                title: document.title,
                time: document.time,
                source: document.source,
                topic: document.topic,
            });
        }

        notes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(notes)
    }

    pub fn read_note(&self, name: &str) -> Result<InboxNote, InboxError> {
        let name = crate::validate_note_name(name)?;
        let path = self.inbox().join(&name);

        if !path.starts_with(self.inbox()) {
            return Err(InboxError::InvalidName(name));
        }

        let canonical = match crate::canonicalize(&path) {
            Ok(path) => path,
            Err(InboxError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(InboxError::NotFound(name));
            }
            Err(err) => return Err(err),
        };

        if !canonical.starts_with(self.inbox()) {
            return Err(InboxError::InvalidName(name));
        }

        let content = fs::read_to_string(&canonical).map_err(|source| InboxError::Io {
            path: canonical,
            source,
        })?;
        let document = parse_inbox_json_document(&content).map_err(InboxError::InvalidInput)?;

        Ok(InboxNote {
            name,
            content: render_note(&document),
            document,
        })
    }

    pub fn create_note(&self, input: InboxNoteInput) -> Result<CreatedInboxNote, InboxError> {
        validate_input(&input)?;

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let source = crate::slug_segment(&input.source, "unknown");
        let topic = crate::slug_segment(&input.topic, "note");
        let base = format!("{date}-{source}-{topic}");
        let document = document_from_input(input)?;
        let content = serialize_inbox_json_document(&document)?;

        for attempt in 0..1000 {
            let name = if attempt == 0 {
                format!("{base}.json")
            } else {
                format!("{base}-{attempt}.json")
            };
            crate::validate_note_name(&name)?;

            let path = self.inbox().join(&name);
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(InboxError::Io { path, source }),
            };

            file.write_all(content.as_bytes())
                .map_err(|source| InboxError::Io {
                    path: path.clone(),
                    source,
                })?;

            let _ = self.rebuild_inbox_index();

            return Ok(CreatedInboxNote {
                name: name.clone(),
                path: format!("inbox/{name}"),
            });
        }

        Err(InboxError::InvalidInput(
            "could not allocate a collision-free inbox filename".to_string(),
        ))
    }

    pub fn delete_note(&self, name: &str) -> Result<DeletedInboxNote, InboxError> {
        let name = crate::validate_note_name(name)?;
        let path = self.inbox().join(&name);

        if !path.starts_with(self.inbox()) {
            return Err(InboxError::InvalidName(name));
        }

        let canonical = match crate::canonicalize(&path) {
            Ok(path) => path,
            Err(InboxError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(InboxError::NotFound(name));
            }
            Err(err) => return Err(err),
        };

        if !canonical.starts_with(self.inbox()) || !canonical.is_file() {
            return Err(InboxError::InvalidName(name));
        }

        fs::remove_file(&canonical).map_err(|source| InboxError::Io {
            path: canonical,
            source,
        })?;
        let _ = self.rebuild_inbox_index();

        Ok(DeletedInboxNote {
            name: name.clone(),
            path: format!("inbox/{name}"),
        })
    }

    pub fn archive_note(&self, name: &str) -> Result<ArchivedInboxNote, InboxError> {
        let name = crate::validate_note_name(name)?;
        let path = self.inbox().join(&name);

        if !path.starts_with(self.inbox()) {
            return Err(InboxError::InvalidName(name));
        }

        let canonical = match crate::canonicalize(&path) {
            Ok(path) => path,
            Err(InboxError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(InboxError::NotFound(name));
            }
            Err(err) => return Err(err),
        };

        if !canonical.starts_with(self.inbox()) || !canonical.is_file() {
            return Err(InboxError::InvalidName(name));
        }

        let archive_dir = self.inbox().join("archive");
        fs::create_dir_all(&archive_dir).map_err(|source| InboxError::Io {
            path: archive_dir.clone(),
            source,
        })?;
        let archived_name = allocate_archived_note_name(&archive_dir, &name)?;
        let archived_path = archive_dir.join(&archived_name);
        fs::rename(&canonical, &archived_path).map_err(|source| InboxError::Io {
            path: archived_path.clone(),
            source,
        })?;
        let _ = self.rebuild_inbox_index();

        Ok(ArchivedInboxNote {
            name: name.clone(),
            original_path: format!("inbox/{name}"),
            archived_name: archived_name.clone(),
            archived_path: format!("inbox/archive/{archived_name}"),
        })
    }
}

fn allocate_archived_note_name(archive_dir: &Path, name: &str) -> Result<String, InboxError> {
    if !archive_dir.join(name).exists() {
        return Ok(name.to_string());
    }

    let stem = name
        .strip_suffix(".json")
        .ok_or_else(|| InboxError::InvalidName(name.to_string()))?;
    for attempt in 1..1000 {
        let candidate = format!("{stem}-{attempt}.json");
        crate::validate_note_name(&candidate)?;
        if !archive_dir.join(&candidate).exists() {
            return Ok(candidate);
        }
    }

    Err(InboxError::InvalidInput(
        "could not allocate a collision-free archived inbox filename".to_string(),
    ))
}
