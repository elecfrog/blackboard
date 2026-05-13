use std::fs::{self, OpenOptions};
use std::io::Write;

use crate::{
    Blackboard, CreatedInboxNote, DeletedInboxNote, InboxError, InboxNote, InboxNoteEntry,
    InboxNoteInput,
};

use super::input::validate_input;
use super::render::render_note;

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
            if crate::is_valid_note_name(&name) {
                notes.push(InboxNoteEntry { name });
            }
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

        Ok(InboxNote { name, content })
    }

    pub fn create_note(&self, input: InboxNoteInput) -> Result<CreatedInboxNote, InboxError> {
        validate_input(&input)?;

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let source = crate::slug_segment(&input.source, "unknown");
        let topic = crate::slug_segment(&input.topic, "note");
        let base = format!("{date}-{source}-{topic}");
        let content = render_note(&input);

        for attempt in 0..1000 {
            let name = if attempt == 0 {
                format!("{base}.md")
            } else {
                format!("{base}-{attempt}.md")
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
}
