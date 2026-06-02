use std::collections::{HashMap, HashSet};

use crate::task_graph::definition::types::{
    BranchConfig, DataValueConfig, EndConfig, HumanGateConfig, InputVarConfig, LlmConfig,
    LlmCoordinatorConfig, LlmRunAs, LoopConfig, NodeType, ShellConfig, StartConfig, SubGraphConfig,
    SystemWriteOutputConfig, TaskGraphDefinition, TaskGraphEdge, TaskGraphNode, TaskGraphRunPolicy,
    TaskGraphValidationError, KNOWN_RUNTIMES, VALID_END_RESULTS,
};
use crate::task_graph::tool_layer::{KNOWN_LLM_TOOLKITS, TOOLKIT_BROWSER_USE};

/// Node id: kebab-case, 1-64 chars, alphanumeric + hyphen.
pub(super) fn is_valid_node_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 64 {
        return false;
    }
    let bytes = id.as_bytes();
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(super) fn validate_graph_inputs(
    def: &TaskGraphDefinition,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let mut seen = HashSet::new();
    let Some(inputs) = &def.inputs else {
        return;
    };

    for (index, input) in inputs.iter().enumerate() {
        if !is_valid_node_id(&input.id) {
            errors.push(TaskGraphValidationError {
                path: format!("inputs[{index}].id"),
                code: "invalid_format".to_string(),
                message: format!("Input id '{}' must be kebab-case", input.id),
            });
        }
        if !seen.insert(input.id.as_str()) {
            errors.push(TaskGraphValidationError {
                path: format!("inputs[{index}].id"),
                code: "duplicate_input".to_string(),
                message: format!("Input id '{}' is duplicated", input.id),
            });
        }
        match input.value_type.as_str() {
            "string" | "number" | "boolean" | "json" | "ticket_ref" => {}
            other if is_valid_array_type(other) => {}
            other => errors.push(TaskGraphValidationError {
                path: format!("inputs[{index}].type"),
                code: "invalid_input_type".to_string(),
                message: format!("Input '{}' has unsupported type '{}'", input.id, other),
            }),
        }
    }
}

pub(super) fn validate_graph_run_policy(
    def: &TaskGraphDefinition,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let Some(policy) = def
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.run_policy.as_ref())
    else {
        return;
    };

    if policy.max_concurrent_runs < TaskGraphRunPolicy::DEFAULT_MAX_CONCURRENT_RUNS
        || policy.max_concurrent_runs > TaskGraphRunPolicy::MAX_CONCURRENT_RUNS
    {
        errors.push(TaskGraphValidationError {
            path: "metadata.run_policy.max_concurrent_runs".to_string(),
            code: "out_of_range".to_string(),
            message: format!(
                "max_concurrent_runs must be 1-{}, got {}",
                TaskGraphRunPolicy::MAX_CONCURRENT_RUNS,
                policy.max_concurrent_runs
            ),
        });
    }

    if policy.max_queue_wait_ms < TaskGraphRunPolicy::MIN_QUEUE_WAIT_MS
        || policy.max_queue_wait_ms > TaskGraphRunPolicy::MAX_QUEUE_WAIT_MS
    {
        errors.push(TaskGraphValidationError {
            path: "metadata.run_policy.max_queue_wait_ms".to_string(),
            code: "out_of_range".to_string(),
            message: format!(
                "max_queue_wait_ms must be {}-{}",
                TaskGraphRunPolicy::MIN_QUEUE_WAIT_MS,
                TaskGraphRunPolicy::MAX_QUEUE_WAIT_MS
            ),
        });
    }

    if policy.max_queue_timeout_retries < TaskGraphRunPolicy::MIN_QUEUE_TIMEOUT_RETRIES
        || policy.max_queue_timeout_retries > TaskGraphRunPolicy::MAX_QUEUE_TIMEOUT_RETRIES
    {
        errors.push(TaskGraphValidationError {
            path: "metadata.run_policy.max_queue_timeout_retries".to_string(),
            code: "out_of_range".to_string(),
            message: format!(
                "max_queue_timeout_retries must be {}-{}",
                TaskGraphRunPolicy::MIN_QUEUE_TIMEOUT_RETRIES,
                TaskGraphRunPolicy::MAX_QUEUE_TIMEOUT_RETRIES
            ),
        });
    }
}

/// 合法的基础类型
const VALID_BASE_TYPES: &[&str] = &["string", "number", "boolean", "json", "ticket_ref"];

/// 检查是否为合法的 array<T> 泛型类型
fn is_valid_array_type(type_str: &str) -> bool {
    type_str
        .strip_prefix("array<")
        .and_then(|s| s.strip_suffix('>'))
        .is_some_and(|inner| VALID_BASE_TYPES.contains(&inner))
}

// ─── Branch validation ───────────────────────────────────────────────────────

pub(super) fn validate_branch_config(
    node: &TaskGraphNode,
    idx: usize,
    outgoing: &HashMap<&str, Vec<&TaskGraphEdge>>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<BranchConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!("Branch node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    // Rule 7: default_rule_id must exist in rules
    if !config.rules.iter().any(|r| r.id == config.default_rule_id) {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.default_rule_id"),
            code: "default_rule_not_found".to_string(),
            message: format!(
                "Branch '{}' default_rule_id '{}' not found in rules",
                node.id, config.default_rule_id
            ),
        });
    }

    // Rule 6: each rule must have a matching outgoing edge with from_pin = "rule:<rule-id>"
    let node_outgoing = outgoing.get(node.id.as_str());
    for rule in &config.rules {
        let expected_pin = format!("rule:{}", rule.id);
        let has_edge = node_outgoing.is_some_and(|edges| {
            edges.iter().any(|e| {
                e.from_pin.as_deref() == Some(expected_pin.as_str())
                    || e.source_handle.as_deref() == Some(expected_pin.as_str())
            })
        });
        if !has_edge {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{}].config.rules[{}]", idx, rule.id),
                code: "missing_branch_edge".to_string(),
                message: format!(
                    "Branch '{}' rule '{}' has no matching outgoing edge with from_pin='{}'",
                    node.id, rule.id, expected_pin
                ),
            });
        }
    }
}

// ─── Loop validation ─────────────────────────────────────────────────────────

pub(super) fn validate_loop_config(
    node: &TaskGraphNode,
    idx: usize,
    node_map: &HashMap<&str, &TaskGraphNode>,
    outgoing: &HashMap<&str, Vec<&TaskGraphEdge>>,
    _incoming: &HashMap<&str, Vec<&TaskGraphEdge>>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<LoopConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!("Loop node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    // Rule 8: max_iterations must be 1..=50
    if config.max_iterations < 1 || config.max_iterations > 50 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.max_iterations"),
            code: "out_of_range".to_string(),
            message: format!(
                "Loop '{}' max_iterations must be 1-50, got {}",
                node.id, config.max_iterations
            ),
        });
    }

    validate_loop_endpoint(
        idx,
        &node.id,
        "body_entry",
        &config.body_entry,
        node_map,
        errors,
    );
    validate_loop_endpoint(
        idx,
        &node.id,
        "body_exit",
        &config.body_exit,
        node_map,
        errors,
    );

    // Rule 9: must have body edge (from_pin="body")
    let node_outgoing = outgoing.get(node.id.as_str());
    let body_edges = matching_loop_edges(node_outgoing, "body");
    if body_edges.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}]"),
            code: "missing_loop_body_edge".to_string(),
            message: format!(
                "Loop '{}' must have an outgoing edge with from_pin='body'",
                node.id
            ),
        });
    } else if !config.body_entry.trim().is_empty()
        && !body_edges.iter().any(|edge| edge.to == config.body_entry)
    {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.body_entry"),
            code: "loop_body_entry_edge_mismatch".to_string(),
            message: format!(
                "Loop '{}' body_entry '{}' must match a body edge target",
                node.id, config.body_entry
            ),
        });
    }

    // Rule 9: must have exit edge (from_pin="exit")
    let exit_edges = matching_loop_edges(node_outgoing, "exit");
    if exit_edges.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}]"),
            code: "missing_loop_exit_edge".to_string(),
            message: format!(
                "Loop '{}' must have an outgoing edge with from_pin='exit'",
                node.id
            ),
        });
    }

    validate_loop_condition(idx, &node.id, config.condition.as_ref(), node_map, errors);
}

fn matching_loop_edges<'a>(
    node_outgoing: Option<&'a Vec<&'a TaskGraphEdge>>,
    pin: &str,
) -> Vec<&'a TaskGraphEdge> {
    node_outgoing
        .map(|edges| {
            edges
                .iter()
                .copied()
                .filter(|edge| {
                    edge.from_pin.as_deref() == Some(pin)
                        || edge.source_handle.as_deref() == Some(pin)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn validate_loop_endpoint(
    idx: usize,
    loop_id: &str,
    field: &str,
    value: &str,
    node_map: &HashMap<&str, &TaskGraphNode>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let value = value.trim();
    if value.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.{field}"),
            code: "empty_loop_endpoint".to_string(),
            message: format!("Loop '{loop_id}' {field} must not be empty"),
        });
        return;
    }
    if !node_map.contains_key(value) {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.{field}"),
            code: "loop_endpoint_not_found".to_string(),
            message: format!("Loop '{loop_id}' {field} references non-existent node '{value}'"),
        });
    }
}

fn validate_loop_condition(
    idx: usize,
    loop_id: &str,
    condition: Option<&serde_json::Value>,
    node_map: &HashMap<&str, &TaskGraphNode>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let Some(condition) = condition else {
        return;
    };
    let Some(object) = condition.as_object() else {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.condition"),
            code: "invalid_loop_condition".to_string(),
            message: format!("Loop '{loop_id}' condition must be an object"),
        });
        return;
    };

    let Some(input_ref) = object.get("input_ref").and_then(serde_json::Value::as_str) else {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.condition.input_ref"),
            code: "missing_loop_condition_input_ref".to_string(),
            message: format!("Loop '{loop_id}' condition must define input_ref"),
        });
        return;
    };
    validate_condition_input_ref(idx, loop_id, input_ref, node_map, errors);

    if let Some(op) = object.get("op").and_then(serde_json::Value::as_str) {
        const ALLOWED_OPS: &[&str] = &[
            "always",
            "exists",
            "equals",
            "not_equals",
            ">",
            "gt",
            ">=",
            "gte",
            "<",
            "lt",
            "<=",
            "lte",
            "contains",
            "is_empty",
            "not_empty",
            "truthy",
            "falsy",
        ];
        if !ALLOWED_OPS.contains(&op) {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.condition.op"),
                code: "invalid_loop_condition_op".to_string(),
                message: format!("Loop '{loop_id}' condition op '{op}' is not supported"),
            });
        }
    }
}

fn validate_condition_input_ref(
    idx: usize,
    loop_id: &str,
    input_ref: &str,
    node_map: &HashMap<&str, &TaskGraphNode>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let Some(path) = input_ref.trim().strip_prefix("$.") else {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.condition.input_ref"),
            code: "invalid_loop_condition_input_ref".to_string(),
            message: format!(
                "Loop '{loop_id}' condition input_ref '{input_ref}' must start with '$.'"
            ),
        });
        return;
    };

    let segments: Vec<&str> = path.split('.').collect();
    match segments.as_slice() {
        ["nodes", node_id, "output", ..] => {
            if !node_map.contains_key(*node_id) {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.condition.input_ref"),
                    code: "loop_condition_node_not_found".to_string(),
                    message: format!(
                        "Loop '{loop_id}' condition input_ref references non-existent node '{node_id}'"
                    ),
                });
            }
        }
        ["input", ..] => {}
        _ => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.condition.input_ref"),
                code: "invalid_loop_condition_input_ref".to_string(),
                message: format!(
                    "Loop '{loop_id}' condition input_ref '{input_ref}' must use '$.nodes.<node-id>.output' or '$.input.<key>'"
                ),
            });
        }
    }
}

// ─── LLM validation ─────────────────────────────────────────────────────────

pub(super) fn validate_llm_config(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<LlmConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!("LLM node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    match config.run_as {
        LlmRunAs::Llm => {
            if !KNOWN_RUNTIMES.contains(&config.runtime.as_str()) {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.runtime"),
                    code: "runtime_not_found".to_string(),
                    message: format!(
                        "LLM node '{}' runtime '{}' is not a known MVP runtime; must be one of: {}",
                        node.id,
                        config.runtime,
                        KNOWN_RUNTIMES.join(", ")
                    ),
                });
            }

            if config.agent.trim().is_empty() {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.agent"),
                    code: "empty_agent".to_string(),
                    message: format!(
                        "LLM node '{}' agent must not be empty (use 'native' for the default agent)",
                        node.id
                    ),
                });
            }
        }
        LlmRunAs::Agent => {
            if config
                .agent_profile
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.agent_profile"),
                    code: "empty_agent_profile".to_string(),
                    message: format!("LLM node '{}' must select an agent profile", node.id),
                });
            }
        }
    }

    if node.config.get("harness").is_some() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.harness"),
            code: "legacy_harness_not_supported".to_string(),
            message: format!(
                "LLM node '{}' uses legacy harness config; use config.resource_bundle/output_contract/tool_policy on the LLM node instead",
                node.id
            ),
        });
    }
    if node
        .config
        .get("preset")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|preset| preset == "agent_node")
    {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.preset"),
            code: "legacy_agent_node_preset".to_string(),
            message: format!(
                "LLM node '{}' uses legacy preset=agent_node; configure the LLM node directly instead",
                node.id
            ),
        });
    }

    if let Some(resource_bundle) = &config.resource_bundle {
        let input = resource_bundle.input.trim();
        if input.is_empty() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.resource_bundle.input"),
                code: "empty_resource_bundle_input".to_string(),
                message: format!(
                    "LLM node '{}' resource_bundle.input must name the data input pin or reference a graph resource via {{{{resources.xxx}}}}",
                    node.id
                ),
            });
        }
        // resource_bundle.input can be either:
        // 1. A data input pin name (legacy, resolved via data edge)
        // 2. A template expression like "{{resources.xxx}}" (new, resolved via graph resources)
    }

    for (toolkit_idx, toolkit) in config.toolkits.iter().enumerate() {
        let toolkit = toolkit.id().trim();
        if toolkit.is_empty() {
            continue;
        }
        if !KNOWN_LLM_TOOLKITS.contains(&toolkit) {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.toolkits[{toolkit_idx}]"),
                code: "unknown_llm_toolkit".to_string(),
                message: format!(
                    "LLM node '{}' toolkit '{}' is not supported",
                    node.id, toolkit
                ),
            });
        }
        if let Some(exposure) = config.toolkits[toolkit_idx].exposure() {
            if !matches!(exposure, "direct" | "proxy") {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config.toolkits[{toolkit_idx}].exposure"),
                    code: "unsupported_llm_toolkit_exposure".to_string(),
                    message: format!(
                        "LLM node '{}' toolkit '{}' exposure '{}' is not supported",
                        node.id, toolkit, exposure
                    ),
                });
            }
        }
        if toolkit == TOOLKIT_BROWSER_USE
            && matches!(config.run_as, LlmRunAs::Llm)
            && config.runtime.as_str() != "codex"
        {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.toolkits[{toolkit_idx}]"),
                code: "unsupported_llm_toolkit_runtime".to_string(),
                message: format!(
                    "LLM node '{}' toolkit browser_use currently requires runtime codex",
                    node.id
                ),
            });
        }
    }

    if let Some(output_contract) = &config.output_contract {
        if !output_contract.is_object() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.output_contract"),
                code: "invalid_output_contract".to_string(),
                message: format!("LLM node '{}' output_contract must be an object", node.id),
            });
        }
    }

    if let Some(lifecycle) = &config.lifecycle {
        if !lifecycle.is_object() {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config.lifecycle"),
                code: "invalid_lifecycle".to_string(),
                message: format!("LLM node '{}' lifecycle must be an object", node.id),
            });
        }
    }
}

pub(super) fn validate_llm_coordinator_config(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<LlmCoordinatorConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!(
                    "LLM Coordinator node '{}' config parse error: {}",
                    node.id, e
                ),
            });
            return;
        }
    };

    let llm_node = TaskGraphNode {
        node_type: NodeType::Llm,
        config: serde_json::to_value(config.llm.clone()).unwrap_or_default(),
        ..node.clone()
    };
    validate_llm_config(&llm_node, idx, errors);

    if config.coordinator.max_nodes == 0 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.coordinator.max_nodes"),
            code: "out_of_range".to_string(),
            message: format!(
                "LLM Coordinator node '{}' coordinator.max_nodes must be greater than 0",
                node.id
            ),
        });
    }
    if config.coordinator.max_edges == 0 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.coordinator.max_edges"),
            code: "out_of_range".to_string(),
            message: format!(
                "LLM Coordinator node '{}' coordinator.max_edges must be greater than 0",
                node.id
            ),
        });
    }
    if config.coordinator.max_repair_attempts == 0 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.coordinator.max_repair_attempts"),
            code: "out_of_range".to_string(),
            message: format!(
                "LLM Coordinator node '{}' coordinator.max_repair_attempts must be greater than 0",
                node.id
            ),
        });
    }
}

// ─── Node config shape validation ────────────────────────────────────────────

pub(super) fn validate_node_config_shape(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match node.node_type {
        NodeType::Start => {
            // config should be empty object or parseable as StartConfig
            let _: Result<StartConfig, _> = serde_json::from_value(node.config.clone());
            // Start config is always {} — no validation needed beyond parse
        }
        NodeType::End => {
            let config: Result<EndConfig, _> = serde_json::from_value(node.config.clone());
            match config {
                Ok(c) => {
                    if !VALID_END_RESULTS.contains(&c.result.as_str()) {
                        errors.push(TaskGraphValidationError {
                            path: format!("nodes[{idx}].config.result"),
                            code: "invalid_end_result".to_string(),
                            message: format!(
                                "End node '{}' result '{}' must be one of: succeeded, failed, cancelled",
                                node.id, c.result
                            ),
                        });
                    }
                }
                Err(e) => {
                    errors.push(TaskGraphValidationError {
                        path: format!("nodes[{idx}].config"),
                        code: "invalid_config".to_string(),
                        message: format!("End node '{}' config parse error: {}", node.id, e),
                    });
                }
            }
        }
        NodeType::HumanGate => {
            let config: Result<HumanGateConfig, _> = serde_json::from_value(node.config.clone());
            if let Err(e) = config {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config"),
                    code: "invalid_config".to_string(),
                    message: format!("HumanGate node '{}' config parse error: {}", node.id, e),
                });
            }
        }
        NodeType::InputVar => {
            let config: Result<InputVarConfig, _> = serde_json::from_value(node.config.clone());
            match config {
                Ok(c) => {
                    if c.input_id.is_empty() {
                        errors.push(TaskGraphValidationError {
                            path: format!("nodes[{idx}].config.input_id"),
                            code: "empty_input_id".to_string(),
                            message: format!(
                                "InputVar node '{}' must have a non-empty input_id",
                                node.id
                            ),
                        });
                    }
                }
                Err(e) => {
                    errors.push(TaskGraphValidationError {
                        path: format!("nodes[{idx}].config"),
                        code: "invalid_config".to_string(),
                        message: format!("InputVar node '{}' config parse error: {}", node.id, e),
                    });
                }
            }
        }
        NodeType::DataValue => {
            let config: Result<DataValueConfig, _> = serde_json::from_value(node.config.clone());
            if let Err(e) = config {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{idx}].config"),
                    code: "invalid_config".to_string(),
                    message: format!("DataValue node '{}' config parse error: {}", node.id, e),
                });
            }
        }
        NodeType::Shell => {
            validate_shell_config(node, idx, errors);
        }
        NodeType::SystemWriteOutput => {
            validate_system_write_output_config(node, idx, errors);
        }
        // LLM, Branch, Loop, SubGraph — already validated in dedicated functions
        _ => {}
    }
}

fn validate_system_write_output_config(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<SystemWriteOutputConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(config) => config,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!(
                    "SystemWriteOutput node '{}' config parse error: {}",
                    node.id, e
                ),
            });
            return;
        }
    };

    if config.output_path.trim().is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.output_path"),
            code: "empty_output_path".to_string(),
            message: format!(
                "SystemWriteOutput node '{}' output_path must not be empty",
                node.id
            ),
        });
    }
}

fn validate_shell_config(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<ShellConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!("Shell node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    if config.command.trim().is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.command"),
            code: "empty_command".to_string(),
            message: format!("Shell node '{}' command must not be empty", node.id),
        });
    }

    if config.command.chars().any(char::is_whitespace) {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.command"),
            code: "raw_shell_string_not_supported".to_string(),
            message: format!(
                "Shell node '{}' command must be an executable name/path; put parameters in args",
                node.id
            ),
        });
    }

    if contains_shell_metachar(&config.command)
        || config.args.iter().any(|arg| contains_shell_metachar(arg))
    {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config"),
            code: "shell_metachar_not_supported".to_string(),
            message: format!(
                "Shell node '{}' does not support shell metacharacters or pipelines",
                node.id
            ),
        });
    }

    if config.timeout_ms == 0 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.timeout_ms"),
            code: "out_of_range".to_string(),
            message: format!("Shell node '{}' timeout_ms must be greater than 0", node.id),
        });
    }

    if config.expected_exit_codes.is_empty() {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.expected_exit_codes"),
            code: "empty_expected_exit_codes".to_string(),
            message: format!(
                "Shell node '{}' expected_exit_codes must contain at least one code",
                node.id
            ),
        });
    }

    if config.capture.max_bytes == 0 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{idx}].config.capture.max_bytes"),
            code: "out_of_range".to_string(),
            message: format!(
                "Shell node '{}' capture.max_bytes must be greater than 0",
                node.id
            ),
        });
    }
}

fn contains_shell_metachar(value: &str) -> bool {
    ["&&", "||", "|", ";", ">", "<", "`", "$("]
        .iter()
        .any(|token| value.contains(token))
}

// ─── SubGraph validation ─────────────────────────────────────────────────────────────

pub(super) fn validate_sub_graph_config(
    node: &TaskGraphNode,
    idx: usize,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<SubGraphConfig, _> = serde_json::from_value(node.config.clone());
    match config {
        Ok(_c) => {
            // graph_id 允许为空（草稿状态），运行时再校验
        }
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{idx}].config"),
                code: "invalid_config".to_string(),
                message: format!("SubGraph node '{}' config parse error: {}", node.id, e),
            });
        }
    }
}
