use std::path::{Component, Path, PathBuf};

use crate::{Blackboard, InboxError};

use super::extensions::{WIKI_BINARY_EXTENSIONS, WIKI_DOC_EXTENSIONS, WIKI_UPLOAD_EXTENSIONS};

impl Blackboard {
    /// Public-facing path validation used by the upload endpoint in
    /// `bb_server`. Uses the upload-specific whitelist (union of document
    /// and binary extensions, minus anything explicitly excluded) so the
    /// set of files on disk is always a superset of what read endpoints
    /// can serve. Callers must `ensure_wiki_root()` first when the wiki
    /// directory might not exist yet.
    pub fn validate_wiki_path_for_upload(&self, file_path: &str) -> Result<PathBuf, InboxError> {
        self.validate_wiki_path(file_path, ValidateKind::Upload)
    }

    /// Resolve a user-supplied wiki-relative path into an absolute path on
    /// disk, rejecting anything that is absolute, contains `..`, empty
    /// segments, Windows drive letters, or escapes the canonical wiki root
    /// via symlinks. `kind` chooses the extension whitelist.
    pub(crate) fn validate_wiki_path(
        &self,
        requested: &str,
        kind: ValidateKind,
    ) -> Result<PathBuf, InboxError> {
        if requested.is_empty() {
            return Err(InboxError::InvalidInput(
                "wiki path must not be empty".to_string(),
            ));
        }
        if requested.len() > 512 {
            return Err(InboxError::InvalidInput("wiki path too long".to_string()));
        }

        // Normalize to forward slashes and sanity-check characters before
        // we hand anything to the OS path APIs.
        let normalized = requested.replace('\\', "/");
        if normalized.starts_with('/') {
            return Err(InboxError::InvalidInput(
                "wiki path must be relative".to_string(),
            ));
        }
        // Windows drive letter like `C:` or `c:/...`.
        if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
            return Err(InboxError::InvalidInput(
                "wiki path must not include a drive letter".to_string(),
            ));
        }
        for segment in normalized.split('/') {
            if segment.is_empty() {
                return Err(InboxError::InvalidInput(
                    "wiki path must not contain empty segments".to_string(),
                ));
            }
            if segment == ".." {
                return Err(InboxError::InvalidInput(
                    "wiki path must not contain parent references".to_string(),
                ));
            }
        }

        let requested_path = Path::new(&normalized);
        if requested_path.is_absolute() {
            return Err(InboxError::InvalidInput(
                "wiki path must be relative".to_string(),
            ));
        }
        for component in requested_path.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(InboxError::InvalidInput(
                        "wiki path escapes wiki directory".to_string(),
                    ));
                }
            }
        }

        // Extension whitelist per mode.
        let ext = requested_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let allowed: &[&str] = match kind {
            ValidateKind::Document => WIKI_DOC_EXTENSIONS,
            ValidateKind::Asset => WIKI_BINARY_EXTENSIONS,
            ValidateKind::Upload => WIKI_UPLOAD_EXTENSIONS,
        };
        if !allowed.iter().any(|a| *a == ext) {
            return Err(InboxError::InvalidInput(format!(
                "wiki path extension '{}' not allowed here",
                if ext.is_empty() {
                    "<none>".to_string()
                } else {
                    ext
                }
            )));
        }

        // Build the absolute target and make sure it lives inside the
        // canonical wiki root. We canonicalize the root (always exists at
        // this point) and then canonicalize the *closest existing
        // ancestor* of the target — this is symlink-safe and works for
        // uploads whose intermediate directories don't yet exist.
        let wiki_root = self
            .wiki_root()
            .ok_or_else(|| InboxError::NotFound("wiki directory not found".to_string()))?;
        let canonical_root = crate::canonicalize(&wiki_root)?;
        let target = wiki_root.join(requested_path);

        if target.exists() {
            let canonical_target = crate::canonicalize(&target)?;
            if !canonical_target.starts_with(&canonical_root) {
                return Err(InboxError::InvalidInput(
                    "wiki path escapes wiki directory".to_string(),
                ));
            }
            return Ok(canonical_target);
        }

        // Walk up until we find an ancestor that does exist. This loop is
        // bounded by the (already-validated) path depth and always
        // terminates at `wiki_root`, which we just canonicalized above.
        let mut probe = target.clone();
        let canonical_anchor = loop {
            match probe.parent() {
                Some(parent) => {
                    if parent.exists() {
                        break crate::canonicalize(parent)?;
                    }
                    probe = parent.to_path_buf();
                }
                None => break canonical_root.clone(),
            }
        };

        if !canonical_anchor.starts_with(&canonical_root) {
            return Err(InboxError::InvalidInput(
                "wiki path escapes wiki directory".to_string(),
            ));
        }

        Ok(target)
    }
}

/// Whether a path is being validated for reading a document (any of the
/// text-y extensions), reading a binary asset (images/pdf/svg-as-image),
/// or accepting an upload (the union minus anything explicitly excluded).
#[derive(Debug, Clone, Copy)]
pub enum ValidateKind {
    Document,
    Asset,
    Upload,
}
