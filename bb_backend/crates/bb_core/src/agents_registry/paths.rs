use std::path::{Path, PathBuf};

use crate::InboxError;

#[must_use]
pub fn registry_path(bb_root: &Path) -> PathBuf {
    bb_root.join("agents").join("agents.toml")
}

#[allow(clippy::option_if_let_else)]
pub(super) fn path_for_display(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = value.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        value.into_owned()
    }
}

pub fn resolve_source_path(bb_root: &Path, value: &str) -> Result<PathBuf, InboxError> {
    let kb_root = bb_root.parent().ok_or_else(|| {
        InboxError::InvalidInput("cannot resolve <kb-root> from Blackboard root".to_string())
    })?;
    if let Some(rest) = value.strip_prefix("<bb-root>/") {
        Ok(join_template_suffix(bb_root.to_path_buf(), rest))
    } else if let Some(rest) = value.strip_prefix("<kb-root>/") {
        Ok(join_template_suffix(kb_root.to_path_buf(), rest))
    } else if let Some(rest) = value.strip_prefix("~/") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| InboxError::InvalidInput("HOME is not set".to_string()))?;
        Ok(join_template_suffix(home, rest))
    } else if Path::new(value).is_absolute() {
        Ok(PathBuf::from(value))
    } else {
        Err(InboxError::InvalidInput(format!(
            "agent source_path {value:?} must be absolute or start with <bb-root>/, <kb-root>/, or ~/"
        )))
    }
}

fn join_template_suffix(mut base: PathBuf, rest: &str) -> PathBuf {
    for part in rest.split('/') {
        if !part.is_empty() {
            base.push(part);
        }
    }
    base
}
