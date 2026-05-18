use std::fs;
use std::path::Path;

use crate::InboxError;

use super::model::{WikiNodeKind, WikiTreeNode};

pub(super) fn read_wiki_dir_recursive(
    current: &Path,
    rel_path: &str,
) -> Result<Vec<WikiTreeNode>, InboxError> {
    let entries = fs::read_dir(current).map_err(|source| InboxError::Io {
        path: current.to_path_buf(),
        source,
    })?;

    let mut dir_nodes: Vec<WikiTreeNode> = Vec::new();
    let mut file_nodes: Vec<WikiTreeNode> = Vec::new();

    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dotfiles (including `.gitkeep`), OS noise, and the
        // swap/backup files editors love to drop next to Markdown docs.
        if name.starts_with('.') {
            continue;
        }

        let file_type = entry.file_type().map_err(|source| InboxError::Io {
            path: entry.path(),
            source,
        })?;

        // Only traverse real dirs / files; skip symlinks to avoid escaping.
        if file_type.is_symlink() {
            continue;
        }

        let entry_rel = if rel_path.is_empty() {
            name.clone()
        } else {
            format!("{rel_path}/{name}")
        };

        if file_type.is_dir() {
            let children = read_wiki_dir_recursive(&entry.path(), &entry_rel)?;
            dir_nodes.push(WikiTreeNode {
                name,
                path: entry_rel,
                kind: WikiNodeKind::Dir,
                children: Some(children),
            });
        } else if file_type.is_file() {
            file_nodes.push(WikiTreeNode {
                name,
                path: entry_rel,
                kind: WikiNodeKind::File,
                children: None,
            });
        }
    }

    dir_nodes.sort_by(|a, b| a.name.cmp(&b.name));
    file_nodes.sort_by(|a, b| a.name.cmp(&b.name));

    // Dirs first, then files — matches common wiki/tree UIs.
    let mut nodes = dir_nodes;
    nodes.extend(file_nodes);
    Ok(nodes)
}
