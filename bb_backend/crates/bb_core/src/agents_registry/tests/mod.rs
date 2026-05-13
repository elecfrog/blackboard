use std::collections::BTreeMap;

use super::*;
use tempfile::TempDir;

fn write_registry(root: &Path, body: &str) {
    fs::create_dir_all(root.join("agents")).unwrap();
    fs::write(root.join("agents/agents.toml"), body).unwrap();
}

/// Helper: build a minimal valid AgentProfile for testing.
fn test_agent(id: &str, display_name: &str, kind: &str) -> AgentProfile {
    AgentProfile {
        id: id.to_string(),
        display_name: display_name.to_string(),
        kind: kind.to_string(),
        runtime: None,
        scope: default_scope(),
        status: default_status(),
        assignable: true,
        distribute: false,
        source_path: None,
        roles: Vec::new(),
        description: None,
        model: None,
        variant: None,
        instructions: None,
        instructions_path: None,
        custom_env: BTreeMap::new(),
        custom_args: Vec::new(),
        max_concurrent_tasks: None,
        org_role: None,
        coordinator: None,
        workers: Vec::new(),
        mcp_servers: Vec::new(),
        skills: Vec::new(),
    }
}

#[test]
fn lists_global_and_project_agents() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
version = 1

[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
runtime = "codex"

[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"
runtime = "opencode"
scope = "project"

[[project_agents]]
project = "blackboard"
agent = "bb-pm"
role = "pm"
lanes = ["bbp"]
"#,
    );

    let list = list_project_agents(temp.path(), "blackboard").unwrap();
    let ids: Vec<_> = list
        .agents
        .iter()
        .map(|item| item.agent.id.as_str())
        .collect();
    assert_eq!(ids, vec!["bb-pm", "codex"]);
    assert_eq!(list.agents[0].project_role.as_deref(), Some("pm"));
}

#[test]
fn validates_assignee_when_registry_exists() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
"#,
    );

    validate_assignee_for_project(temp.path(), "blackboard", "codex").unwrap();
    assert!(validate_assignee_for_project(temp.path(), "blackboard", "ghost").is_err());
}

#[test]
fn missing_registry_does_not_block_legacy_writes() {
    let temp = TempDir::new().unwrap();
    validate_assignee_for_project(temp.path(), "blackboard", "ghost").unwrap();
}

#[test]
fn upserts_agent_and_project_registration() {
    let temp = TempDir::new().unwrap();
    let mut agent_profile = test_agent("codex", "Codex", "platform_agent");
    agent_profile.runtime = Some("codex".to_string());
    agent_profile.roles = vec!["coding".to_string()];
    agent_profile.description = Some("primary coding agent".to_string());

    let agent = upsert_agent(temp.path(), agent_profile).unwrap();
    assert_eq!(agent.id, "codex");

    let registration = upsert_project_agent(
        temp.path(),
        ProjectAgentRegistration {
            project: "blackboard".to_string(),
            agent: "codex".to_string(),
            role: Some("owner".to_string()),
            lanes: vec!["bbt".to_string()],
        },
    )
    .unwrap();
    assert_eq!(registration.agent, "codex");

    let list = list_project_agents(temp.path(), "blackboard").unwrap();
    assert_eq!(list.agents.len(), 1);
    assert_eq!(list.agents[0].project_role.as_deref(), Some("owner"));
}

#[test]
fn removes_project_registration_idempotently() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"

[[project_agents]]
project = "blackboard"
agent = "codex"
"#,
    );

    let removed = remove_project_agent(temp.path(), "blackboard", "codex").unwrap();
    assert!(removed.removed);
    let removed_again = remove_project_agent(temp.path(), "blackboard", "codex").unwrap();
    assert!(!removed_again.removed);
}

// ── New Profile field tests (Ticket #000048) ──

#[test]
fn parse_profile_with_model_variant_and_instructions() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"
model = "minimax/MiniMax-M2.7-highspeed"
variant = "xhigh"
instructions = "You are BB-PM, the project manager."
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let agent = &list.agents[0];
    assert_eq!(
        agent.model.as_deref(),
        Some("minimax/MiniMax-M2.7-highspeed")
    );
    assert_eq!(agent.variant.as_deref(), Some("xhigh"));
    assert_eq!(
        agent.instructions.as_deref(),
        Some("You are BB-PM, the project manager.")
    );
    assert!(agent.instructions_path.is_none());
}

#[test]
fn parse_profile_with_legacy_model_reasoning_effort_alias() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"
model_reasoning_effort = "xhigh"
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let agent = &list.agents[0];
    assert_eq!(agent.variant.as_deref(), Some("xhigh"));
}

#[test]
fn parse_profile_with_instructions_path() {
    let temp = TempDir::new().unwrap();
    // Create the instructions file
    fs::create_dir_all(temp.path().join("agents/prompts")).unwrap();
    fs::write(
        temp.path().join("agents/prompts/bb-pm.md"),
        "# BB PM\nYou are the PM.",
    )
    .unwrap();

    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"
instructions_path = "agents/prompts/bb-pm.md"
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let agent = &list.agents[0];
    assert_eq!(
        agent.instructions_path.as_deref(),
        Some("agents/prompts/bb-pm.md")
    );
    assert!(agent.instructions.is_none());
}

#[test]
fn validate_instructions_mutual_exclusion() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("bb-pm", "BB PM", "agent");
    agent.instructions = Some("inline".to_string());
    agent.instructions_path = Some("agents/prompts/bb-pm.md".to_string());

    let result = upsert_agent(temp.path(), agent);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("mutually exclusive"), "error: {err}");
}

#[test]
fn parse_profile_with_custom_env_and_args() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
custom_env = { BB_DAEMON = "1", OPENAI_API_KEY = "sk-test" }
custom_args = ["--timeout", "300"]
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let agent = &list.agents[0];
    assert_eq!(
        agent.custom_env.get("BB_DAEMON").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        agent.custom_env.get("OPENAI_API_KEY").map(String::as_str),
        Some("sk-test")
    );
    assert_eq!(agent.custom_args, vec!["--timeout", "300"]);
}

#[test]
fn parse_profile_with_max_concurrent_tasks() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
max_concurrent_tasks = 3
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    assert_eq!(list.agents[0].max_concurrent_tasks, Some(3));
}

#[test]
fn validate_max_concurrent_tasks_zero_rejected() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("codex", "Codex", "platform_agent");
    agent.max_concurrent_tasks = Some(0);

    let result = upsert_agent(temp.path(), agent);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("max_concurrent_tasks"), "error: {err}");
}

#[test]
fn parse_profile_with_org_role_coordinator() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"
org_role = "coordinator"
workers = ["codex", "codebuddy"]

[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"

[[agents]]
id = "codebuddy"
display_name = "CodeBuddy"
kind = "platform_agent"
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let pm = list.agents.iter().find(|a| a.id == "bb-pm").unwrap();
    assert_eq!(pm.org_role.as_deref(), Some("coordinator"));
    assert_eq!(pm.workers, vec!["codex", "codebuddy"]);
}

#[test]
fn parse_profile_with_org_role_worker() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "bb-pm"
display_name = "BB PM"
kind = "agent"

[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
org_role = "worker"
coordinator = "bb-pm"
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let codex = list.agents.iter().find(|a| a.id == "codex").unwrap();
    assert_eq!(codex.org_role.as_deref(), Some("worker"));
    assert_eq!(codex.coordinator.as_deref(), Some("bb-pm"));
}

#[test]
fn validate_worker_cannot_have_workers() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("codex", "Codex", "platform_agent");
    agent.org_role = Some("worker".to_string());
    agent.workers = vec!["other".to_string()];

    let result = upsert_agent(temp.path(), agent);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("worker cannot declare workers"),
        "error: {err}"
    );
}

#[test]
fn validate_workers_no_self_reference() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("bb-pm", "BB PM", "agent");
    agent.org_role = Some("coordinator".to_string());
    agent.workers = vec!["bb-pm".to_string()];

    let result = upsert_agent(temp.path(), agent);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("must not contain the agent itself"),
        "error: {err}"
    );
}

#[test]
fn validate_invalid_org_role_rejected() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("codex", "Codex", "platform_agent");
    agent.org_role = Some("manager".to_string());

    let result = upsert_agent(temp.path(), agent);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid org_role"), "error: {err}");
}

#[test]
fn backward_compat_no_new_fields() {
    let temp = TempDir::new().unwrap();
    write_registry(
        temp.path(),
        r#"
[[agents]]
id = "codex"
display_name = "Codex"
kind = "platform_agent"
runtime = "codex"
"#,
    );

    let list = list_agents(temp.path()).unwrap();
    let agent = &list.agents[0];
    assert_eq!(agent.id, "codex");
    assert!(agent.model.is_none());
    assert!(agent.variant.is_none());
    assert!(agent.instructions.is_none());
    assert!(agent.instructions_path.is_none());
    assert!(agent.custom_env.is_empty());
    assert!(agent.custom_args.is_empty());
    assert!(agent.max_concurrent_tasks.is_none());
    assert!(agent.org_role.is_none());
    assert!(agent.coordinator.is_none());
    assert!(agent.workers.is_empty());
}

#[test]
fn upsert_agent_with_full_profile() {
    let temp = TempDir::new().unwrap();
    let mut agent = test_agent("bb-pm", "BB PM", "agent");
    agent.model = Some("minimax/MiniMax-M2.7-highspeed".to_string());
    agent.variant = Some("xhigh".to_string());
    agent.instructions = Some("You are BB-PM.".to_string());
    agent.org_role = Some("coordinator".to_string());
    agent.custom_env = BTreeMap::from([("BB_DAEMON".to_string(), "1".to_string())]);
    agent.custom_args = vec!["--timeout".to_string(), "300".to_string()];
    agent.max_concurrent_tasks = Some(3);

    let result = upsert_agent(temp.path(), agent).unwrap();
    assert_eq!(
        result.model.as_deref(),
        Some("minimax/MiniMax-M2.7-highspeed")
    );
    assert_eq!(result.variant.as_deref(), Some("xhigh"));
    assert_eq!(result.instructions.as_deref(), Some("You are BB-PM."));
    assert_eq!(result.org_role.as_deref(), Some("coordinator"));
    assert_eq!(result.max_concurrent_tasks, Some(3));
    assert_eq!(
        result.custom_env.get("BB_DAEMON").map(String::as_str),
        Some("1")
    );
    assert_eq!(result.custom_args, vec!["--timeout", "300"]);

    // Verify round-trip: read back from disk
    let list = list_agents(temp.path()).unwrap();
    let reloaded = &list.agents[0];
    assert_eq!(
        reloaded.model.as_deref(),
        Some("minimax/MiniMax-M2.7-highspeed")
    );
    assert_eq!(reloaded.variant.as_deref(), Some("xhigh"));
    assert_eq!(reloaded.org_role.as_deref(), Some("coordinator"));
}
