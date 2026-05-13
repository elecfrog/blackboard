//! 文件系统和 JSON 工具函数。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::InboxError;

/// 从 JSON 文件反序列化。
pub fn read_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, InboxError> {
    let content = fs::read_to_string(path).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|err| InboxError::InvalidProjectMeta {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

/// 将值序列化为格式化 JSON 并写入文件。
pub fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), InboxError> {
    let rendered =
        serde_json::to_string_pretty(value).map_err(|err| InboxError::InvalidProjectMeta {
            path: path.to_path_buf(),
            message: format!("failed to serialize JSON: {err}"),
        })?;
    write_file_atomic(path, &rendered)
}

/// 原子写入文件（先写临时文件再 rename）。
pub fn write_file_atomic(path: &Path, content: &str) -> Result<(), InboxError> {
    let parent = path
        .parent()
        .ok_or_else(|| InboxError::InvalidName(path.display().to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| InboxError::InvalidName(path.display().to_string()))?;
    let tmp = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    fs::write(&tmp, content).map_err(|source| InboxError::Io {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, path).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// 规范化路径。
pub fn canonicalize(path: &Path) -> Result<std::path::PathBuf, InboxError> {
    path.canonicalize()
        .map(clean_path_buf)
        .map_err(|source| InboxError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// 规范化路径并验证其为目录。
pub fn canonicalize_existing_dir(path: &Path) -> Result<std::path::PathBuf, std::io::Error> {
    let canonical = clean_path_buf(path.canonicalize()?);
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not a directory",
        ))
    }
}

/// Convert a path to a stable user/config-facing string.
///
/// Windows `std::fs::canonicalize` can produce extended-length paths such as
/// `\\?\C:\...`. Those are useful for low-level Win32 APIs, but they should not
/// leak into portable config files, API responses, or UI state.
pub fn path_to_string(path: &Path) -> String {
    clean_path_string(&path.to_string_lossy())
}

pub fn clean_path_string(value: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            return rest.to_string();
        }
    }
    value.to_string()
}

fn clean_path_buf(path: PathBuf) -> PathBuf {
    PathBuf::from(clean_path_string(&path.to_string_lossy()))
}

/// 将字符串转为 URL-safe slug 片段。
pub fn slug_segment(value: &str, fallback: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.trim().chars() {
        if ch.is_alphanumeric() {
            for lowered in ch.to_lowercase() {
                slug.push(lowered);
            }
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}
