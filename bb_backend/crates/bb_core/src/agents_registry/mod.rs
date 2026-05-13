//! Agent registry.
//!
//! `<bb-root>/agents/agents.toml` is the source of truth for agents that can
//! own Blackboard tickets. It deliberately separates:
//! - platform connectors (how rules are distributed)
//! - agents / sub-agents (who can be assigned or distributed)
//! - project registration (which project opts into which agents)

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::InboxError;

mod model;
mod paths;
mod validation;

pub use model::{
    AgentProfile, AgentRegistryFile, AgentRegistryList, AgentRegistrySourceState, McpServerConfig,
    ProjectAgentList, ProjectAgentProfile, ProjectAgentRegistration,
    RemovedProjectAgentRegistration,
};
pub use paths::{registry_path, resolve_source_path};

#[cfg(test)]
use model::{default_scope, default_status};
use paths::path_for_display;
use validation::{
    is_active_assignable, validate_agent, validate_agent_id, validate_instructions_path_safety,
    validate_lane_ref, validate_project_ref, validate_registry, validate_registry_with_warnings,
    validate_required_string,
};

pub fn list_agents(bb_root: &Path) -> Result<AgentRegistryList, InboxError> {
    let path = registry_path(bb_root);
    let Some(registry) = load_registry_optional(bb_root)? else {
        return Ok(AgentRegistryList {
            source_state: AgentRegistrySourceState::Missing,
            source_path: path_for_display(&path),
            agents: Vec::new(),
            project_agents: Vec::new(),
        });
    };

    Ok(AgentRegistryList {
        source_state: AgentRegistrySourceState::Present,
        source_path: path_for_display(&path),
        agents: registry.agents,
        project_agents: registry.project_agents,
    })
}

pub fn list_project_agents(bb_root: &Path, project: &str) -> Result<ProjectAgentList, InboxError> {
    validate_project_ref(project)?;
    let path = registry_path(bb_root);
    let Some(registry) = load_registry_optional(bb_root)? else {
        return Ok(ProjectAgentList {
            source_state: AgentRegistrySourceState::Missing,
            source_path: path_for_display(&path),
            project: project.to_string(),
            agents: Vec::new(),
        });
    };

    let mut by_id = BTreeMap::new();
    for agent in registry.agents {
        by_id.insert(agent.id.clone(), agent);
    }

    let mut memberships = BTreeMap::<String, ProjectAgentRegistration>::new();
    for membership in registry.project_agents {
        validate_project_ref(&membership.project)?;
        validate_agent_id(&membership.agent)?;
        if membership.project == project {
            memberships.insert(membership.agent.clone(), membership);
        }
    }

    let mut agents = Vec::new();
    let mut seen = BTreeSet::new();
    for agent in by_id.values() {
        validate_agent(agent)?;
        if !is_active_assignable(agent) {
            continue;
        }
        if agent.scope == "global" {
            seen.insert(agent.id.clone());
            let membership = memberships.get(&agent.id);
            agents.push(ProjectAgentProfile {
                agent: agent.clone(),
                origin: if membership.is_some() {
                    "project".to_string()
                } else {
                    "global".to_string()
                },
                project_role: membership.and_then(|m| m.role.clone()),
                project_lanes: membership.map(|m| m.lanes.clone()).unwrap_or_default(),
            });
        }
    }

    for (id, membership) in memberships {
        if seen.contains(&id) {
            continue;
        }
        let agent = by_id.get(&id).ok_or_else(|| {
            InboxError::InvalidInput(format!(
                "project `{project}` references unknown agent `{id}` in agents.toml"
            ))
        })?;
        validate_agent(agent)?;
        if !is_active_assignable(agent) {
            continue;
        }
        agents.push(ProjectAgentProfile {
            agent: agent.clone(),
            origin: "project".to_string(),
            project_role: membership.role,
            project_lanes: membership.lanes,
        });
    }

    agents.sort_by(|a, b| a.agent.id.cmp(&b.agent.id));
    Ok(ProjectAgentList {
        source_state: AgentRegistrySourceState::Present,
        source_path: path_for_display(&path),
        project: project.to_string(),
        agents,
    })
}

pub fn upsert_agent(bb_root: &Path, mut agent: AgentProfile) -> Result<AgentProfile, InboxError> {
    agent.id = validate_agent_id(&agent.id)?.to_string();
    agent.display_name =
        validate_required_string("agent display_name", &agent.display_name)?.to_string();
    agent.kind = validate_required_string("agent kind", &agent.kind)?.to_string();
    agent.scope = validate_required_string("agent scope", &agent.scope)?.to_string();
    agent.status = validate_required_string("agent status", &agent.status)?.to_string();
    if let Some(runtime) = agent.runtime.as_mut() {
        *runtime = runtime.trim().to_string();
        if runtime.is_empty() {
            agent.runtime = None;
        }
    }
    if let Some(source_path) = agent.source_path.as_deref() {
        resolve_source_path(bb_root, source_path)?;
    }

    // ── Clean new Profile fields (Ticket #000048) ──
    if let Some(model) = agent.model.as_mut() {
        *model = model.trim().to_string();
        if model.is_empty() {
            agent.model = None;
        }
    }
    if let Some(variant) = agent.variant.as_mut() {
        *variant = variant.trim().to_string();
        if variant.is_empty() {
            agent.variant = None;
        }
    }
    if let Some(instructions) = agent.instructions.as_mut() {
        *instructions = instructions.trim().to_string();
        if instructions.is_empty() {
            agent.instructions = None;
        }
    }
    if let Some(instructions_path) = agent.instructions_path.as_mut() {
        *instructions_path = instructions_path.trim().to_string();
        if instructions_path.is_empty() {
            agent.instructions_path = None;
        }
    }
    if let Some(org_role) = agent.org_role.as_mut() {
        *org_role = org_role.trim().to_string();
        if org_role.is_empty() {
            agent.org_role = None;
        }
    }
    if let Some(coordinator) = agent.coordinator.as_mut() {
        *coordinator = coordinator.trim().to_string();
        if coordinator.is_empty() {
            agent.coordinator = None;
        }
    }
    agent.workers.retain(|w| !w.trim().is_empty());
    // Remove empty custom_env entries
    agent
        .custom_env
        .retain(|k, v| !k.trim().is_empty() && !v.trim().is_empty());

    // ── Clean MCP servers (Ticket #000049) ──
    for server in &mut agent.mcp_servers {
        server.name = server.name.trim().to_string();
        if let Some(cmd) = server.command.as_mut() {
            *cmd = cmd.trim().to_string();
            if cmd.is_empty() {
                server.command = None;
            }
        }
        if let Some(url) = server.url.as_mut() {
            *url = url.trim().to_string();
            if url.is_empty() {
                server.url = None;
            }
        }
        server.args.retain(|a| !a.trim().is_empty());
        server.env.retain(|k, _| !k.trim().is_empty());
    }
    agent.mcp_servers.retain(|s| !s.name.is_empty());

    // ── Clean skills (Ticket #000050) ──
    agent.skills.retain(|s| !s.trim().is_empty());

    validate_agent(&agent)?;
    validate_instructions_path_safety(bb_root, &agent)?;

    let mut registry = load_registry_optional(bb_root)?.unwrap_or_else(default_registry);
    if let Some(existing) = registry.agents.iter_mut().find(|item| item.id == agent.id) {
        *existing = agent.clone();
    } else {
        registry.agents.push(agent.clone());
    }
    registry.agents.sort_by(|a, b| a.id.cmp(&b.id));
    validate_registry(&registry)?;
    // Collect warnings (logged but do not block)
    let _warnings = validate_registry_with_warnings(&registry).unwrap_or_default();
    write_registry(bb_root, &registry)?;
    Ok(agent)
}

pub fn upsert_project_agent(
    bb_root: &Path,
    mut registration: ProjectAgentRegistration,
) -> Result<ProjectAgentRegistration, InboxError> {
    registration.project = validate_project_ref(&registration.project)?.to_string();
    registration.agent = validate_agent_id(&registration.agent)?.to_string();
    if let Some(role) = registration.role.as_mut() {
        *role = role.trim().to_string();
        if role.is_empty() {
            registration.role = None;
        }
    }
    for lane in &registration.lanes {
        validate_lane_ref(lane)?;
    }

    let mut registry = load_registry_optional(bb_root)?.unwrap_or_else(default_registry);
    if !registry
        .agents
        .iter()
        .any(|agent| agent.id == registration.agent)
    {
        return Err(InboxError::InvalidInput(format!(
            "project `{}` references unknown agent `{}` in agents.toml",
            registration.project, registration.agent
        )));
    }

    if let Some(existing) = registry
        .project_agents
        .iter_mut()
        .find(|item| item.project == registration.project && item.agent == registration.agent)
    {
        *existing = registration.clone();
    } else {
        registry.project_agents.push(registration.clone());
    }
    registry
        .project_agents
        .sort_by(|a, b| (&a.project, &a.agent).cmp(&(&b.project, &b.agent)));
    validate_registry(&registry)?;
    write_registry(bb_root, &registry)?;
    Ok(registration)
}

pub fn remove_project_agent(
    bb_root: &Path,
    project: &str,
    agent: &str,
) -> Result<RemovedProjectAgentRegistration, InboxError> {
    let project = validate_project_ref(project)?.to_string();
    let agent = validate_agent_id(agent)?.to_string();
    let mut registry = load_registry_optional(bb_root)?.unwrap_or_else(default_registry);
    let before = registry.project_agents.len();
    registry
        .project_agents
        .retain(|item| !(item.project == project && item.agent == agent));
    let removed = registry.project_agents.len() != before;
    validate_registry(&registry)?;
    write_registry(bb_root, &registry)?;
    Ok(RemovedProjectAgentRegistration {
        project,
        agent,
        removed,
    })
}

pub fn validate_assignee_for_project(
    bb_root: &Path,
    project: &str,
    assignee: &str,
) -> Result<(), InboxError> {
    let assignee = assignee.trim();
    if assignee.is_empty() {
        return Ok(());
    }
    let Some(_) = load_registry_optional(bb_root)? else {
        return Ok(());
    };
    let list = list_project_agents(bb_root, project)?;
    if list.agents.iter().any(|item| item.agent.id == assignee) {
        Ok(())
    } else {
        Err(InboxError::InvalidInput(format!(
            "assignee `{assignee}` is not an active registered agent for project `{project}`"
        )))
    }
}

pub fn opencode_distribution_sources(
    bb_root: &Path,
) -> Result<Vec<(AgentProfile, PathBuf)>, InboxError> {
    let Some(registry) = load_registry_optional(bb_root)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for agent in registry.agents {
        validate_agent(&agent)?;
        if agent.runtime.as_deref() != Some("opencode") || !agent.distribute {
            continue;
        }
        let Some(source_path) = agent.source_path.as_deref() else {
            continue;
        };
        out.push((agent.clone(), resolve_source_path(bb_root, source_path)?));
    }
    out.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    Ok(out)
}

fn load_registry_optional(bb_root: &Path) -> Result<Option<AgentRegistryFile>, InboxError> {
    let path = registry_path(bb_root);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(InboxError::Io { path, source }),
    };
    let registry: AgentRegistryFile =
        toml::from_str(&content).map_err(|err| InboxError::InvalidProjectMeta {
            path: path.clone(),
            message: format!("failed to parse agents.toml: {err}"),
        })?;
    validate_registry(&registry)?;
    Ok(Some(registry))
}

fn default_registry() -> AgentRegistryFile {
    AgentRegistryFile {
        version: Some(1),
        agents: Vec::new(),
        project_agents: Vec::new(),
    }
}

fn write_registry(bb_root: &Path, registry: &AgentRegistryFile) -> Result<(), InboxError> {
    let path = registry_path(bb_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| InboxError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content =
        toml::to_string_pretty(registry).map_err(|err| InboxError::InvalidProjectMeta {
            path: path.clone(),
            message: format!("failed to serialize agents.toml: {err}"),
        })?;
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    fs::write(&tmp, content).map_err(|source| InboxError::Io {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, &path).map_err(|source| InboxError::Io { path, source })
}

#[cfg(test)]
mod tests;
