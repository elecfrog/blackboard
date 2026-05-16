//! Agent configuration connectors.
//!
//! Treats `<bb-root>/agents/AGENTS.md` as the default source of truth for
//! cross-Agent global rules and exposes a small "connector" abstraction that
//! lets callers inspect / sync / disconnect the per-Agent target paths
//! (e.g. `~/.codex/AGENTS.md`). Individual connectors may point at a
//! specialized source file while still writing the tool-required target name.
//! MCP server target injection is part of this connector domain as
//! `mcp_connector`; Blackboard's own MCP server tools live in `bb_cli`.
//!
//! The catalog of supported Agents is intentionally a static array for the
//! first version. Future iterations can layer a `<bb-root>/agents/connectors.toml`
//! override on top, but that is explicitly out of scope here.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{agents_registry, InboxError};

mod digest;
mod mcp_connector;
mod model;
mod paths;
mod rules;

use digest::{mtime_rfc3339, read_to_bytes, sha256_hex, short_hash};
use mcp_connector::{AgentMcpServerConfig, AgentMcpServerTarget, AgentMcpTransport};
use paths::{expand_target, path_for_display};
use rules::{extract_rules_body, wrap_rules_body};

pub use mcp_connector::AgentMcpConfigFormat;
pub use model::*;
pub use paths::source_path;

#[derive(Debug, Clone)]
struct ResolvedConnectorTargetSpec {
    label: String,
    target_template: String,
    source_path: PathBuf,
    connector_type: AgentConnectorType,
    /// When `Some`, this target is an MCP server injection; `source_path` is
    /// not read as bytes, and instead the embedded server config is the
    /// "source of truth" compared against the target file contents.
    mcp: Option<ResolvedMcpTarget>,
}

#[derive(Debug, Clone)]
struct ResolvedMcpTarget {
    format: AgentMcpConfigFormat,
    server: AgentMcpServerConfig,
}

fn resolve_connector_targets(
    spec: &AgentConnectorSpec,
    bb_root: &Path,
) -> Result<Vec<ResolvedConnectorTargetSpec>, InboxError> {
    resolve_connector_targets_with_mcp_url(spec, bb_root, None)
}

fn resolve_connector_targets_with_mcp_url(
    spec: &AgentConnectorSpec,
    bb_root: &Path,
    mcp_remote_url: Option<&str>,
) -> Result<Vec<ResolvedConnectorTargetSpec>, InboxError> {
    let mut targets = Vec::new();
    let canonical_agents = source_path(bb_root);
    for target in spec.targets {
        let source_path = match target.source_template {
            Some(template) => expand_target(template, None, bb_root)?,
            None => canonical_agents.clone(),
        };
        targets.push(ResolvedConnectorTargetSpec {
            label: target.label.to_string(),
            target_template: target.target_template.to_string(),
            source_path,
            connector_type: target.connector_type,
            mcp: None,
        });
    }

    if spec.id == "opencode" {
        for (agent, source_path) in agents_registry::opencode_distribution_sources(bb_root)? {
            let Some(file_name) = source_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            targets.push(ResolvedConnectorTargetSpec {
                label: format!("Agent: {}", agent.id),
                target_template: format!("~/.config/opencode/agents/{file_name}"),
                source_path,
                connector_type: AgentConnectorType::AgentsMd,
                mcp: None,
            });
        }
    }

    if let Some(mcp_spec) = spec.mcp_target {
        let remote_url = mcp_remote_url.unwrap_or(mcp_spec.remote_url);
        let server = AgentMcpServerConfig {
            id: mcp_spec.server_id.to_string(),
            description: mcp_spec.server_description.to_string(),
            transport: AgentMcpTransport::RemoteHttp {
                url: remote_url.to_string(),
            },
        };
        targets.push(ResolvedConnectorTargetSpec {
            label: mcp_spec.label.to_string(),
            target_template: mcp_spec.target_template.to_string(),
            // Mcp targets do not read source bytes; keep a placeholder path so
            // `source_path` fields stay populated for diagnostics.
            source_path: PathBuf::from(format!("mcp:{}/{}", spec.id, mcp_spec.server_id)),
            connector_type: AgentConnectorType::McpServer,
            mcp: Some(ResolvedMcpTarget {
                format: mcp_spec.format,
                server,
            }),
        });
    }

    Ok(targets)
}

/// Look up a connector spec by id, returning `InvalidInput` for unknown ids.
pub fn find_connector_spec(id: &str) -> Result<&'static AgentConnectorSpec, InboxError> {
    AGENT_CONNECTORS
        .iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| InboxError::InvalidInput(format!("unknown agent connector id: {id}")))
}

/// Inspect a single connector against the current source bytes (which may be
/// `None` when the source is missing).
fn inspect_connector_target(
    target_spec: &ResolvedConnectorTargetSpec,
    home_override: Option<&Path>,
    bb_root: &Path,
) -> Result<AgentConnectorTarget, InboxError> {
    let target_path = expand_target(&target_spec.target_template, home_override, bb_root)?;
    let target_path_str = path_for_display(&target_path);
    let source_path_str = path_for_display(&target_spec.source_path);

    // McpServer targets use a completely separate inspection path because the
    // "source of truth" is the static AgentMcpServerConfig, not a file-bytes hash.
    if target_spec.connector_type == AgentConnectorType::McpServer {
        let mcp = target_spec
            .mcp
            .as_ref()
            .expect("McpServer target must carry ResolvedMcpTarget");
        let (state, target_sha, error) =
            match mcp_connector::inspect_server(mcp.format, &target_path, &mcp.server) {
                Ok(AgentMcpServerTarget::Synced) => (AgentConnectorState::Synced, None, None),
                Ok(AgentMcpServerTarget::Drift { actual_summary }) => (
                    AgentConnectorState::Drift,
                    Some(short_hash(&actual_summary)),
                    Some(actual_summary),
                ),
                Ok(AgentMcpServerTarget::Missing) => (AgentConnectorState::Missing, None, None),
                Ok(AgentMcpServerTarget::Unreachable) => {
                    (AgentConnectorState::Unreachable, None, None)
                }
                Err(err) => {
                    let message = err.to_string();
                    (
                        AgentConnectorState::Drift,
                        Some(short_hash(&message)),
                        Some(message),
                    )
                }
            };
        let target_mtime = fs::metadata(&target_path)
            .ok()
            .and_then(|meta| mtime_rfc3339(&meta));
        let is_symlink = fs::symlink_metadata(&target_path)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        return Ok(AgentConnectorTarget {
            label: target_spec.label.clone(),
            target_template: target_spec.target_template.clone(),
            target_path: target_path_str,
            source_path: Some(source_path_str),
            connector_type: target_spec.connector_type,
            state,
            source_sha256_short: None,
            target_sha256_short: target_sha,
            target_mtime,
            is_symlink,
            error,
            events: Vec::new(),
        });
    }

    let source_bytes =
        read_to_bytes(&target_spec.source_path).map_err(|source| InboxError::Io {
            path: target_spec.source_path.clone(),
            source,
        })?;
    let source_full = source_bytes.as_deref().map(sha256_hex);
    let source_short = source_full.as_deref().map(short_hash);

    let symlink_meta = fs::symlink_metadata(&target_path);
    let is_symlink = symlink_meta
        .as_ref()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    let state;
    let mut target_sha = None;
    let mut target_mtime = None;

    if source_full.is_none() {
        state = AgentConnectorState::SourceMissing;
        if let Some(bytes) = read_to_bytes(&target_path).map_err(|source| InboxError::Io {
            path: target_path.clone(),
            source,
        })? {
            target_sha = Some(short_hash(&sha256_hex(&bytes)));
        }
        if let Ok(meta) = fs::metadata(&target_path) {
            target_mtime = mtime_rfc3339(&meta);
        }
    } else if let Some(bytes) = read_to_bytes(&target_path).map_err(|source| InboxError::Io {
        path: target_path.clone(),
        source,
    })? {
        let target_full = sha256_hex(&bytes);
        target_sha = Some(short_hash(&target_full));
        if let Ok(meta) = fs::metadata(&target_path) {
            target_mtime = mtime_rfc3339(&meta);
        }

        // For Rules connectors, compare only the body (after the frontmatter
        // header) against the source content.
        state = if target_spec.connector_type == AgentConnectorType::Rules {
            let target_str = String::from_utf8_lossy(&bytes);
            let target_body = extract_rules_body(&target_str);
            let target_body_full = sha256_hex(target_body.as_bytes());
            if target_body_full == source_full.as_deref().unwrap() {
                AgentConnectorState::Synced
            } else {
                AgentConnectorState::Drift
            }
        } else {
            if Some(target_full.as_str()) == source_full.as_deref() {
                AgentConnectorState::Synced
            } else {
                AgentConnectorState::Drift
            }
        };
    } else {
        let parent_exists = target_path.parent().map(|p| p.is_dir()).unwrap_or(false);
        let rules_root_exists = target_spec.connector_type == AgentConnectorType::Rules
            && target_path
                .parent()
                .and_then(Path::parent)
                .map(|p| p.is_dir())
                .unwrap_or(false);
        state = if parent_exists || rules_root_exists {
            AgentConnectorState::Missing
        } else {
            AgentConnectorState::Unreachable
        };
    }

    Ok(AgentConnectorTarget {
        label: target_spec.label.to_string(),
        target_template: target_spec.target_template.to_string(),
        target_path: target_path_str,
        source_path: Some(source_path_str),
        connector_type: target_spec.connector_type,
        state,
        source_sha256_short: source_short,
        target_sha256_short: target_sha,
        target_mtime,
        is_symlink,
        error: None,
        events: Vec::new(),
    })
}

fn aggregate_target_states(targets: &[AgentConnectorTarget]) -> AgentConnectorState {
    if targets
        .iter()
        .any(|t| t.state == AgentConnectorState::SourceMissing)
    {
        AgentConnectorState::SourceMissing
    } else if targets
        .iter()
        .all(|t| t.state == AgentConnectorState::Synced)
    {
        AgentConnectorState::Synced
    } else if targets
        .iter()
        .any(|t| t.state == AgentConnectorState::Drift)
    {
        AgentConnectorState::Drift
    } else if targets
        .iter()
        .any(|t| t.state == AgentConnectorState::Missing)
    {
        AgentConnectorState::Missing
    } else {
        AgentConnectorState::Unreachable
    }
}

fn inspect_connector(
    spec: &AgentConnectorSpec,
    home_override: Option<&Path>,
    bb_root: &Path,
) -> Result<AgentConnector, InboxError> {
    inspect_connector_with_mcp_url(spec, home_override, bb_root, None)
}

fn inspect_connector_with_mcp_url(
    spec: &AgentConnectorSpec,
    home_override: Option<&Path>,
    bb_root: &Path,
    mcp_remote_url: Option<&str>,
) -> Result<AgentConnector, InboxError> {
    let resolved_targets = resolve_connector_targets_with_mcp_url(spec, bb_root, mcp_remote_url)?;
    let mut targets = Vec::with_capacity(spec.targets.len());
    for target_spec in &resolved_targets {
        targets.push(inspect_connector_target(
            target_spec,
            home_override,
            bb_root,
        )?);
    }

    let primary = targets.first().ok_or_else(|| {
        InboxError::InvalidInput(format!("connector {:?} has no targets", spec.id))
    })?;
    let primary_source_full = resolved_targets
        .first()
        .filter(|target| target.connector_type != AgentConnectorType::McpServer)
        .map(|target| {
            read_to_bytes(&target.source_path)
                .map_err(|source| InboxError::Io {
                    path: target.source_path.clone(),
                    source,
                })
                .map(|bytes| bytes.as_deref().map(sha256_hex))
        })
        .transpose()?
        .flatten();

    Ok(AgentConnector {
        id: spec.id.to_string(),
        display_name: spec.display_name.to_string(),
        target_template: primary.target_template.clone(),
        target_path: primary.target_path.clone(),
        connector_type: primary.connector_type,
        state: aggregate_target_states(&targets),
        source_sha256_short: primary_source_full.as_deref().map(short_hash),
        target_sha256_short: primary.target_sha256_short.clone(),
        target_mtime: primary.target_mtime.clone(),
        is_symlink: targets.iter().any(|target| target.is_symlink),
        events: Vec::new(),
        targets,
    })
}

/// List all supported connectors and their current state.
pub fn list_agent_connectors(bb_root: &Path) -> Result<AgentConnectorList, InboxError> {
    list_with_home(bb_root, None)
}

/// List connectors using a runtime MCP URL instead of the static default.
pub fn list_agent_connectors_with_mcp_url(
    bb_root: &Path,
    mcp_remote_url: Option<&str>,
) -> Result<AgentConnectorList, InboxError> {
    list_with_home_and_mcp_url(bb_root, None, mcp_remote_url)
}

fn list_with_home(
    bb_root: &Path,
    home_override: Option<&Path>,
) -> Result<AgentConnectorList, InboxError> {
    list_with_home_and_mcp_url(bb_root, home_override, None)
}

fn list_with_home_and_mcp_url(
    bb_root: &Path,
    home_override: Option<&Path>,
    mcp_remote_url: Option<&str>,
) -> Result<AgentConnectorList, InboxError> {
    let source = source_path(bb_root);
    let source_path_str = path_for_display(&source);
    let source_bytes = read_to_bytes(&source).map_err(|err| InboxError::Io {
        path: source.clone(),
        source: err,
    })?;

    let (source_state, source_short) = match &source_bytes {
        Some(bytes) => {
            let full = sha256_hex(bytes);
            let short = short_hash(&full);
            (AgentSourceState::Present, Some(short))
        }
        None => (AgentSourceState::Missing, None),
    };

    let mut connectors = Vec::with_capacity(AGENT_CONNECTORS.len());
    for spec in AGENT_CONNECTORS {
        connectors.push(inspect_connector_with_mcp_url(
            spec,
            home_override,
            bb_root,
            mcp_remote_url,
        )?);
    }

    Ok(AgentConnectorList {
        source_state,
        source_path: source_path_str,
        source_sha256_short: source_short,
        connectors,
    })
}

/// Sync a single connector by overwriting its target file with the source
/// bytes. Creates the parent directory if needed.
pub fn sync_agent_connector(bb_root: &Path, id: &str) -> Result<AgentConnector, InboxError> {
    sync_with_home(bb_root, id, None)
}

/// Sync a connector using a runtime MCP URL instead of the static default.
pub fn sync_agent_connector_with_mcp_url(
    bb_root: &Path,
    id: &str,
    mcp_remote_url: Option<&str>,
) -> Result<AgentConnector, InboxError> {
    sync_with_home_and_mcp_url(bb_root, id, None, mcp_remote_url)
}

fn sync_with_home(
    bb_root: &Path,
    id: &str,
    home_override: Option<&Path>,
) -> Result<AgentConnector, InboxError> {
    sync_with_home_and_mcp_url(bb_root, id, home_override, None)
}

fn sync_with_home_and_mcp_url(
    bb_root: &Path,
    id: &str,
    home_override: Option<&Path>,
    mcp_remote_url: Option<&str>,
) -> Result<AgentConnector, InboxError> {
    let spec = find_connector_spec(id)?;
    let resolved_targets = resolve_connector_targets_with_mcp_url(spec, bb_root, mcp_remote_url)?;

    // Collect per-target side-effects produced during this sync so we can
    // attach them to the post-sync snapshot returned to the caller. The key
    // matches `AgentConnectorTarget::target_path` (produced by
    // `path_for_display`) so we can pair events with targets by path string.
    let mut events_by_target: HashMap<String, Vec<AgentConnectorSyncEvent>> = HashMap::new();

    for target_spec in &resolved_targets {
        let target_path = expand_target(&target_spec.target_template, home_override, bb_root)?;

        // McpServer: merge-into-existing-file semantics. Don't read AGENTS.md
        // source, don't overwrite; delegate to mcp_connector::upsert_server.
        if target_spec.connector_type == AgentConnectorType::McpServer {
            let mcp = target_spec
                .mcp
                .as_ref()
                .expect("McpServer target must carry ResolvedMcpTarget");
            mcp_connector::upsert_server(mcp.format, &target_path, &mcp.server)?;
            continue;
        }

        let source_bytes =
            read_to_bytes(&target_spec.source_path).map_err(|source| InboxError::Io {
                path: target_spec.source_path.clone(),
                source,
            })?;
        let source_bytes = source_bytes.ok_or_else(|| {
            InboxError::InvalidInput(format!(
                "agent source-of-truth missing at {}",
                target_spec.source_path.display()
            ))
        })?;
        if same_path(&target_path, &target_spec.source_path) {
            return Err(InboxError::InvalidInput(format!(
                "connector {id:?} target equals source path; refusing to overwrite"
            )));
        }

        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| InboxError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
        }

        if let Ok(meta) = fs::symlink_metadata(&target_path) {
            if meta.file_type().is_symlink() {
                // Strict-flatten semantics are preserved: we still unlink the
                // symlink unconditionally. The only change is that we now
                // capture what it used to point at and emit a structured
                // `FlattenedSymlink` event so the sync does not happen
                // silently for the human operator.
                let previous_link_target = fs::read_link(&target_path)
                    .ok()
                    .map(|p| path_for_display(&p));
                fs::remove_file(&target_path).map_err(|source| InboxError::Io {
                    path: target_path.clone(),
                    source,
                })?;
                let target_path_display = path_for_display(&target_path);
                events_by_target
                    .entry(target_path_display.clone())
                    .or_default()
                    .push(AgentConnectorSyncEvent::FlattenedSymlink {
                        target_path: target_path_display,
                        previous_link_target,
                    });
            }
        }

        let write_bytes = match target_spec.connector_type {
            AgentConnectorType::AgentsMd => source_bytes.clone(),
            AgentConnectorType::Rules => wrap_rules_body(&source_bytes),
            AgentConnectorType::McpServer => unreachable!("handled above"),
        };

        fs::write(&target_path, &write_bytes).map_err(|source| InboxError::Io {
            path: target_path.clone(),
            source,
        })?;
    }

    let mut connector =
        inspect_connector_with_mcp_url(spec, home_override, bb_root, mcp_remote_url)?;
    if !events_by_target.is_empty() {
        let mut aggregated: Vec<AgentConnectorSyncEvent> = Vec::new();
        for target in connector.targets.iter_mut() {
            if let Some(events) = events_by_target.remove(&target.target_path) {
                aggregated.extend(events.iter().cloned());
                target.events = events;
            }
        }
        // Any events whose target_path did not line up with an inspect
        // result (shouldn't normally happen) still surface on the aggregate.
        for (_, leftover) in events_by_target.drain() {
            aggregated.extend(leftover);
        }
        connector.events = aggregated;
    }
    Ok(connector)
}

/// Disconnect a connector by removing its target file. Parent directory is
/// left intact.
pub fn disconnect_agent_connector(bb_root: &Path, id: &str) -> Result<AgentConnector, InboxError> {
    disconnect_with_home(bb_root, id, None)
}

fn disconnect_with_home(
    bb_root: &Path,
    id: &str,
    home_override: Option<&Path>,
) -> Result<AgentConnector, InboxError> {
    let spec = find_connector_spec(id)?;
    let resolved_targets = resolve_connector_targets(spec, bb_root)?;
    for target_spec in &resolved_targets {
        let target_path = expand_target(&target_spec.target_template, home_override, bb_root)?;

        // McpServer: do NOT delete the config file; only remove our own key.
        if target_spec.connector_type == AgentConnectorType::McpServer {
            let mcp = target_spec
                .mcp
                .as_ref()
                .expect("McpServer target must carry ResolvedMcpTarget");
            mcp_connector::remove_server(mcp.format, &target_path, &mcp.server.id)?;
            continue;
        }

        match fs::remove_file(&target_path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(InboxError::Io {
                    path: target_path.clone(),
                    source,
                });
            }
        }
    }

    inspect_connector(spec, home_override, bb_root)
}

fn same_path(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a)
        .ok()
        .zip(fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;
