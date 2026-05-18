use std::fs;
use std::path::PathBuf;

use crate::{Blackboard, InboxError};

impl Blackboard {
    /// Absolute path to `projects/<project>/wiki/`, whether or not it exists.
    /// Distinct from `wiki_root_if_present`: callers that want to *create*
    /// the directory (e.g. the upload endpoint) need the path even when the
    /// directory is missing.
    #[must_use]
    pub fn wiki_root_path(&self) -> PathBuf {
        self.root.join("wiki")
    }

    /// Absolute path to `projects/<project>/wiki/` when the directory
    /// currently exists on disk; otherwise `None`. Kept for read-only
    /// callers that treat a missing directory as an empty wiki.
    #[must_use]
    pub fn wiki_root(&self) -> Option<PathBuf> {
        let wiki = self.wiki_root_path();
        if wiki.is_dir() {
            Some(wiki)
        } else {
            None
        }
    }

    /// Ensure `projects/<project>/wiki/` exists, creating it (and any missing
    /// parents) if necessary. Returns the canonical path on success.
    pub fn ensure_wiki_root(&self) -> Result<PathBuf, InboxError> {
        let wiki = self.wiki_root_path();
        if !wiki.exists() {
            fs::create_dir_all(&wiki).map_err(|source| InboxError::Io {
                path: wiki.clone(),
                source,
            })?;
        }
        Ok(wiki)
    }
}
