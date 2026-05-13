//! Blackboard (ProjectBoard) — 单项目数据句柄。
//!
//! 每个 `Blackboard` 实例代表一个项目的 inbox/ticket CRUD 入口。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::InboxError;
use crate::fs_util::{canonicalize, canonicalize_existing_dir};
use crate::validate_project_name;

/// Per-project handle carrying the inbox/ticket CRUD surface.
#[derive(Debug, Clone)]
pub struct Blackboard {
    pub(crate) name: String,
    pub(crate) root: PathBuf,
    pub(crate) inbox: PathBuf,
    pub(crate) workspace_root: Option<PathBuf>,
}

/// Readable alias preferred in call sites that already think in terms of a
/// single project.
pub type ProjectBoard = Blackboard;

impl Blackboard {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, InboxError> {
        let root_ref = root.as_ref();
        let raw_name = root_ref
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let name = if validate_project_name(raw_name).is_ok() {
            raw_name.to_string()
        } else {
            "kb".to_string()
        };
        Self::open_with_name(root_ref, &name)
    }

    pub(crate) fn open_with_name(root: impl AsRef<Path>, name: &str) -> Result<Self, InboxError> {
        let name = validate_project_name(name)?.to_string();
        let root = canonicalize(root.as_ref())?;
        let inbox = root.join("inbox");
        // Auto-create inbox directory if missing (e.g. legacy or newly created project)
        if !inbox.exists() {
            fs::create_dir_all(&inbox).map_err(|e| InboxError::Io {
                path: inbox.clone(),
                source: e,
            })?;
        }
        let inbox = canonicalize_existing_dir(&inbox)
            .map_err(|_| InboxError::MissingInbox(root.clone()))?;

        if !inbox.starts_with(&root) {
            return Err(InboxError::MissingInbox(root));
        }

        let board = Self {
            name,
            root,
            inbox,
            workspace_root: None,
        };
        board.ensure_indexes();
        Ok(board)
    }

    /// Ensure derived JSON indexes are present.
    fn ensure_indexes(&self) {
        let _ = self.read_ticket_index();
        if !self.root.join("__inbox__.json").exists() {
            let _ = self.rebuild_inbox_index();
        }
        if !self.root.join("__project__.json").exists() {
            let _ = self.rebuild_project_index();
        }
    }

    pub fn discover() -> Result<Self, InboxError> {
        let cwd = std::env::current_dir().map_err(|source| InboxError::Io {
            path: PathBuf::from("."),
            source,
        })?;
        Self::discover_from(cwd)
    }

    pub fn discover_from(start: impl AsRef<Path>) -> Result<Self, InboxError> {
        let start = canonicalize(start.as_ref())?;

        for ancestor in start.ancestors() {
            let inbox = ancestor.join("inbox");
            if inbox.is_dir() {
                return Self::open(ancestor);
            }
        }

        Err(InboxError::RootNotFound(start))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inbox(&self) -> &Path {
        &self.inbox
    }

    pub fn workspace_root(&self) -> Result<PathBuf, InboxError> {
        if let Some(root) = &self.workspace_root {
            return Ok(root.clone());
        }
        self.root
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                InboxError::InvalidInput(format!(
                    "cannot resolve Blackboard workspace root from {}",
                    self.root.display()
                ))
            })
    }
}
