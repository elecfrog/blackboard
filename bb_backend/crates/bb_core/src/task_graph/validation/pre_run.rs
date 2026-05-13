use std::path::Path;

use super::super::store;
use super::super::types::*;
use super::validate_graph;
use crate::agents_registry;

// ─── Pre-run integrity validation ────────────────────────────────────────────

/// 运行前完整性校验。
///
/// 在创建 run 之前调用，确保图所有运行时依赖都就绪：
/// 1. graph 结构校验（调用 validate_graph）
/// 2. llm 节点引用的 prompt_file 必须存在（如有）
/// 3. sub_graph 节点引用的子图必须可加载
/// 4. llm 节点引用的 agent 必须已注册
///
/// 返回空 vec 表示校验通过。
pub fn validate_pre_run(
    def: &TaskGraphDefinition,
    workspace_root: &Path,
    project: &str,
) -> Vec<TaskGraphValidationError> {
    // 第一层：结构校验
    let mut errors = validate_graph(def);

    // Load project-assignable profiles for agent-mode validation.
    let project_agents: Vec<agents_registry::AgentProfile> =
        agents_registry::list_project_agents(workspace_root, project)
            .map(|list| list.agents.into_iter().map(|a| a.agent).collect())
            .unwrap_or_default();

    // 第二层：运行时依赖校验
    for (i, node) in def.nodes.iter().enumerate() {
        match node.node_type {
            NodeType::Llm => {
                validate_llm_runtime(node, i, workspace_root, &project_agents, &mut errors);
            }
            NodeType::SubGraph => {
                validate_sub_graph_runtime(node, i, workspace_root, project, &mut errors);
            }
            _ => {}
        }
    }

    errors
}

/// 校验 llm 节点的运行时依赖：prompt template 非空，agent 已注册。
fn validate_llm_runtime(
    node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    project_agents: &[agents_registry::AgentProfile],
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<LlmConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(_) => return,
    };

    match config.run_as {
        LlmRunAs::Llm => validate_inline_llm_runtime(node, idx, workspace_root, &config, errors),
        LlmRunAs::Agent => {
            validate_agent_llm_runtime(node, idx, workspace_root, &config, project_agents, errors)
        }
    }
}

fn validate_inline_llm_runtime(
    node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    config: &LlmConfig,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if config.prompt.template.trim().is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.prompt.template", idx),
            code: "empty_prompt_template".to_string(),
            message: format!("LLM 节点 '{}' 的 prompt template 为空", node.id),
        });
        return;
    }

    if config.prompt.mode == "file" {
        let prompt_path = workspace_root.join(&config.prompt.template);
        if !prompt_path.is_file() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{}].config.prompt.template", idx),
                code: "prompt_file_not_found".to_string(),
                message: format!(
                    "LLM node '{}' references missing prompt file '{}'",
                    node.id, config.prompt.template
                ),
            });
        }
    }
}

fn validate_agent_llm_runtime(
    node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    config: &LlmConfig,
    project_agents: &[agents_registry::AgentProfile],
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let profile_id = config
        .agent_profile
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if profile_id.is_empty() {
        return;
    }
    let Some(profile) = project_agents.iter().find(|agent| agent.id == profile_id) else {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_profile_not_found".to_string(),
            message: format!(
                "LLM node '{}' references agent profile '{}' which is not active/assignable for this project",
                node.id, profile_id
            ),
        });
        return;
    };
    let runtime = profile.runtime.as_deref().unwrap_or_default().trim();
    if runtime.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_profile_missing_runtime".to_string(),
            message: format!(
                "Agent profile '{}' used by node '{}' does not define runtime",
                profile_id, node.id
            ),
        });
    } else if !KNOWN_RUNTIMES.contains(&runtime) {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_profile_runtime_not_found".to_string(),
            message: format!(
                "Agent profile '{}' runtime '{}' used by node '{}' is not a known runtime",
                profile_id, runtime, node.id
            ),
        });
    }
    if has_node_task_prompt(config) {
        validate_node_prompt_file_if_needed(node, idx, workspace_root, config, errors);
        return;
    }

    let Some(instructions_path) = profile
        .instructions_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_task_prompt_missing".to_string(),
            message: format!(
                "Agent node '{}' has empty task prompt and profile '{}' does not define instructions_path",
                node.id, profile_id
            ),
        });
        return;
    };
    validate_profile_default_prompt(
        node,
        idx,
        workspace_root,
        profile_id,
        instructions_path,
        errors,
    );
}

fn has_node_task_prompt(config: &LlmConfig) -> bool {
    !config.prompt.template.trim().is_empty()
}

fn validate_node_prompt_file_if_needed(
    node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    config: &LlmConfig,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if config.prompt.mode != "file" {
        return;
    }
    let prompt_path = workspace_root.join(&config.prompt.template);
    if !prompt_path.is_file() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.prompt.template", idx),
            code: "prompt_file_not_found".to_string(),
            message: format!(
                "LLM node '{}' references missing prompt file '{}'",
                node.id, config.prompt.template
            ),
        });
    }
}

fn validate_profile_default_prompt(
    _node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    profile_id: &str,
    instructions_path: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let prompt_path = workspace_root.join(instructions_path);
    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    match prompt_path.canonicalize() {
        Ok(canonical_path) if canonical_path.starts_with(&canonical_root) => {}
        Ok(_) => errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_profile_prompt_escapes_root".to_string(),
            message: format!(
                "Agent profile '{}' instructions_path escapes workspace root",
                profile_id
            ),
        }),
        Err(_) => errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.agent_profile", idx),
            code: "agent_profile_prompt_not_found".to_string(),
            message: format!(
                "Agent profile '{}' instructions_path '{}' does not exist",
                profile_id, instructions_path
            ),
        }),
    }
}

/// 校验 sub_graph 节点的运行时依赖：子图可加载。
fn validate_sub_graph_runtime(
    node: &TaskGraphNode,
    idx: usize,
    workspace_root: &Path,
    project: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<SubGraphConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(_) => return,
    };

    if config.graph_id.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.graph_id", idx),
            code: "empty_graph_id".to_string(),
            message: format!("SubGraph 节点 '{}' 的 graph_id 为空", node.id),
        });
        return;
    }

    // 按 scope 优先级尝试加载子图
    let child_graph = match config.graph_scope {
        TaskGraphScope::System => store::read_system_graph(workspace_root, &config.graph_id)
            .or_else(|_| store::read_project_graph(workspace_root, project, &config.graph_id)),
        TaskGraphScope::Project => {
            store::read_project_graph(workspace_root, project, &config.graph_id)
                .or_else(|_| store::read_system_graph(workspace_root, &config.graph_id))
        }
    };

    if child_graph.is_err() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.graph_id", idx),
            code: "sub_graph_not_found".to_string(),
            message: format!(
                "SubGraph 节点 '{}' 引用的子图 '{}' (scope={:?}) 无法加载",
                node.id, config.graph_id, config.graph_scope
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn graph_with_agent_node(prompt: serde_json::Value) -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "agent-prompt-test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Agent prompt test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![
                TaskGraphNode {
                    id: "start".to_string(),
                    node_type: NodeType::Start,
                    label: "Start".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({}),
                    pins: Vec::new(),
                },
                TaskGraphNode {
                    id: "agent".to_string(),
                    node_type: NodeType::Llm,
                    label: "Agent".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({
                        "run_as": "agent",
                        "agent_profile": "bb-pm",
                        "prompt": prompt
                    }),
                    pins: Vec::new(),
                },
                TaskGraphNode {
                    id: "end".to_string(),
                    node_type: NodeType::End,
                    label: "End".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({ "result": "succeeded" }),
                    pins: Vec::new(),
                },
            ],
            edges: vec![
                TaskGraphEdge {
                    id: "start-agent".to_string(),
                    from: "start".to_string(),
                    to: "agent".to_string(),
                    kind: EdgeKind::Exec,
                    label: None,
                    source_handle: None,
                    target_handle: None,
                    from_pin: None,
                    to_pin: None,
                },
                TaskGraphEdge {
                    id: "agent-end".to_string(),
                    from: "agent".to_string(),
                    to: "end".to_string(),
                    kind: EdgeKind::Exec,
                    label: None,
                    source_handle: None,
                    target_handle: None,
                    from_pin: None,
                    to_pin: None,
                },
            ],
            layout: None,
        }
    }

    fn write_profile_without_prompt(root: &std::path::Path) {
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(
            root.join("agents/agents.toml"),
            r#"
[[agents]]
id = "bb-pm"
display_name = "BBPM"
kind = "opencode_agent"
runtime = "opencode"
"#,
        )
        .unwrap();
    }

    #[test]
    fn agent_mode_without_node_or_profile_prompt_fails_pre_run() {
        let tmp = TempDir::new().unwrap();
        write_profile_without_prompt(tmp.path());
        let graph = graph_with_agent_node(serde_json::json!({
            "mode": "inline",
            "template": ""
        }));

        let errors = validate_pre_run(&graph, tmp.path(), "blackboard");

        assert!(errors
            .iter()
            .any(|error| error.code == "agent_task_prompt_missing"));
    }

    #[test]
    fn agent_mode_with_inline_task_prompt_does_not_require_profile_default() {
        let tmp = TempDir::new().unwrap();
        write_profile_without_prompt(tmp.path());
        let graph = graph_with_agent_node(serde_json::json!({
            "mode": "inline",
            "template": "audit tickets"
        }));

        let errors = validate_pre_run(&graph, tmp.path(), "blackboard");

        assert!(
            errors.is_empty(),
            "expected no validation errors, got: {:?}",
            errors
        );
    }
}
