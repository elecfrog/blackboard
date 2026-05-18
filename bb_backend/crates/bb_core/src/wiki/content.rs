use std::fs;

use crate::{Blackboard, InboxError};

use super::extensions::sniff_content_type;
use super::model::{WikiAsset, WikiContentResponse, WikiTreeResponse};
use super::tree::read_wiki_dir_recursive;
use super::validation::ValidateKind;

impl Blackboard {
    /// List the full wiki tree relative to `projects/<project>/wiki/`.
    /// When the directory does not exist, returns an empty tree rather than
    /// an error — this matches how `tickets/` / `inbox/` treat an empty
    /// project in the rest of the codebase.
    pub fn list_wiki_tree(&self) -> Result<WikiTreeResponse, InboxError> {
        let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let wiki_root = match self.wiki_root() {
            Some(root) => root,
            None => {
                return Ok(WikiTreeResponse {
                    generated_at,
                    tree: Vec::new(),
                });
            }
        };

        let tree = read_wiki_dir_recursive(&wiki_root, "")?;

        Ok(WikiTreeResponse { generated_at, tree })
    }

    /// Read one text document from the wiki. Accepts any extension in the
    /// broad `WIKI_DOC_EXTENSIONS` set (Markdown, config, source code,
    /// SVG/XML/JSON, ...). The returned `content_type` is the lowercased
    /// extension, used by the frontend to pick a renderer.
    pub fn read_wiki_content(&self, file_path: &str) -> Result<WikiContentResponse, InboxError> {
        let safe_path = self.validate_wiki_path(file_path, ValidateKind::Document)?;

        if !safe_path.is_file() {
            return Err(InboxError::NotFound(file_path.to_string()));
        }

        let content = fs::read_to_string(&safe_path).map_err(|source| InboxError::Io {
            path: safe_path.clone(),
            source,
        })?;

        let content_type = safe_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();

        Ok(WikiContentResponse {
            content,
            content_type,
        })
    }

    /// Read a raw relative asset (images, svg, pdf, ...) from the wiki.
    /// Path is validated against the same escape rules as documents; the
    /// allowed extension set is broader but still bounded.
    pub fn read_wiki_asset(&self, file_path: &str) -> Result<WikiAsset, InboxError> {
        let safe_path = self.validate_wiki_path(file_path, ValidateKind::Asset)?;

        if !safe_path.is_file() {
            return Err(InboxError::NotFound(file_path.to_string()));
        }

        let bytes = fs::read(&safe_path).map_err(|source| InboxError::Io {
            path: safe_path.clone(),
            source,
        })?;
        let content_type = sniff_content_type(&safe_path);

        Ok(WikiAsset {
            bytes,
            content_type,
        })
    }
}
