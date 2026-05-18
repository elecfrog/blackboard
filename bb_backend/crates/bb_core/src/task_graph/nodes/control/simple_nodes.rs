use std::collections::HashMap;

use chrono::Utc;

use crate::task_graph::definition::types::{
    DataValueConfig, EndConfig, HumanGateConfig, InputVarConfig, PinValueType, TaskGraphError,
    TaskGraphNode,
};
use crate::task_graph::nodes::eval;
use crate::task_graph::pregel::outcome::{NodeOutcome, SideEffect};
use crate::task_graph::run_state::{
    NodeRunStatus, PausedAction, RunPaused, TaskGraphRun, TaskGraphRunNode,
};

pub fn execute_start_node(
    node: &TaskGraphNode,
    _run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&crate::task_graph::definition::types::TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let now = Utc::now().to_rfc3339();
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: None,
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

// ─── End Node ────────────────────────────────────────────────────────────────

pub fn execute_end_node(node: &TaskGraphNode) -> Result<NodeOutcome, TaskGraphError> {
    let config: EndConfig =
        serde_json::from_value(node.config.clone()).unwrap_or_else(|_| EndConfig {
            result: "succeeded".to_string(),
        });

    let now = Utc::now().to_rfc3339();
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: None,
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: Some(config.result),
        control: vec![],
        graph_mutations: vec![],
    })
}

// ─── InputVar Node ───────────────────────────────────────────────────────────

pub fn execute_input_var_node(
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&crate::task_graph::definition::types::TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: InputVarConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let value = run
        .context
        .input
        .get(&config.input_id)
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let now = Utc::now().to_rfc3339();

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(value),
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Read input: {}", config.input_id)),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

// ─── DataValue Node ─────────────────────────────────────────────────────────

pub fn execute_data_value_node(
    opts: &crate::task_graph::pregel::runner::RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: DataValueConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let value = resolve_data_value(opts, &config, run);
    let now = Utc::now().to_rfc3339();

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(value),
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Emitted {:?} data value", config.value_type)),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

fn resolve_data_value(
    opts: &crate::task_graph::pregel::runner::RunnerOptions,
    config: &DataValueConfig,
    run: &TaskGraphRun,
) -> serde_json::Value {
    match &config.value {
        serde_json::Value::String(raw) => {
            let rendered = eval::render_prompt_template(
                raw,
                &run.project,
                &opts.workspace_root,
                &opts.scripts_dir,
                &run.context,
                None,
            );
            match config.value_type {
                PinValueType::Json | PinValueType::Array | PinValueType::Any => {
                    serde_json::from_str(&rendered).unwrap_or(serde_json::Value::String(rendered))
                }
                PinValueType::Int => rendered.trim().parse::<i64>().map_or_else(
                    |_| serde_json::Value::String(rendered),
                    serde_json::Value::from,
                ),
                PinValueType::Float => rendered
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map_or_else(
                        || serde_json::Value::String(rendered),
                        serde_json::Value::Number,
                    ),
                PinValueType::Bool => rendered.trim().parse::<bool>().map_or_else(
                    |_| serde_json::Value::String(rendered),
                    serde_json::Value::Bool,
                ),
                _ => serde_json::Value::String(rendered),
            }
        }
        other => resolve_data_value_tree(opts, run, other),
    }
}

fn resolve_data_value_tree(
    opts: &crate::task_graph::pregel::runner::RunnerOptions,
    run: &TaskGraphRun,
    value: &serde_json::Value,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(raw) => {
            let trimmed = raw.trim();
            if let Some(expr) = full_mustache_expr(trimmed) {
                if let Some(value) = eval::resolve_template_expr_as_value(
                    expr,
                    &run.project,
                    &opts.workspace_root,
                    &opts.scripts_dir,
                    &run.context,
                ) {
                    return value;
                }
            }
            serde_json::Value::String(eval::render_prompt_template(
                raw,
                &run.project,
                &opts.workspace_root,
                &opts.scripts_dir,
                &run.context,
                None,
            ))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| resolve_data_value_tree(opts, run, value))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), resolve_data_value_tree(opts, run, value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn full_mustache_expr(raw: &str) -> Option<&str> {
    if raw.starts_with("{{") && raw.ends_with("}}") {
        return Some(raw.trim_start_matches("{{").trim_end_matches("}}").trim());
    }
    None
}

// ─── HumanGate Node ──────────────────────────────────────────────────────────

pub fn execute_human_gate_node(node: &TaskGraphNode) -> Result<NodeOutcome, TaskGraphError> {
    let config: HumanGateConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let now = Utc::now().to_rfc3339();
    let paused = RunPaused {
        node_id: node.id.clone(),
        reason: "waiting_for_human_gate".to_string(),
        actions: config
            .actions
            .iter()
            .map(|a| PausedAction {
                id: a.id.clone(),
                label: a.label.clone(),
                result: a.result.clone(),
            })
            .collect(),
    };

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Paused,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Paused,
            started_at: Some(now),
            completed_at: None,
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!("Waiting for human: {}", config.title)),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![SideEffect::RunPaused(paused)],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}
