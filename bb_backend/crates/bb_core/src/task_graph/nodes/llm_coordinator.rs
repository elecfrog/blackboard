use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde_json::{json, Map, Value};

use crate::fs_util::resolve_slash;
use crate::task_graph::definition::types::{
    EdgeKind, LlmConfig, LlmCoordinatorConfig, NodeType, TaskGraphDefinition, TaskGraphEdge,
    TaskGraphError, TaskGraphNode, TaskGraphScope, TaskGraphValidationError,
};
use crate::task_graph::definition::upgrade::upgrade_graph;
use crate::task_graph::nodes::{eval, runtime};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::{execute_run, RunOutcome, RunnerOptions};
use crate::task_graph::pregel::{child_checkpoint_namespace, DEFAULT_CHECKPOINT_NAMESPACE};
use crate::task_graph::run_state::{
    self, GraphRef, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::validation::{decode_graph_value_at, validate_graph, validate_pre_run};

pub fn execute_llm_coordinator_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;
    let start_time = Utc::now().to_rfc3339();
    let started = Instant::now();

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!("[{start_time}] llm_coordinator node starting"),
    )?;
    run_state::update_node_state(
        ws,
        project,
        run_id,
        &node_state(
            node,
            NodeRunStatus::Running,
            start_time.clone(),
            None,
            None,
            None,
            None,
            Some("llm_coordinator node starting".to_string()),
        ),
    )?;

    let config: LlmCoordinatorConfig = match serde_json::from_value(node.config.clone()) {
        Ok(config) => config,
        Err(source) => {
            return Ok(failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_config",
                format!("LLM Coordinator node config parse error: {source}"),
            ));
        }
    };

    let child_graph = match generate_validated_subgraph(opts, node, run, edge_map, &config)? {
        Ok(graph) => graph,
        Err(error) => {
            return Ok(failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                error.code(),
                error.message().to_string(),
            ));
        }
    };

    let validation_errors = validate_graph(&child_graph);
    if !validation_errors.is_empty() {
        return Ok(failed_outcome(
            node,
            start_time,
            started.elapsed().as_millis() as u64,
            "validate_subgraph_failed",
            validation_error_message(&validation_errors),
        ));
    }

    let child_input = resolve_subgraph_input(&config, run, project, ws, &opts.scripts_dir);
    let graph_ref = GraphRef {
        scope: TaskGraphScope::Project,
        id: child_graph.id.clone(),
        version: child_graph.version,
    };
    let mut child_run = run_state::create_run(ws, project, graph_ref, &child_graph, child_input)?;
    child_run.parent_run_id = Some(run_id.clone());
    child_run.checkpoint_ns = Some(child_checkpoint_namespace(
        run.checkpoint_ns
            .as_deref()
            .unwrap_or(DEFAULT_CHECKPOINT_NAMESPACE),
        &node.id,
    ));
    run_state::write_run_json(ws, project, &child_run.id, &child_run)?;
    let child_run_id = child_run.id;

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] execute_subgraph child_run_id={} graph_id={}",
            Utc::now().to_rfc3339(),
            child_run_id,
            child_graph.id
        ),
    )?;
    run_state::update_node_state(
        ws,
        project,
        run_id,
        &TaskGraphRunNode {
            child_run_id: Some(child_run_id.clone()),
            ..node_state(
                node,
                NodeRunStatus::Running,
                start_time.clone(),
                None,
                None,
                None,
                None,
                Some(format!("executing isolated subgraph run {child_run_id}")),
            )
        },
    )?;

    let child_opts = RunnerOptions {
        workspace_root: opts.workspace_root.clone(),
        scripts_dir: opts.scripts_dir.clone(),
        project: project.clone(),
        run_id: child_run_id.clone(),
        codex_path: opts.codex_path.clone(),
        codebuddy_path: opts.codebuddy_path.clone(),
        opencode_path: opts.opencode_path.clone(),
        opencode_config_content: opts.opencode_config_content.clone(),
        pi_path: opts.pi_path.clone(),
        model: opts.model.clone(),
        node_timeout: opts.node_timeout,
        run_timeout: opts.run_timeout,
        dry_run: opts.dry_run,
        custom_env: opts.custom_env.clone(),
        custom_args: opts.custom_args.clone(),
        mcp_servers: opts.mcp_servers.clone(),
        skills: opts.skills.clone(),
    };

    let child_outcome = execute_run(&child_opts)?;
    let end_time = Utc::now().to_rfc3339();
    let duration_ms = started.elapsed().as_millis() as u64;

    match child_outcome {
        RunOutcome::Succeeded => {
            let child_detail = run_state::read_run_detail(ws, project, &child_run_id)?;
            let child_output = collect_child_output(&child_detail);
            let result = json!({
                "status": "completed",
                "subgraph_run_id": child_run_id,
                "audit_ref": {
                    "project": project,
                    "run_id": child_run_id,
                    "graph_id": child_graph.id,
                },
                "outputs": child_output,
            });
            Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                output: Some(result),
                node_state: TaskGraphRunNode {
                    child_run_id: Some(child_run_id.clone()),
                    ..node_state(
                        node,
                        NodeRunStatus::Succeeded,
                        start_time,
                        Some(end_time),
                        Some(duration_ms),
                        Some(0),
                        None,
                        Some(format!("subgraph run {child_run_id} completed")),
                    )
                },
                side_effects: vec![],
                child_run_id: Some(child_run_id),
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            })
        }
        RunOutcome::Failed {
            node_id: failed_node,
            message,
        } => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                child_run_id: Some(child_run_id.clone()),
                ..node_state(
                    node,
                    NodeRunStatus::Failed,
                    start_time,
                    Some(end_time),
                    Some(duration_ms),
                    None,
                    Some(NodeError {
                        code: "execute_subgraph_failed".to_string(),
                        message: format!(
                            "Subgraph run '{child_run_id}' failed at node '{failed_node}': {message}"
                        ),
                    }),
                    Some(format!("subgraph run {child_run_id} failed: {message}")),
                )
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
        RunOutcome::Paused { node_id } => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Paused,
            output: None,
            node_state: TaskGraphRunNode {
                child_run_id: Some(child_run_id.clone()),
                ..node_state(
                    node,
                    NodeRunStatus::Paused,
                    start_time,
                    None,
                    None,
                    None,
                    None,
                    Some(format!(
                        "subgraph run {child_run_id} paused at {node_id}"
                    )),
                )
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
        RunOutcome::Cancelled => Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state: TaskGraphRunNode {
                child_run_id: Some(child_run_id.clone()),
                ..node_state(
                    node,
                    NodeRunStatus::Failed,
                    start_time,
                    Some(end_time),
                    Some(duration_ms),
                    None,
                    Some(NodeError {
                        code: "execute_subgraph_cancelled".to_string(),
                        message: format!("Subgraph run '{child_run_id}' was cancelled"),
                    }),
                    Some(format!("subgraph run {child_run_id} cancelled")),
                )
            },
            side_effects: vec![],
            child_run_id: Some(child_run_id),
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        }),
    }
}

fn generate_validated_subgraph(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    config: &LlmCoordinatorConfig,
) -> Result<Result<TaskGraphDefinition, CoordinatorDraftError>, TaskGraphError> {
    if let Some(value) = &config.static_subgraph {
        let raw = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
        return Ok(validate_subgraph_for_coordinator(&raw, config, opts, run)
            .map_err(|failure| CoordinatorDraftError::Validate(failure.message())));
    }

    let max_attempts = config.coordinator.max_repair_attempts.max(1);
    let mut feedback: Option<SubgraphRepairFeedback> = None;

    for attempt in 1..=max_attempts {
        run_state::append_node_log(
            &opts.workspace_root,
            &opts.project,
            &opts.run_id,
            &node.id,
            &format!(
                "[{}] generate_subgraph attempt {}/{}",
                Utc::now().to_rfc3339(),
                attempt,
                max_attempts
            ),
        )?;

        let mut llm_node = node.clone();
        llm_node.node_type = NodeType::Llm;
        let llm_config = llm_config_for_coordinator_attempt(opts, config, run, feedback.as_ref())?;
        llm_node.config = serde_json::to_value(llm_config).unwrap_or_default();

        let outcome = runtime::execute_llm_node(opts, &llm_node, run, edge_map)?;
        if outcome.status != NodeRunStatus::Succeeded {
            return Ok(Err(CoordinatorDraftError::Generate(
                outcome.node_state.error.map_or_else(
                    || "LLM draft generation failed".to_string(),
                    |error| error.message,
                ),
            )));
        }

        let raw = raw_draft_from_llm_outcome(opts, run, &outcome)?;
        match validate_subgraph_for_coordinator(&raw, config, opts, run) {
            Ok(graph) => return Ok(Ok(graph)),
            Err(failure) => {
                let tool_result = failure.tool_result();
                run_state::append_node_log(
                    &opts.workspace_root,
                    &opts.project,
                    &opts.run_id,
                    &node.id,
                    &format!(
                        "[{}] validate_subgraph static pass failed\n{}",
                        Utc::now().to_rfc3339(),
                        serde_json::to_string_pretty(&tool_result)
                            .unwrap_or_else(|_| tool_result.to_string())
                    ),
                )?;

                if attempt == max_attempts {
                    return Ok(Err(CoordinatorDraftError::Validate(format!(
                        "subgraph static validation failed after {attempt} attempt(s): {}",
                        failure.message()
                    ))));
                }

                feedback = Some(SubgraphRepairFeedback {
                    failure,
                    previous_output: raw,
                });
            }
        }
    }

    Ok(Err(CoordinatorDraftError::Validate(
        "subgraph static validation exhausted all attempts".to_string(),
    )))
}

#[derive(Debug, Clone)]
enum CoordinatorDraftError {
    Generate(String),
    Validate(String),
}

impl CoordinatorDraftError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Generate(_) => "generate_subgraph_failed",
            Self::Validate(_) => "validate_subgraph_failed",
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Generate(message) | Self::Validate(message) => message,
        }
    }
}

#[derive(Debug, Clone)]
struct SubgraphRepairFeedback {
    failure: SubgraphPassFailure,
    previous_output: String,
}

#[derive(Debug, Clone)]
struct SubgraphPassFailure {
    stage: &'static str,
    errors: Vec<TaskGraphValidationError>,
}

impl SubgraphPassFailure {
    const fn new(stage: &'static str, errors: Vec<TaskGraphValidationError>) -> Self {
        Self { stage, errors }
    }

    fn tool_result(&self) -> Value {
        json!({
            "status": "failed",
            "stage": self.stage,
            "errors": self.errors,
            "instruction": "Return exactly one complete TaskGraphDefinition JSON object. Do not include Markdown fences, prose, or patches. Fix only the reported validation errors and preserve the original task intent."
        })
    }

    fn message(&self) -> String {
        format!(
            "{} pass failed: {}",
            self.stage,
            validation_error_message(&self.errors)
        )
    }
}

fn validate_subgraph_static_passes(raw: &str) -> Result<TaskGraphDefinition, SubgraphPassFailure> {
    let value = serde_json::from_str::<Value>(raw.trim()).map_err(|source| {
        SubgraphPassFailure::new(
            "json_syntax",
            vec![TaskGraphValidationError {
                path: "$".to_string(),
                code: "invalid_json".to_string(),
                message: format!("Invalid JSON: {source}"),
            }],
        )
    })?;

    let (graph_value, path_prefix) = graph_value_for_static_passes(value);
    let mut graph = decode_graph_value_at(graph_value, path_prefix)
        .map_err(|errors| SubgraphPassFailure::new("task_graph_schema", errors))?;
    upgrade_graph(&mut graph);

    let errors = validate_graph(&graph);
    if errors.is_empty() {
        Ok(graph)
    } else {
        Err(SubgraphPassFailure::new("graph_semantic", errors))
    }
}

fn validate_subgraph_for_coordinator(
    raw: &str,
    config: &LlmCoordinatorConfig,
    opts: &RunnerOptions,
    run: &TaskGraphRun,
) -> Result<TaskGraphDefinition, SubgraphPassFailure> {
    let graph = validate_subgraph_static_passes(raw)?;
    let mut errors = validate_subgraph_policy_errors(&graph, config);
    let requested_output_paths = requested_output_paths_from_run(run);
    validate_child_llm_output_path_policy(
        &graph,
        &opts.workspace_root,
        &requested_output_paths,
        &mut errors,
    );
    errors.extend(validate_pre_run(
        &graph,
        &opts.workspace_root,
        &opts.project,
    ));
    if errors.is_empty() {
        Ok(graph)
    } else {
        Err(SubgraphPassFailure::new("graph_semantic", errors))
    }
}

fn graph_value_for_static_passes(value: Value) -> (Value, &'static str) {
    let Value::Object(mut object) = value else {
        return (value, "");
    };
    if let Some(subgraph) = object.remove("subgraph") {
        return (subgraph, "subgraph");
    }
    if let Some(graph) = object.remove("graph") {
        return (graph, "graph");
    }
    (Value::Object(object), "")
}

fn llm_config_for_coordinator_attempt(
    opts: &RunnerOptions,
    config: &LlmCoordinatorConfig,
    run: &TaskGraphRun,
    feedback: Option<&SubgraphRepairFeedback>,
) -> Result<LlmConfig, TaskGraphError> {
    let mut llm = llm_config_with_coordinator_input(config, run);
    if let Some(feedback) = feedback {
        let base_template = prompt_template_for_repair(opts, &llm)?;
        llm.prompt.mode = "inline".to_string();
        llm.prompt.template = format!(
            "{}\n\n{}",
            base_template,
            coordinator_repair_instruction(feedback)
        );
    }

    // Coordinator generation needs the raw model text so JSON syntax can be
    // checked strictly before any permissive parser or schema pass runs.
    llm.output = Some(json!({ "artifact_type": "text", "required": true }));
    Ok(llm)
}

fn prompt_template_for_repair(
    opts: &RunnerOptions,
    llm: &LlmConfig,
) -> Result<String, TaskGraphError> {
    if llm.prompt.mode == "file" {
        let path = opts.workspace_root.join(&llm.prompt.template);
        return fs::read_to_string(&path).map_err(|source| TaskGraphError::Io { path, source });
    }
    Ok(llm.prompt.template.clone())
}

fn coordinator_repair_instruction(feedback: &SubgraphRepairFeedback) -> String {
    let tool_result = serde_json::to_string_pretty(&feedback.failure.tool_result())
        .unwrap_or_else(|_| feedback.failure.tool_result().to_string());
    let previous_output = truncate_chars(&feedback.previous_output, 12_000);

    format!(
        r"The previous generate_subgraph result failed static validation.

Treat the following validation result like a tool response. The system will not fix or normalize the graph for you.

Repair rules:
- Return exactly one complete TaskGraphDefinition JSON object.
- Do not return Markdown fences, prose, comments, or a patch/diff.
- Preserve the original task intent; only fix the validation errors.
- The result will be checked in this order: json_syntax, task_graph_schema, graph_semantic.

Validation result:
```json
{tool_result}
```

Previous output:
```text
{previous_output}
```"
    )
}

fn raw_draft_from_llm_outcome(
    opts: &RunnerOptions,
    run: &TaskGraphRun,
    outcome: &NodeOutcome,
) -> Result<String, TaskGraphError> {
    if let Some(artifact) = outcome.node_state.output_artifact.as_ref() {
        let path = run_relative_path(&opts.workspace_root, &run.project, &run.id, &artifact.path);
        if path.exists() {
            return fs::read_to_string(&path).map_err(|source| TaskGraphError::Io {
                path: path.clone(),
                source,
            });
        }
    }

    Ok(match outcome.output.as_ref() {
        Some(Value::String(text)) => text.clone(),
        Some(value) => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
        None => String::new(),
    })
}

fn run_relative_path(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    relative: &str,
) -> PathBuf {
    workspace_root
        .join("runtime")
        .join("task_graph_runs")
        .join(project)
        .join(run_id)
        .join(relative)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            output.push_str("\n...[truncated]");
            break;
        }
        output.push(ch);
    }
    output
}

fn non_empty_string(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn validate_subgraph_policy_errors(
    graph: &TaskGraphDefinition,
    config: &LlmCoordinatorConfig,
) -> Vec<TaskGraphValidationError> {
    let mut errors = Vec::new();
    if graph.nodes.len() > config.coordinator.max_nodes {
        errors.push(TaskGraphValidationError {
            path: "nodes".to_string(),
            code: "max_nodes_exceeded".to_string(),
            message: format!(
                "Subgraph has {} nodes, exceeding coordinator.max_nodes={}",
                graph.nodes.len(),
                config.coordinator.max_nodes
            ),
        });
    }
    if graph.edges.len() > config.coordinator.max_edges {
        errors.push(TaskGraphValidationError {
            path: "edges".to_string(),
            code: "max_edges_exceeded".to_string(),
            message: format!(
                "Subgraph has {} edges, exceeding coordinator.max_edges={}",
                graph.edges.len(),
                config.coordinator.max_edges
            ),
        });
    }
    if !config.coordinator.allowed_node_types.is_empty() {
        for (idx, node) in graph.nodes.iter().enumerate() {
            if !config
                .coordinator
                .allowed_node_types
                .contains(&node.node_type)
            {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].type"),
                    code: "node_type_disallowed".to_string(),
                    message: format!(
                        "Subgraph node '{}' uses disallowed type {:?}",
                        node.id, node.node_type
                    ),
                });
            }
        }
    }

    validate_child_llm_runtime_policy(graph, config, &mut errors);
    errors
}

fn validate_child_llm_runtime_policy(
    graph: &TaskGraphDefinition,
    config: &LlmCoordinatorConfig,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let parent_runtime = non_empty_string(&config.llm.runtime).unwrap_or("opencode");
    let parent_agent = non_empty_string(&config.llm.agent).unwrap_or("native");
    let parent_model = config.llm.model.as_ref().and_then(|model| {
        let trimmed = model.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    });

    for (idx, node) in graph.nodes.iter().enumerate() {
        if !matches!(node.node_type, NodeType::Llm | NodeType::LlmCoordinator) {
            continue;
        }
        let Ok(child) = serde_json::from_value::<LlmConfig>(node.config.clone()) else {
            continue;
        };
        let child_runtime = non_empty_string(&child.runtime).unwrap_or("");
        if child_runtime != parent_runtime {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.runtime"),
                code: "runtime_policy_mismatch".to_string(),
                message: format!(
                    "Child LLM node '{}' runtime '{}' must use coordinator runtime '{}'",
                    node.id, child_runtime, parent_runtime
                ),
            });
        }

        let child_agent = non_empty_string(&child.agent).unwrap_or("");
        if child_agent != parent_agent {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.agent"),
                code: "agent_policy_mismatch".to_string(),
                message: format!(
                    "Child LLM node '{}' agent '{}' must use coordinator agent '{}'",
                    node.id, child_agent, parent_agent
                ),
            });
        }

        if let Some(parent_model) = parent_model {
            let child_model = child
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .unwrap_or("");
            if child_model != parent_model {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.model"),
                    code: "model_policy_mismatch".to_string(),
                    message: format!(
                        "Child LLM node '{}' model '{}' must use coordinator model '{}'",
                        node.id, child_model, parent_model
                    ),
                });
            }
        }
    }
}

fn validate_child_llm_output_path_policy(
    graph: &TaskGraphDefinition,
    workspace_root: &Path,
    requested_output_paths: &[String],
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let synthesize_nodes = synthesize_node_ids(graph);
    let requested_basenames: Vec<String> = requested_output_paths
        .iter()
        .filter_map(|path| output_basename(path))
        .collect();

    for (idx, node) in graph.nodes.iter().enumerate() {
        if node.node_type != NodeType::Llm {
            continue;
        }
        let Ok(child) = serde_json::from_value::<LlmConfig>(node.config.clone()) else {
            continue;
        };
        let prompt = child.prompt.template.trim();
        let path_refs = extract_markdown_path_refs(prompt);
        let is_synthesize = synthesize_nodes.iter().any(|id| id == &node.id);

        if !is_synthesize {
            if !path_refs.is_empty() {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.prompt.template"),
                    code: "output_path_on_non_synthesize_node".to_string(),
                    message: format!(
                        "Child LLM node '{}' mentions markdown output path(s) {:?}; only the final synthesize node may write the requested report file",
                        node.id, path_refs
                    ),
                });
            }

            if is_scout_like_node(node)
                && prompt_references_graph_input(prompt)
                && !prompt_has_no_write_guard(prompt)
            {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.prompt.template"),
                    code: "scout_prompt_missing_no_write_guard".to_string(),
                    message: format!(
                        "Scout node '{}' passes through inputs.input and must explicitly say not to write/save files; scouts should return markdown artifacts only, while synthesize writes the final report",
                        node.id
                    ),
                });
            }
            continue;
        }

        for path_ref in &path_refs {
            if !output_path_is_workspace_scoped(path_ref, workspace_root) {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.prompt.template"),
                    code: "output_path_not_workspace_scoped".to_string(),
                    message: format!(
                        "Synthesize node '{}' output path '{}' must be anchored under the TaskGraph workspace using '{{{{env.workspace}}}}/file.md' or an absolute path inside '{}'",
                        node.id,
                        path_ref,
                        resolve_slash(workspace_root)
                    ),
                });
            }
        }

        for basename in &requested_basenames {
            let has_workspace_scoped_requested_path = path_refs.iter().any(|path_ref| {
                output_basename(path_ref).as_deref() == Some(basename.as_str())
                    && output_path_is_workspace_scoped(path_ref, workspace_root)
            });
            if !has_workspace_scoped_requested_path {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.prompt.template"),
                    code: "requested_output_path_missing_or_relative".to_string(),
                    message: format!(
                        "Synthesize node '{}' must explicitly write requested report '{}' to '{{{{env.workspace}}}}/{}'; relative paths are not allowed",
                        node.id, basename, basename
                    ),
                });
            }
        }
    }
}

fn synthesize_node_ids(graph: &TaskGraphDefinition) -> Vec<String> {
    let mut ids = Vec::new();
    for node in &graph.nodes {
        if is_synthesize_like_node(node) || is_join_before_end(graph, node) {
            ids.push(node.id.clone());
        }
    }
    ids
}

fn is_synthesize_like_node(node: &TaskGraphNode) -> bool {
    let id = node.id.to_ascii_lowercase();
    let label = node.label.to_ascii_lowercase();
    id.contains("synth")
        || id.contains("final")
        || id.contains("merge")
        || label.contains("synth")
        || label.contains("final")
        || label.contains("merge")
        || node.label.contains("综合")
        || node.label.contains("整合")
        || node.label.contains("汇总")
}

fn is_scout_like_node(node: &TaskGraphNode) -> bool {
    let id = node.id.to_ascii_lowercase();
    let label = node.label.to_ascii_lowercase();
    id.contains("scout")
        || id.contains("research")
        || label.contains("scout")
        || label.contains("research")
        || node.label.contains("调研")
}

fn is_join_before_end(graph: &TaskGraphDefinition, node: &TaskGraphNode) -> bool {
    if node.node_type != NodeType::Llm {
        return false;
    }
    let has_end_edge = graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Exec
            && edge.from == node.id
            && graph
                .nodes
                .iter()
                .any(|candidate| candidate.id == edge.to && candidate.node_type == NodeType::End)
    });
    if !has_end_edge {
        return false;
    }
    let incoming_llm_count = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Exec && edge.to == node.id)
        .filter(|edge| {
            graph
                .nodes
                .iter()
                .any(|candidate| candidate.id == edge.from && candidate.node_type == NodeType::Llm)
        })
        .count();
    incoming_llm_count >= 2
}

fn requested_output_paths_from_run(run: &TaskGraphRun) -> Vec<String> {
    coordinator_input_value(run)
        .map(|value| markdown_path_refs_from_value(&value))
        .unwrap_or_default()
}

fn markdown_path_refs_from_value(value: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_markdown_path_refs(value, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_markdown_path_refs(value: &Value, refs: &mut Vec<String>) {
    match value {
        Value::String(text) => refs.extend(extract_markdown_path_refs(text)),
        Value::Array(items) => {
            for item in items {
                collect_markdown_path_refs(item, refs);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                collect_markdown_path_refs(value, refs);
            }
        }
        _ => {}
    }
}

fn extract_markdown_path_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for token in text.split(path_token_separator) {
        let token = token
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    '`' | '"'
                        | '\''
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '。'
                        | '，'
                        | ','
                        | ';'
                        | '；'
                )
            })
            .trim();
        let Some(md_end) = token.to_ascii_lowercase().find(".md") else {
            continue;
        };
        let end = md_end + ".md".len();
        if end <= token.len() {
            refs.push(token[..end].to_string());
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

const fn path_token_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '，' | '。' | ',' | ';' | '；' | '\n' | '\r' | '\t')
}

fn output_basename(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    normalized
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.trim_matches(|ch: char| {
                matches!(ch, '`' | '"' | '\'' | '。' | '，' | ',' | ';' | '；')
            })
            .to_string()
        })
}

fn output_path_is_workspace_scoped(path: &str, workspace_root: &Path) -> bool {
    let compact = path.split_whitespace().collect::<String>();
    if compact.contains("{{env.workspace}}") {
        return true;
    }

    let normalized = path.replace('\\', "/");
    let root = resolve_slash(workspace_root);
    let normalized_cmp = normalized.to_ascii_lowercase();
    let root_cmp = root.to_ascii_lowercase();
    normalized_cmp == root_cmp || normalized_cmp.starts_with(&format!("{root_cmp}/"))
}

fn prompt_references_graph_input(prompt: &str) -> bool {
    let compact = prompt.split_whitespace().collect::<String>();
    compact.contains("{{inputs.input}}")
}

fn prompt_has_no_write_guard(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    lower.contains("do not write")
        || lower.contains("don't write")
        || lower.contains("do not save")
        || prompt.contains("不要写")
        || prompt.contains("不要保存")
        || prompt.contains("不要调用 write")
        || prompt.contains("不得写")
        || prompt.contains("不得保存")
        || prompt.contains("只输出 markdown artifact")
        || prompt.contains("仅输出 markdown artifact")
}

fn resolve_subgraph_input(
    config: &LlmCoordinatorConfig,
    run: &TaskGraphRun,
    project: &str,
    ws: &std::path::Path,
    scripts_dir: &std::path::Path,
) -> Value {
    let Some(bindings) = config.input_bindings.as_ref() else {
        if let Some(input) = coordinator_input_value(run) {
            let mut default_input = Map::new();
            default_input.insert("input".to_string(), input);
            return Value::Object(default_input);
        }
        return Value::Object(Default::default());
    };
    let resolved = match bindings {
        Value::Object(_) => Value::Object(eval::resolve_node_inputs(
            Some(bindings),
            project,
            ws,
            scripts_dir,
            &run.context,
        )),
        other => {
            let mut wrapped = serde_json::Map::new();
            wrapped.insert("value".to_string(), other.clone());
            eval::resolve_node_inputs(
                Some(&Value::Object(wrapped)),
                project,
                ws,
                scripts_dir,
                &run.context,
            )
            .remove("value")
            .unwrap_or(Value::Null)
        }
    };
    inject_coordinator_input_into_object(resolved, run)
}

fn llm_config_with_coordinator_input(
    config: &LlmCoordinatorConfig,
    run: &TaskGraphRun,
) -> LlmConfig {
    let mut llm = config.llm.clone();
    let Some(input) = coordinator_input_value(run) else {
        return llm;
    };

    let mut inputs = match llm.inputs.take() {
        Some(Value::Object(map)) => map,
        Some(other) => {
            let mut map = Map::new();
            map.insert("value".to_string(), other);
            map
        }
        None => Map::new(),
    };
    insert_if_missing_or_empty(&mut inputs, "input", input);
    llm.inputs = Some(Value::Object(inputs));
    llm
}

fn inject_coordinator_input_into_object(value: Value, run: &TaskGraphRun) -> Value {
    let Some(input) = coordinator_input_value(run) else {
        return value;
    };
    let Value::Object(mut map) = value else {
        return value;
    };
    insert_if_missing_or_empty(&mut map, "input", input);
    Value::Object(map)
}

fn coordinator_input_value(run: &TaskGraphRun) -> Option<Value> {
    let data = run.context.input.get("__data")?.as_object()?;
    if data.is_empty() {
        return None;
    }
    if data.len() == 1 {
        let value = data.values().next()?.clone();
        return Some(value.get("output").cloned().unwrap_or(value));
    }

    let mut values = Map::new();
    for (source_node_id, value) in data {
        values.insert(
            source_node_id.clone(),
            value
                .get("output")
                .cloned()
                .unwrap_or_else(|| value.clone()),
        );
    }
    Some(Value::Object(values))
}

fn insert_if_missing_or_empty(map: &mut Map<String, Value>, key: &str, value: Value) {
    if map.get(key).is_some_and(is_non_empty_value) {
        return;
    }
    map.insert(key.to_string(), value);
}

fn is_non_empty_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn collect_child_output(detail: &run_state::TaskGraphRunDetail) -> Value {
    if !detail.run.context.node_outputs.is_empty() {
        for node in detail.graph_snapshot.nodes.iter().rev() {
            if let Some(output) = detail.run.context.node_outputs.get(&node.id) {
                return output.clone();
            }
        }
    }
    Value::Null
}

fn validation_error_message(errors: &[TaskGraphValidationError]) -> String {
    errors
        .iter()
        .map(|error| format!("{}: {}", error.path, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn failed_outcome(
    node: &TaskGraphNode,
    start_time: String,
    duration_ms: u64,
    code: &str,
    message: String,
) -> NodeOutcome {
    let end_time = Utc::now().to_rfc3339();
    NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        output: None,
        node_state: node_state(
            node,
            NodeRunStatus::Failed,
            start_time,
            Some(end_time),
            Some(duration_ms),
            None,
            Some(NodeError {
                code: code.to_string(),
                message: message.clone(),
            }),
            Some(message),
        ),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    }
}

#[allow(clippy::too_many_arguments)]
fn node_state(
    node: &TaskGraphNode,
    status: NodeRunStatus,
    started_at: String,
    completed_at: Option<String>,
    duration_ms: Option<u64>,
    exit_code: Option<i32>,
    error: Option<NodeError>,
    log_tail: Option<String>,
) -> TaskGraphRunNode {
    TaskGraphRunNode {
        node_id: node.id.clone(),
        status,
        started_at: Some(started_at),
        completed_at,
        duration_ms,
        iteration: None,
        exit_code,
        error,
        output_artifact: None,
        log_tail,
        child_run_id: None,
        runtime: None,
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn coordinator_policy_rejects_child_runtime_mismatch_without_rewriting() {
        let config: LlmCoordinatorConfig = serde_json::from_value(json!({
            "run_as": "llm",
            "runtime": "opencode",
            "agent": "native",
            "model": "provider/model",
            "prompt": { "mode": "inline", "template": "" }
        }))
        .unwrap();

        let graph = validate_subgraph_static_passes(
            r#"{
                "schema_version": 1,
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [
                    { "id": "start", "type": "start", "label": "Start", "config": {} },
                    {
                        "id": "research",
                        "type": "llm",
                        "label": "Research",
                        "config": {
                            "runtime": "codex",
                            "agent": "codex",
                            "prompt": { "mode": "inline", "template": "research" },
                            "output": { "artifact_type": "markdown", "required": true }
                        }
                    },
                    { "id": "end", "type": "end", "label": "End", "config": { "result": "succeeded" } }
                ],
                "edges": [
                    { "id": "e1", "from": "start", "to": "research", "kind": "exec" },
                    { "id": "e2", "from": "research", "to": "end", "kind": "exec" }
                ]
            }"#,
        )
        .unwrap();
        let errors = validate_subgraph_policy_errors(&graph, &config);

        assert!(
            errors
                .iter()
                .any(|error| error.code == "runtime_policy_mismatch"),
            "expected runtime policy mismatch, got {:?}",
            errors
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "model_policy_mismatch"),
            "expected model policy mismatch, got {:?}",
            errors
        );
    }

    #[test]
    fn static_passes_reject_invalid_json_before_schema() {
        let failure = validate_subgraph_static_passes("{ invalid").unwrap_err();

        assert_eq!(failure.stage, "json_syntax");
        assert_eq!(failure.errors[0].code, "invalid_json");
    }

    #[test]
    fn static_passes_reject_loose_shape_without_normalizing() {
        let failure = validate_subgraph_static_passes(
            r#"{
                "schema_version": "1.0",
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [],
                "edges": []
            }"#,
        )
        .unwrap_err();

        assert_eq!(failure.stage, "task_graph_schema");
        assert!(
            failure
                .errors
                .iter()
                .any(|error| error.path == "schema_version" && error.code == "invalid_type"),
            "expected schema_version type error, got {:?}",
            failure.errors
        );
    }

    #[test]
    fn static_passes_reject_semantic_graph_errors_after_schema() {
        let failure = validate_subgraph_static_passes(
            r#"{
                "schema_version": 1,
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [
                    { "id": "start", "type": "start", "label": "Start", "config": {} },
                    { "id": "end", "type": "end", "label": "End", "config": { "result": "succeeded" } }
                ],
                "edges": [
                    { "id": "e1", "from": "start", "to": "missing", "kind": "exec" }
                ]
            }"#,
        )
        .unwrap_err();

        assert_eq!(failure.stage, "graph_semantic");
        assert!(
            failure
                .errors
                .iter()
                .any(|error| error.path.contains("edges[0].to")),
            "expected edge endpoint error, got {:?}",
            failure.errors
        );
    }

    #[test]
    fn static_passes_accept_valid_task_graph() {
        let graph = validate_subgraph_static_passes(
            r#"{
                "schema_version": 1,
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [
                    { "id": "start", "type": "start", "label": "Start", "config": {} },
                    { "id": "end", "type": "end", "label": "End", "config": { "result": "succeeded" } }
                ],
                "edges": [
                    { "id": "e1", "from": "start", "to": "end", "kind": "exec" }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(graph.id, "child");
        assert!(!graph.nodes[0].pins.is_empty());
    }

    #[test]
    fn output_path_policy_rejects_scout_writes_and_relative_synthesize_path() {
        let tmp = TempDir::new().unwrap();
        let graph = validate_subgraph_static_passes(
            r#"{
                "schema_version": 1,
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [
                    { "id": "start", "type": "start", "label": "Start", "config": {} },
                    {
                        "id": "scout-runtime",
                        "type": "llm",
                        "label": "Scout Runtime",
                        "config": {
                            "runtime": "opencode",
                            "agent": "native",
                            "prompt": { "mode": "inline", "template": "{{inputs.input}}\n请写入 report.md" },
                            "output": { "artifact_type": "markdown", "required": true }
                        }
                    },
                    {
                        "id": "synthesize",
                        "type": "llm",
                        "label": "Synthesize",
                        "config": {
                            "runtime": "opencode",
                            "agent": "native",
                            "prompt": { "mode": "inline", "template": "{{inputs.input}}\n最终报告写入 report.md" },
                            "output": { "artifact_type": "markdown", "required": true }
                        }
                    },
                    { "id": "end", "type": "end", "label": "End", "config": { "result": "succeeded" } }
                ],
                "edges": [
                    { "id": "e1", "from": "start", "to": "scout-runtime", "kind": "exec" },
                    { "id": "e2", "from": "scout-runtime", "to": "synthesize", "kind": "exec" },
                    { "id": "e3", "from": "synthesize", "to": "end", "kind": "exec" }
                ]
            }"#,
        )
        .unwrap();
        let mut errors = Vec::new();

        validate_child_llm_output_path_policy(
            &graph,
            tmp.path(),
            &["report.md".to_string()],
            &mut errors,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.code == "output_path_on_non_synthesize_node"),
            "expected scout output path error, got {:?}",
            errors
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "output_path_not_workspace_scoped"),
            "expected relative synthesize path error, got {:?}",
            errors
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "requested_output_path_missing_or_relative"),
            "expected requested path error, got {:?}",
            errors
        );
    }

    #[test]
    fn output_path_policy_accepts_guarded_scout_and_workspace_synthesize_path() {
        let tmp = TempDir::new().unwrap();
        let graph = validate_subgraph_static_passes(
            r#"{
                "schema_version": 1,
                "id": "child",
                "scope": "project",
                "title": "Child",
                "version": 1,
                "readonly": false,
                "nodes": [
                    { "id": "start", "type": "start", "label": "Start", "config": {} },
                    {
                        "id": "scout-runtime",
                        "type": "llm",
                        "label": "Scout Runtime",
                        "config": {
                            "runtime": "opencode",
                            "agent": "native",
                            "prompt": { "mode": "inline", "template": "{{inputs.input}}\n不要写入或保存任何文件；只输出 markdown artifact。" },
                            "output": { "artifact_type": "markdown", "required": true }
                        }
                    },
                    {
                        "id": "synthesize",
                        "type": "llm",
                        "label": "Synthesize",
                        "config": {
                            "runtime": "opencode",
                            "agent": "native",
                            "prompt": { "mode": "inline", "template": "{{inputs.input}}\n最终报告写入 {{env.workspace}}/report.md，同时输出 markdown artifact。" },
                            "output": { "artifact_type": "markdown", "required": true }
                        }
                    },
                    { "id": "end", "type": "end", "label": "End", "config": { "result": "succeeded" } }
                ],
                "edges": [
                    { "id": "e1", "from": "start", "to": "scout-runtime", "kind": "exec" },
                    { "id": "e2", "from": "scout-runtime", "to": "synthesize", "kind": "exec" },
                    { "id": "e3", "from": "synthesize", "to": "end", "kind": "exec" }
                ]
            }"#,
        )
        .unwrap();
        let mut errors = Vec::new();

        validate_child_llm_output_path_policy(
            &graph,
            tmp.path(),
            &["report.md".to_string()],
            &mut errors,
        );

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn repair_instruction_contains_tool_style_failure() {
        let feedback = SubgraphRepairFeedback {
            failure: SubgraphPassFailure::new(
                "json_syntax",
                vec![TaskGraphValidationError {
                    path: "$".to_string(),
                    code: "invalid_json".to_string(),
                    message: "Invalid JSON: expected value".to_string(),
                }],
            ),
            previous_output: "{ invalid".to_string(),
        };

        let prompt = coordinator_repair_instruction(&feedback);

        assert!(prompt.contains("\"stage\": \"json_syntax\""));
        assert!(prompt.contains("Return exactly one complete TaskGraphDefinition JSON object"));
        assert!(prompt.contains("{ invalid"));
    }
}
