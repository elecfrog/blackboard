use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::InboxError;

/// Compute `<bb-root>/agents/AGENTS.md` from a workspace root.
pub fn source_path(bb_root: &Path) -> PathBuf {
    bb_root.join("agents").join("AGENTS.md")
}

/// Resolve the home directory used to expand `~/...`. Production code reads
/// `$HOME`; tests inject a sandbox path via the `home_override` argument so
/// they never have to mutate process-global env vars.
fn resolve_home(home_override: Option<&Path>) -> Result<PathBuf, InboxError> {
    if let Some(path) = home_override {
        return Ok(path.to_path_buf());
    }
    resolve_home_from_env_vars(
        std::env::var_os("HOME"),
        std::env::var_os("USERPROFILE"),
        std::env::var_os("HOMEDRIVE"),
        std::env::var_os("HOMEPATH"),
    )
    .ok_or_else(|| {
        InboxError::InvalidInput(
            "cannot expand `~` in connector target: HOME/USERPROFILE is not set".to_string(),
        )
    })
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn resolve_home_from_env_vars(
    home: Option<OsString>,
    userprofile: Option<OsString>,
    homedrive: Option<OsString>,
    homepath: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(path) = non_empty_path(home) {
        return Some(path);
    }
    if let Some(path) = non_empty_path(userprofile) {
        return Some(path);
    }

    let drive = homedrive?;
    let path = homepath?;
    if drive.is_empty() || path.is_empty() {
        return None;
    }
    let mut combined = OsString::new();
    combined.push(drive);
    combined.push(path);
    Some(PathBuf::from(combined))
}

fn join_template_suffix(mut base: PathBuf, rest: &str) -> PathBuf {
    for part in rest.split('/') {
        if !part.is_empty() {
            base.push(part);
        }
    }
    base
}

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

/// Expand a `target_template` into an absolute path.
///
/// Supports:
/// - `~/...` → expands via `$HOME`
/// - `<bb-root>/...` → expands relative to the workspace root
/// - absolute path → used as-is
pub(super) fn expand_target(
    template: &str,
    home_override: Option<&Path>,
    bb_root: &Path,
) -> Result<PathBuf, InboxError> {
    if let Some(rest) = template.strip_prefix("~/") {
        Ok(join_template_suffix(resolve_home(home_override)?, rest))
    } else if template == "~" {
        resolve_home(home_override)
    } else if let Some(rest) = template.strip_prefix("<bb-root>/") {
        Ok(join_template_suffix(bb_root.to_path_buf(), rest))
    } else if Path::new(template).is_absolute() {
        Ok(PathBuf::from(template))
    } else {
        Err(InboxError::InvalidInput(format!(
            "connector target {template:?} must be absolute or start with `~/` or `<bb-root>/`"
        )))
    }
}
