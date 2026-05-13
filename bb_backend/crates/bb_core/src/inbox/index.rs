use std::fs;

use chrono::SecondsFormat;

use crate::{Blackboard, InboxError, InboxIndex, InboxIndexEntry};

impl Blackboard {
    /// Rebuild the persistent inbox index (`__inbox__.json`).
    pub fn rebuild_inbox_index(&self) -> Result<(), InboxError> {
        let mut notes = Vec::new();
        let entries = fs::read_dir(self.inbox()).map_err(|source| InboxError::Io {
            path: self.inbox().to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| InboxError::Io {
                path: self.inbox().to_path_buf(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            let path = entry.path();
            let meta = fs::metadata(&path).map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;
            if !meta.is_file() {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;
            let excerpt = build_inbox_excerpt(&content);
            let modified_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            notes.push(InboxIndexEntry {
                name,
                size: meta.len(),
                modified_at,
                excerpt,
            });
        }
        notes.sort_by(|a, b| a.name.cmp(&b.name));
        let index = InboxIndex { notes };
        let json = serde_json::to_string_pretty(&index).map_err(|err| {
            InboxError::InvalidInput(format!("failed to serialize inbox index: {err}"))
        })?;
        let index_path = self.root().join("__inbox__.json");
        let tmp_path = self
            .root()
            .join(format!(".__inbox__.json.{}.tmp", std::process::id()));
        fs::write(&tmp_path, json.as_bytes()).map_err(|source| InboxError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        fs::rename(&tmp_path, &index_path).map_err(|source| InboxError::Io {
            path: index_path,
            source,
        })?;
        Ok(())
    }

    /// Read the persisted inbox index. Rebuilds if missing.
    pub fn read_inbox_index(&self) -> Result<InboxIndex, InboxError> {
        let index_path = self.root().join("__inbox__.json");
        match fs::read_to_string(&index_path) {
            Ok(content) => serde_json::from_str(&content).or_else(|_| {
                self.rebuild_inbox_index()?;
                let fresh = fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                    path: index_path.clone(),
                    source,
                })?;
                serde_json::from_str(&fresh).map_err(|err| {
                    InboxError::InvalidInput(format!("__inbox__.json invalid: {err}"))
                })
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.rebuild_inbox_index()?;
                let content = fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                    path: index_path,
                    source,
                })?;
                serde_json::from_str(&content).map_err(|err| {
                    InboxError::InvalidInput(format!("__inbox__.json invalid: {err}"))
                })
            }
            Err(source) => Err(InboxError::Io {
                path: index_path,
                source,
            }),
        }
    }
}

fn build_inbox_excerpt(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let clean = trimmed
            .trim_start_matches(['-', '*', ' '])
            .replace('`', "")
            .replace("**", "");
        if clean.chars().count() <= 140 {
            return clean;
        }
        let excerpt: String = clean.chars().take(140).collect();
        return format!("{excerpt}...");
    }
    String::new()
}
