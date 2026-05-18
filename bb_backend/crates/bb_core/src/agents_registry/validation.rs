use std::collections::BTreeSet;
use std::path::Path;

use crate::InboxError;

use super::model::{AgentProfile, AgentRegistryFile};

/// Warnings emitted during validation that do not block writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationWarning {
    pub message: String,
}

pub(super) fn validate_registry(registry: &AgentRegistryFile) -> Result<(), InboxError> {
    let mut ids = BTreeSet::new();
    for agent in &registry.agents {
        validate_agent(agent)?;
        if !ids.insert(agent.id.clone()) {
            return Err(InboxError::InvalidInput(format!(
                "duplicate agent id `{}` in agents.toml",
                agent.id
            )));
        }
    }
    for membership in &registry.project_agents {
        validate_project_ref(&membership.project)?;
        validate_agent_id(&membership.agent)?;
    }
    Ok(())
}

/// Extended registry-level validation that also collects warnings.
pub(super) fn validate_registry_with_warnings(
    registry: &AgentRegistryFile,
) -> Result<Vec<ValidationWarning>, InboxError> {
    validate_registry(registry)?;

    let mut warnings = Vec::new();

    for agent in &registry.agents {
        // custom_args safety: warn about --mcp-config
        warnings.extend(validate_custom_args_safety(&agent.id, &agent.custom_args));

        // MCP servers warnings (path traversal in command)
        warnings.extend(validate_mcp_servers_warnings(agent));
    }

    Ok(warnings)
}

pub(super) fn validate_agent(agent: &AgentProfile) -> Result<(), InboxError> {
    validate_agent_id(&agent.id)?;
    validate_required_string("agent display_name", &agent.display_name)?;
    validate_required_string("agent kind", &agent.kind)?;
    match agent.scope.as_str() {
        "global" | "project" => {}
        other => {
            return Err(InboxError::InvalidInput(format!(
                "agent `{}` has invalid scope `{other}`",
                agent.id
            )))
        }
    }
    match agent.status.as_str() {
        "active" | "inactive" | "archived" => {}
        other => {
            return Err(InboxError::InvalidInput(format!(
                "agent `{}` has invalid status `{other}`",
                agent.id
            )))
        }
    }

    // --- New Profile field validations (Ticket #000048) ---
    validate_instructions_mutual_exclusion(agent)?;
    validate_max_concurrent_tasks(agent)?;
    // --- MCP servers (Ticket #000049) ---
    validate_mcp_servers(agent)?;

    // --- Skills (Ticket #000050) ---
    validate_skills(agent)?;

    Ok(())
}

// ── Instructions mutual exclusion ──

fn validate_instructions_mutual_exclusion(agent: &AgentProfile) -> Result<(), InboxError> {
    if agent.instructions.is_some() && agent.instructions_path.is_some() {
        return Err(InboxError::InvalidInput(format!(
            "agent `{}`: instructions and instructions_path are mutually exclusive",
            agent.id
        )));
    }
    Ok(())
}

// ── Instructions path safety ──

/// Validate that `instructions_path` does not escape `bb_root` via path traversal.
/// Actual file existence is checked at load time, not during schema validation,
/// so this only rejects obviously unsafe paths.
pub(super) fn validate_instructions_path_safety(
    bb_root: &Path,
    agent: &AgentProfile,
) -> Result<(), InboxError> {
    let Some(ref rel) = agent.instructions_path else {
        return Ok(());
    };
    let resolved = bb_root.join(rel);
    let canonical_root = bb_root
        .canonicalize()
        .unwrap_or_else(|_| bb_root.to_path_buf());
    let canonical_resolved = resolved.canonicalize().map_err(|_| {
        InboxError::InvalidInput(format!(
            "agent `{}`: instructions_path `{rel}` does not exist",
            agent.id
        ))
    })?;
    if !canonical_resolved.starts_with(&canonical_root) {
        return Err(InboxError::InvalidInput(format!(
            "agent `{}`: instructions_path `{rel}` escapes bb_root",
            agent.id
        )));
    }
    Ok(())
}

// ── max_concurrent_tasks ──

fn validate_max_concurrent_tasks(agent: &AgentProfile) -> Result<(), InboxError> {
    if agent.max_concurrent_tasks == Some(0) {
        return Err(InboxError::InvalidInput(format!(
            "agent `{}`: max_concurrent_tasks must be > 0",
            agent.id
        )));
    }
    Ok(())
}

// ── MCP servers (Ticket #000049) ──

fn validate_mcp_servers(agent: &AgentProfile) -> Result<(), InboxError> {
    let mut seen_names = BTreeSet::new();
    for server in &agent.mcp_servers {
        // name format: kebab-case
        validate_mcp_server_name(&agent.id, &server.name)?;

        // name uniqueness within this agent
        if !seen_names.insert(server.name.clone()) {
            return Err(InboxError::InvalidInput(format!(
                "agent `{}`: duplicate MCP server name `{}`",
                agent.id, server.name
            )));
        }

        // transport validation
        match server.transport.as_str() {
            "stdio" => {
                if server.command.is_none() || server.command.as_deref() == Some("") {
                    return Err(InboxError::InvalidInput(format!(
                        "agent `{}`: MCP server `{}` with transport=stdio requires a command",
                        agent.id, server.name
                    )));
                }
            }
            "sse" => {
                if server.url.is_none() || server.url.as_deref() == Some("") {
                    return Err(InboxError::InvalidInput(format!(
                        "agent `{}`: MCP server `{}` with transport=sse requires a url",
                        agent.id, server.name
                    )));
                }
            }
            other => {
                return Err(InboxError::InvalidInput(format!(
                    "agent `{}`: MCP server `{}` has invalid transport `{other}`, must be \"stdio\" or \"sse\"",
                    agent.id, server.name
                )));
            }
        }

        // env key validation
        for key in server.env.keys() {
            if key.is_empty() {
                return Err(InboxError::InvalidInput(format!(
                    "agent `{}`: MCP server `{}` has an empty env key",
                    agent.id, server.name
                )));
            }
        }
    }
    Ok(())
}

fn validate_mcp_server_name(agent_id: &str, name: &str) -> Result<(), InboxError> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(InboxError::InvalidInput(format!(
            "agent `{agent_id}`: invalid MCP server name `{name}` (must be kebab-case)"
        )))
    }
}

/// Collect MCP-related warnings (e.g. command path traversal).
pub(super) fn validate_mcp_servers_warnings(agent: &AgentProfile) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    for server in &agent.mcp_servers {
        if let Some(ref cmd) = server.command {
            if cmd.contains("..") {
                warnings.push(ValidationWarning {
                    message: format!(
                        "agent `{}`: MCP server `{}` command contains path traversal characters",
                        agent.id, server.name
                    ),
                });
            }
        }
    }
    warnings
}

// ── Skills (Ticket #000050) ──

fn validate_skills(agent: &AgentProfile) -> Result<(), InboxError> {
    let mut seen = BTreeSet::new();
    for skill_name in &agent.skills {
        // format: kebab-case
        let valid = !skill_name.is_empty()
            && skill_name.len() <= 64
            && skill_name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
            && skill_name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
        if !valid {
            return Err(InboxError::InvalidInput(format!(
                "agent `{}`: invalid skill name `{skill_name}` (must be kebab-case)",
                agent.id
            )));
        }
        if !seen.insert(skill_name.clone()) {
            return Err(InboxError::InvalidInput(format!(
                "agent `{}`: duplicate skill name `{skill_name}`",
                agent.id
            )));
        }
    }
    Ok(())
}

// ── custom_args safety ──

fn validate_custom_args_safety(agent_id: &str, args: &[String]) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    if args.iter().any(|a| a == "--mcp-config") {
        warnings.push(ValidationWarning {
            message: format!(
                "agent `{agent_id}`: custom_args contains --mcp-config; MCP config should be injected by daemon, not via custom_args"
            ),
        });
    }
    warnings
}

// ── Existing helpers (unchanged) ──

pub(super) fn is_active_assignable(agent: &AgentProfile) -> bool {
    agent.status == "active" && agent.assignable
}

pub(super) fn validate_agent_id(value: &str) -> Result<&str, InboxError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if valid {
        Ok(value)
    } else {
        Err(InboxError::InvalidInput(format!(
            "invalid agent id `{value}`"
        )))
    }
}

pub(super) fn validate_project_ref(value: &str) -> Result<&str, InboxError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if valid {
        Ok(value)
    } else {
        Err(InboxError::InvalidProjectName(value.to_string()))
    }
}

pub(super) fn validate_lane_ref(value: &str) -> Result<&str, InboxError> {
    let valid = !value.is_empty()
        && value.len() <= 32
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase());
    if valid {
        Ok(value)
    } else {
        Err(InboxError::InvalidInput(format!(
            "invalid project agent lane `{value}`"
        )))
    }
}

pub(super) fn validate_required_string<'a>(
    label: &str,
    value: &'a str,
) -> Result<&'a str, InboxError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(InboxError::InvalidInput(format!(
            "{label} must not be empty"
        )))
    } else {
        Ok(trimmed)
    }
}
