use std::collections::{HashMap, HashSet};

use super::super::types::*;

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
                path: format!("inputs[{}].id", index),
                code: "invalid_format".to_string(),
                message: format!("Input id '{}' must be kebab-case", input.id),
            });
        }
        if !seen.insert(input.id.as_str()) {
            errors.push(TaskGraphValidationError {
                path: format!("inputs[{}].id", index),
                code: "duplicate_input".to_string(),
                message: format!("Input id '{}' is duplicated", input.id),
            });
        }
        match input.value_type.as_str() {
            "string" | "number" | "boolean" | "json" | "ticket_ref" => {}
            other if is_valid_array_type(other) => {}
            other => errors.push(TaskGraphValidationError {
                path: format!("inputs[{}].type", index),
                code: "invalid_input_type".to_string(),
                message: format!("Input '{}' has unsupported type '{}'", input.id, other),
            }),
        }
    }
}

/// 合法的基础类型
const VALID_BASE_TYPES: &[&str] = &["string", "number", "boolean", "json", "ticket_ref"];

/// 检查是否为合法的 array<T> 泛型类型
fn is_valid_array_type(type_str: &str) -> bool {
    if let Some(inner) = type_str
        .strip_prefix("array<")
        .and_then(|s| s.strip_suffix('>'))
    {
        VALID_BASE_TYPES.contains(&inner)
    } else {
        false
    }
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
                path: format!("nodes[{}].config", idx),
                code: "invalid_config".to_string(),
                message: format!("Branch node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    // Rule 7: default_rule_id must exist in rules
    if !config.rules.iter().any(|r| r.id == config.default_rule_id) {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.default_rule_id", idx),
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
        let has_edge = node_outgoing
            .map(|edges| {
                edges.iter().any(|e| {
                    e.from_pin.as_deref() == Some(expected_pin.as_str())
                        || e.source_handle.as_deref() == Some(expected_pin.as_str())
                })
            })
            .unwrap_or(false);
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
    outgoing: &HashMap<&str, Vec<&TaskGraphEdge>>,
    _incoming: &HashMap<&str, Vec<&TaskGraphEdge>>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let config: Result<LoopConfig, _> = serde_json::from_value(node.config.clone());
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            errors.push(TaskGraphValidationError {
                path: format!("nodes[{}].config", idx),
                code: "invalid_config".to_string(),
                message: format!("Loop node '{}' config parse error: {}", node.id, e),
            });
            return;
        }
    };

    // Rule 8: max_iterations must be 1..=50
    if config.max_iterations < 1 || config.max_iterations > 50 {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}].config.max_iterations", idx),
            code: "out_of_range".to_string(),
            message: format!(
                "Loop '{}' max_iterations must be 1-50, got {}",
                node.id, config.max_iterations
            ),
        });
    }

    // Rule 9: must have body edge (from_pin="body")
    let node_outgoing = outgoing.get(node.id.as_str());
    let has_body = node_outgoing
        .map(|edges| {
            edges.iter().any(|e| {
                e.from_pin.as_deref() == Some("body") || e.source_handle.as_deref() == Some("body")
            })
        })
        .unwrap_or(false);
    if !has_body {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}]", idx),
            code: "missing_loop_body_edge".to_string(),
            message: format!(
                "Loop '{}' must have an outgoing edge with from_pin='body'",
                node.id
            ),
        });
    }

    // Rule 9: must have exit edge (from_pin="exit")
    let has_exit = node_outgoing
        .map(|edges| {
            edges.iter().any(|e| {
                e.from_pin.as_deref() == Some("exit") || e.source_handle.as_deref() == Some("exit")
            })
        })
        .unwrap_or(false);
    if !has_exit {
        errors.push(TaskGraphValidationError {
            path: format!("nodes[{}]", idx),
            code: "missing_loop_exit_edge".to_string(),
            message: format!(
                "Loop '{}' must have an outgoing edge with from_pin='exit'",
                node.id
            ),
        });
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
                path: format!("nodes[{}].config", idx),
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
                    path: format!("nodes[{}].config.runtime", idx),
                    code: "runtime_not_found".to_string(),
                    message: format!(
                        "LLM node '{}' runtime '{}' is not a known MVP runtime",
                        node.id, config.runtime
                    ),
                });
            }

            if config.agent.trim().is_empty() {
                errors.push(TaskGraphValidationError {
                    path: format!("nodes[{}].config.agent", idx),
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
                    path: format!("nodes[{}].config.agent_profile", idx),
                    code: "empty_agent_profile".to_string(),
                    message: format!("LLM node '{}' must select an agent profile", node.id),
                });
            }
        }
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
                            path: format!("nodes[{}].config.result", idx),
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
                        path: format!("nodes[{}].config", idx),
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
                    path: format!("nodes[{}].config", idx),
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
                            path: format!("nodes[{}].config.input_id", idx),
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
                        path: format!("nodes[{}].config", idx),
                        code: "invalid_config".to_string(),
                        message: format!("InputVar node '{}' config parse error: {}", node.id, e),
                    });
                }
            }
        }
        // LLM, Branch, Loop, SubGraph — already validated in dedicated functions
        _ => {}
    }
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
                path: format!("nodes[{}].config", idx),
                code: "invalid_config".to_string(),
                message: format!("SubGraph node '{}' config parse error: {}", node.id, e),
            });
        }
    }
}
