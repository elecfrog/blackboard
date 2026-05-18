//! Agent connector MCP server config injection.
//!
//! Supports three user-level configuration formats:
//!
//! - Codex CLI: `~/.codex/config.toml`   (TOML, `[mcp_servers.<id>]` subtable)
//! - `CodeBuddy`: `~/.codebuddy/mcp.json`  (JSON, `mcpServers.<id>` key)
//! - `OpenCode`:  `~/.config/opencode/opencode.json`  (JSON, `mcp.<id>` key)
//!
//! All three are edited in place: other keys, tools, permissions, commands,
//! plugins etc. are preserved verbatim so that inserting Blackboard's `bb`
//! MCP server never disturbs the user's existing Agent configuration.
//!
//! This is only the on-disk Agent config editor. Blackboard's own MCP server
//! protocol surface lives in `bb_daemon`, not here.

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::fs;
use std::path::Path;
use toml_edit::{value, DocumentMut, Item, Table};

use crate::InboxError;

/// Which on-disk MCP config format a connector target writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMcpConfigFormat {
    /// Codex `~/.codex/config.toml` — TOML with a `[mcp_servers.<id>]` subtable.
    CodexToml,
    /// `CodeBuddy` `~/.codebuddy/mcp.json` — JSON with top-level `mcpServers`.
    CodebuddyJson,
    /// `OpenCode` `~/.config/opencode/opencode.json` — JSON with top-level `mcp`.
    OpencodeJson,
}

/// Canonical description of one MCP server entry as it should appear in the
/// target config. Transport shape is explicit because the three formats
/// express "remote HTTP" with different keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct AgentMcpServerConfig {
    /// Logical id (e.g. `"bb"`). Used as the JSON/TOML key.
    pub id: String,
    /// Human-facing description; not written to the target file.
    pub description: String,
    /// Transport definition.
    pub transport: AgentMcpTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum AgentMcpTransport {
    /// Remote HTTP endpoint (e.g. bb-server's `/mcp`).
    RemoteHttp { url: String },
}

/// Result of inspecting a target config file for a given server id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AgentMcpServerTarget {
    /// Target file's parent directory does not exist (Agent not installed).
    Unreachable,
    /// Parent dir is OK but the `<id>` key is not present.
    Missing,
    /// `<id>` key exists and matches the expected config exactly.
    Synced,
    /// `<id>` key exists but drifts from the expected config (user edited, or
    /// bb-server endpoint moved).
    Drift { actual_summary: String },
}

/// Inspect the current on-disk state of the `<id>` entry.
pub(super) fn inspect_server(
    format: AgentMcpConfigFormat,
    target_path: &Path,
    desired: &AgentMcpServerConfig,
) -> Result<AgentMcpServerTarget, InboxError> {
    let parent_exists = target_path.parent().is_some_and(std::path::Path::is_dir);

    let content = match fs::read_to_string(target_path) {
        Ok(content) => Some(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(InboxError::Io {
                path: target_path.to_path_buf(),
                source,
            });
        }
    };

    let Some(content) = content else {
        if parent_exists {
            return Ok(AgentMcpServerTarget::Missing);
        }
        return Ok(AgentMcpServerTarget::Unreachable);
    };

    match format {
        AgentMcpConfigFormat::CodexToml => inspect_codex_toml(&content, desired),
        AgentMcpConfigFormat::CodebuddyJson => {
            inspect_json(&content, "mcpServers", desired, codebuddy_expected)
        }
        AgentMcpConfigFormat::OpencodeJson => {
            inspect_json(&content, "mcp", desired, opencode_expected)
        }
    }
}

/// Upsert the `<id>` entry into the target file. Creates parent dir and an
/// empty skeleton if the file does not exist yet. Other keys are preserved
/// verbatim.
pub(super) fn upsert_server(
    format: AgentMcpConfigFormat,
    target_path: &Path,
    desired: &AgentMcpServerConfig,
) -> Result<(), InboxError> {
    if let Some(parent) = target_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| InboxError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }

    let current = match fs::read_to_string(target_path) {
        Ok(content) => Some(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(InboxError::Io {
                path: target_path.to_path_buf(),
                source,
            });
        }
    };

    let updated = match format {
        AgentMcpConfigFormat::CodexToml => upsert_codex_toml(current.as_deref(), desired)?,
        AgentMcpConfigFormat::CodebuddyJson => upsert_json(
            current.as_deref(),
            "mcpServers",
            desired,
            codebuddy_expected,
        )?,
        AgentMcpConfigFormat::OpencodeJson => {
            upsert_json(current.as_deref(), "mcp", desired, opencode_expected)?
        }
    };

    fs::write(target_path, updated).map_err(|source| InboxError::Io {
        path: target_path.to_path_buf(),
        source,
    })
}

/// Remove the `<id>` entry from the target file. Leaves the file itself and
/// every other key in place. Silently succeeds when the file or the key do
/// not exist (disconnect is idempotent).
pub(super) fn remove_server(
    format: AgentMcpConfigFormat,
    target_path: &Path,
    id: &str,
) -> Result<(), InboxError> {
    let current = match fs::read_to_string(target_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(InboxError::Io {
                path: target_path.to_path_buf(),
                source,
            });
        }
    };

    let updated = match format {
        AgentMcpConfigFormat::CodexToml => remove_codex_toml(&current, id)?,
        AgentMcpConfigFormat::CodebuddyJson => remove_json(&current, "mcpServers", id)?,
        AgentMcpConfigFormat::OpencodeJson => remove_json(&current, "mcp", id)?,
    };

    fs::write(target_path, updated).map_err(|source| InboxError::Io {
        path: target_path.to_path_buf(),
        source,
    })
}

// ---------------------------------------------------------------------------
// Codex TOML
// ---------------------------------------------------------------------------

fn parse_codex_doc(content: &str) -> Result<DocumentMut, InboxError> {
    match content.parse::<DocumentMut>() {
        Ok(doc) => Ok(doc),
        Err(first_err) => {
            let repaired = escape_invalid_toml_backslashes(content);
            if repaired != content {
                if let Ok(doc) = repaired.parse::<DocumentMut>() {
                    return Ok(doc);
                }
            }
            Err(InboxError::InvalidInput(format!(
                "codex config.toml parse error: {first_err}"
            )))
        }
    }
}

fn inspect_codex_toml(
    content: &str,
    desired: &AgentMcpServerConfig,
) -> Result<AgentMcpServerTarget, InboxError> {
    let doc = match parse_codex_doc(content) {
        Ok(doc) => doc,
        Err(err) => {
            return Ok(AgentMcpServerTarget::Drift {
                actual_summary: err.to_string(),
            });
        }
    };
    let Some(servers) = doc.get("mcp_servers").and_then(|item| item.as_table_like()) else {
        return Ok(AgentMcpServerTarget::Missing);
    };
    let Some(entry) = servers.get(&desired.id) else {
        return Ok(AgentMcpServerTarget::Missing);
    };
    let Some(table) = entry.as_table_like() else {
        return Ok(AgentMcpServerTarget::Drift {
            actual_summary: format!("mcp_servers.{} is not a table", desired.id),
        });
    };
    let AgentMcpTransport::RemoteHttp { url } = &desired.transport;
    let actual_url = table
        .get("url")
        .and_then(|item| item.as_value())
        .and_then(|v| v.as_str());
    // Other transports (command/args) would appear here — if present, Codex
    // considers them alternative transports, which counts as drift for the
    // purpose of a remote-HTTP bb endpoint.
    let has_command = table.contains_key("command") || table.contains_key("args");
    if has_command {
        return Ok(AgentMcpServerTarget::Drift {
            actual_summary: format!(
                "mcp_servers.{} uses command/args transport; expected url",
                desired.id
            ),
        });
    }
    match actual_url {
        Some(actual) if actual == url => Ok(AgentMcpServerTarget::Synced),
        Some(actual) => Ok(AgentMcpServerTarget::Drift {
            actual_summary: format!("url={actual}"),
        }),
        None => Ok(AgentMcpServerTarget::Drift {
            actual_summary: format!("mcp_servers.{} missing url", desired.id),
        }),
    }
}

fn escape_invalid_toml_backslashes(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('b' | 't' | 'n' | 'f' | 'r' | '"' | '\\' | 'u' | 'U') => out.push('\\'),
            Some(_) | None => out.push_str("\\\\"),
        }
    }
    out
}

fn upsert_codex_toml(
    current: Option<&str>,
    desired: &AgentMcpServerConfig,
) -> Result<String, InboxError> {
    let mut doc = match current {
        Some(content) if !content.trim().is_empty() => parse_codex_doc(content)?,
        _ => DocumentMut::new(),
    };

    if doc.get("mcp_servers").is_none() {
        let mut table = Table::new();
        table.set_implicit(true);
        doc.insert("mcp_servers", Item::Table(table));
    }

    let servers = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_mut())
        .ok_or_else(|| {
            InboxError::InvalidInput("codex config.toml has non-table mcp_servers".to_string())
        })?;

    let AgentMcpTransport::RemoteHttp { url } = &desired.transport;
    // Build an explicit subtable (not inline) so the result prints like
    // `[mcp_servers.bb]` / `url = "..."` — matching the existing style in
    // the user's config.toml.
    let mut entry = Table::new();
    entry.insert("url", value(url.as_str()));
    // Remove any stale command/args/env keys from a previously different
    // transport shape; upsert is authoritative.
    if let Some(existing) = servers.get(&desired.id).and_then(|i| i.as_table()) {
        for key in existing
            .iter()
            .map(|(k, _)| k.to_string())
            .collect::<Vec<_>>()
        {
            if key != "url" {
                entry.remove(&key);
            }
        }
    }
    servers.insert(&desired.id, Item::Table(entry));

    Ok(doc.to_string())
}

fn remove_codex_toml(content: &str, id: &str) -> Result<String, InboxError> {
    let mut doc = parse_codex_doc(content)?;
    if let Some(servers) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_mut())
    {
        servers.remove(id);
    }
    Ok(doc.to_string())
}

// ---------------------------------------------------------------------------
// JSON (CodeBuddy + OpenCode)
// ---------------------------------------------------------------------------

fn parse_json_doc(content: &str) -> Result<JsonValue, InboxError> {
    serde_json::from_str(content)
        .map_err(|err| InboxError::InvalidInput(format!("json config parse error: {err}")))
}

fn render_json(value: &JsonValue) -> String {
    // Preserve readability — the existing files are pretty-printed with 2
    // spaces; matching that keeps diffs minimal for the user.
    let mut out = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    out.push('\n');
    out
}

/// Build the JSON object representing the `CodeBuddy` `<id>` entry.
fn codebuddy_expected(desired: &AgentMcpServerConfig) -> JsonValue {
    let AgentMcpTransport::RemoteHttp { url } = &desired.transport;
    serde_json::json!({
        "type": "streamable-http",
        "url": url,
        "disabled": false,
        "timeout": 30000,
    })
}

/// Build the JSON object representing the `OpenCode` `<id>` entry.
fn opencode_expected(desired: &AgentMcpServerConfig) -> JsonValue {
    let AgentMcpTransport::RemoteHttp { url } = &desired.transport;
    serde_json::json!({
        "type": "remote",
        "url": url,
        "enabled": true,
    })
}

fn inspect_json(
    content: &str,
    section_key: &str,
    desired: &AgentMcpServerConfig,
    expected_fn: fn(&AgentMcpServerConfig) -> JsonValue,
) -> Result<AgentMcpServerTarget, InboxError> {
    let doc = parse_json_doc(content)?;
    let Some(section) = doc.get(section_key).and_then(|v| v.as_object()) else {
        return Ok(AgentMcpServerTarget::Missing);
    };
    let Some(actual) = section.get(&desired.id) else {
        return Ok(AgentMcpServerTarget::Missing);
    };
    let expected = expected_fn(desired);
    if actual == &expected {
        Ok(AgentMcpServerTarget::Synced)
    } else {
        Ok(AgentMcpServerTarget::Drift {
            actual_summary: summarize_json_value(actual),
        })
    }
}

fn upsert_json(
    current: Option<&str>,
    section_key: &str,
    desired: &AgentMcpServerConfig,
    expected_fn: fn(&AgentMcpServerConfig) -> JsonValue,
) -> Result<String, InboxError> {
    let mut doc = match current {
        Some(content) if !content.trim().is_empty() => parse_json_doc(content)?,
        _ => JsonValue::Object(JsonMap::new()),
    };

    let obj = doc.as_object_mut().ok_or_else(|| {
        InboxError::InvalidInput(format!(
            "top-level json config must be an object to host `{section_key}`"
        ))
    })?;

    let section = obj
        .entry(section_key.to_string())
        .or_insert_with(|| JsonValue::Object(JsonMap::new()));
    let kind = section_kind(section);
    let section_obj = section.as_object_mut().ok_or_else(|| {
        InboxError::InvalidInput(format!(
            "`{section_key}` in json config must be an object, not {kind}"
        ))
    })?;
    section_obj.insert(desired.id.clone(), expected_fn(desired));

    Ok(render_json(&doc))
}

fn remove_json(content: &str, section_key: &str, id: &str) -> Result<String, InboxError> {
    let mut doc = parse_json_doc(content)?;
    if let Some(obj) = doc.as_object_mut() {
        if let Some(section) = obj.get_mut(section_key).and_then(|v| v.as_object_mut()) {
            section.remove(id);
        }
    }
    Ok(render_json(&doc))
}

fn summarize_json_value(value: &JsonValue) -> String {
    let mut text = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    if text.len() > 120 {
        text.truncate(120);
        text.push('…');
    }
    text
}

const fn section_kind(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests;
