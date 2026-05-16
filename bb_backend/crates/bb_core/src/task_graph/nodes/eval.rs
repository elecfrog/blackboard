//! Branch/loop condition evaluation and template rendering.
//!
//! Pure logic used by the runner — no I/O, no process spawning.
//! Contains: JSONPath resolution, condition evaluation, prompt template rendering.

use std::path::Path;

use crate::task_graph::definition::types::{BranchConfig, LoopConfig};
use crate::task_graph::run_state::RunContext;

pub(in crate::task_graph::nodes) type NodeInputs = serde_json::Map<String, serde_json::Value>;

// ─── Branch condition evaluation ─────────────────────────────────────────────

/// Evaluate a branch node's rules against the run context. Returns the selected rule_id.
pub fn evaluate_branch(config: &BranchConfig, context: &RunContext) -> String {
    let input_value = if let Some(ref input_ref) = config.input_ref {
        resolve_json_path(input_ref, context)
    } else {
        serde_json::Value::Null
    };

    for rule in &config.rules {
        if evaluate_condition(&rule.when, &input_value) {
            return rule.id.clone();
        }
    }

    config.default_rule_id.clone()
}

/// Resolve a simplified JSONPath reference against run context.
/// Supported: $.nodes.<node-id>.output, $.input.<key>, $.nodes.<node-id>.output.<path>
pub(super) fn resolve_json_path(path: &str, context: &RunContext) -> serde_json::Value {
    let path = path.trim();
    if !path.starts_with("$.") {
        return serde_json::Value::Null;
    }

    let segments: Vec<&str> = path[2..].split('.').collect();
    if segments.is_empty() {
        return serde_json::Value::Null;
    }

    match segments[0] {
        "nodes" if segments.len() >= 3 => {
            let node_id = segments[1];
            if segments[2] == "output" {
                let node_output = context
                    .node_outputs
                    .get(node_id)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                if segments.len() == 3 {
                    return node_output;
                }
                return navigate_value(&node_output, &segments[3..]);
            }
            serde_json::Value::Null
        }
        "input" => {
            if segments.len() == 1 {
                return context.input.clone();
            }
            navigate_value(&context.input, &segments[1..])
        }
        _ => serde_json::Value::Null,
    }
}

pub(super) fn resolve_template_value(
    value: &serde_json::Value,
    context: &RunContext,
) -> serde_json::Value {
    let Some(raw) = value.as_str() else {
        return value.clone();
    };
    let trimmed = raw.trim();
    if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
        let inner = trimmed
            .trim_start_matches("{{")
            .trim_end_matches("}}")
            .trim();
        if let Some(path) = inner.strip_prefix("inputs.") {
            return resolve_json_path(&format!("$.input.{}", path), context);
        }
        if let Some(path) = inner.strip_prefix("input.") {
            return resolve_json_path(&format!("$.input.{}", path), context);
        }
        if inner.starts_with("$.") {
            return resolve_json_path(inner, context);
        }
    }
    if trimmed.starts_with("$.") {
        return resolve_json_path(trimmed, context);
    }
    value.clone()
}

pub(super) fn prompt_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub(in crate::task_graph) fn render_prompt_template(
    template: &str,
    project: &str,
    root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
    node_inputs: Option<&serde_json::Value>,
) -> String {
    let node_inputs = resolve_node_inputs(node_inputs, project, root, scripts_dir, context);

    // 使用正则匹配所有 {{...}} 占位符并替换
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        if let Some(end) = after_open.find("}}") {
            let expr = after_open[..end].trim();
            let replacement =
                resolve_template_expr(expr, project, root, scripts_dir, context, &node_inputs);
            rendered.push_str(&replacement);
            rest = &after_open[end + 2..];
        } else {
            // 没有匹配的 }}，原样保留
            rendered.push_str("{{");
            rest = after_open;
        }
    }
    rendered.push_str(rest);
    rendered
}

/// 解析单个模板表达式并返回替换后的字符串。
/// 支持的表达式：
/// - `env.project` — 系统环境变量 project
/// - `env.root` — 系统环境变量 workspace root
/// - `env.scripts_dir` — Blackboard scripts directory
/// - `inputs.xxx` — graph input 变量
/// - `data.<node-id>.output` — 当前 Pull task 注入的 data edge 输入
/// - `data.<node-id>.artifact_path` — 上游 data output 中的 artifact_path
/// - `nodes.xxx.output` — 上游节点输出（JSONPath 风格）
fn resolve_template_expr(
    expr: &str,
    project: &str,
    root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
    node_inputs: &NodeInputs,
) -> String {
    resolve_template_expr_value(expr, project, root, scripts_dir, context, Some(node_inputs))
        .map(|value| prompt_value(&value))
        .unwrap_or_default()
}

fn resolve_template_expr_value(
    expr: &str,
    project: &str,
    root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
    node_inputs: Option<&NodeInputs>,
) -> Option<serde_json::Value> {
    if let Some(env_key) = expr.strip_prefix("env.") {
        return match env_key {
            "project" => Some(serde_json::Value::String(project.to_string())),
            "root" => Some(serde_json::Value::String(path_template_string(root))),
            "scripts_dir" => Some(serde_json::Value::String(path_template_string(scripts_dir))),
            _ => None,
        };
    }

    if let Some(input_key) = expr.strip_prefix("inputs.") {
        if let Some(value) = resolve_node_input_path(input_key, node_inputs) {
            return Some(value);
        }
        return Some(resolve_json_path(
            &format!("$.input.{}", input_key),
            context,
        ));
    }

    if let Some(data_path) = expr.strip_prefix("data.") {
        return Some(resolve_json_path(
            &format!("$.input.__data.{}", data_path),
            context,
        ));
    }

    if let Some(node_path) = expr.strip_prefix("nodes.") {
        return Some(resolve_json_path(
            &format!("$.nodes.{}", node_path),
            context,
        ));
    }

    None
}

pub(in crate::task_graph::nodes) fn resolve_node_inputs(
    bindings: Option<&serde_json::Value>,
    project: &str,
    root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
) -> NodeInputs {
    let Some(serde_json::Value::Object(map)) = bindings else {
        return NodeInputs::new();
    };

    map.iter()
        .map(|(key, value)| {
            (
                key.clone(),
                resolve_node_input_value(value, project, root, scripts_dir, context),
            )
        })
        .collect()
}

fn resolve_node_input_value(
    value: &serde_json::Value,
    project: &str,
    root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(raw) => {
            let trimmed = raw.trim();
            if let Some(expr) = full_mustache_expr(trimmed) {
                return resolve_template_expr_value(
                    expr,
                    project,
                    root,
                    scripts_dir,
                    context,
                    None,
                )
                .unwrap_or(serde_json::Value::Null);
            }
            serde_json::Value::String(render_prompt_template(
                raw,
                project,
                root,
                scripts_dir,
                context,
                None,
            ))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| resolve_node_input_value(value, project, root, scripts_dir, context))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        resolve_node_input_value(value, project, root, scripts_dir, context),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn resolve_node_input_path(
    path: &str,
    node_inputs: Option<&NodeInputs>,
) -> Option<serde_json::Value> {
    let node_inputs = node_inputs?;
    let mut segments = path.split('.');
    let key = segments.next()?;
    let mut value = node_inputs.get(key)?.clone();
    let rest: Vec<&str> = segments.collect();
    if !rest.is_empty() {
        value = navigate_value(&value, &rest);
    }
    Some(value)
}

fn full_mustache_expr(raw: &str) -> Option<&str> {
    if raw.starts_with("{{") && raw.ends_with("}}") {
        Some(raw.trim_start_matches("{{").trim_end_matches("}}").trim())
    } else {
        None
    }
}

fn path_template_string(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub(super) fn resolve_loop_max_iterations(config: &LoopConfig, context: &RunContext) -> u32 {
    let Some(reference) = config.max_iterations_ref.as_deref() else {
        return config.max_iterations;
    };
    let value = resolve_template_value(&serde_json::Value::String(reference.to_string()), context);
    value_to_f64(&value)
        .map(|num| num.round().clamp(1.0, 50.0) as u32)
        .unwrap_or(config.max_iterations)
}

/// Navigate a JSON value through a dot path (supports .length for arrays/strings).
pub(super) fn navigate_value(val: &serde_json::Value, segments: &[&str]) -> serde_json::Value {
    let mut current = val.clone();
    for &seg in segments {
        if seg == "length" {
            return match &current {
                serde_json::Value::Array(arr) => {
                    serde_json::Value::Number(serde_json::Number::from(arr.len()))
                }
                serde_json::Value::String(s) => {
                    serde_json::Value::Number(serde_json::Number::from(s.len()))
                }
                _ => serde_json::Value::Number(serde_json::Number::from(0)),
            };
        }
        current = match &current {
            serde_json::Value::Object(map) => {
                map.get(seg).cloned().unwrap_or(serde_json::Value::Null)
            }
            serde_json::Value::Array(arr) => {
                if let Ok(idx) = seg.parse::<usize>() {
                    arr.get(idx).cloned().unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::Value::Null
                }
            }
            _ => serde_json::Value::Null,
        };
    }
    current
}

/// Evaluate a single condition (`when` clause).
pub(super) fn evaluate_condition(when: &serde_json::Value, input: &serde_json::Value) -> bool {
    let Some(obj) = when.as_object() else {
        return false;
    };

    let op = obj.get("op").and_then(|v| v.as_str()).unwrap_or("always");

    if op == "always" {
        return true;
    }

    let compare_value = if let Some(path_val) = obj.get("path").and_then(|v| v.as_str()) {
        let segs: Vec<&str> = if let Some(stripped) = path_val.strip_prefix("$.") {
            stripped.split('.').collect()
        } else {
            path_val.split('.').collect()
        };
        navigate_value(input, &segs)
    } else {
        input.clone()
    };

    let expected = obj.get("value").cloned().unwrap_or(serde_json::Value::Null);

    match op {
        "exists" => !compare_value.is_null(),
        "equals" => compare_value == expected,
        "not_equals" => compare_value != expected,
        ">" | "gt" => compare_numbers(&compare_value, &expected, |a, b| a > b),
        ">=" | "gte" => compare_numbers(&compare_value, &expected, |a, b| a >= b),
        "<" | "lt" => compare_numbers(&compare_value, &expected, |a, b| a < b),
        "<=" | "lte" => compare_numbers(&compare_value, &expected, |a, b| a <= b),
        "contains" => {
            if let (Some(s), Some(needle)) = (compare_value.as_str(), expected.as_str()) {
                s.contains(needle)
            } else if let Some(arr) = compare_value.as_array() {
                arr.contains(&expected)
            } else {
                false
            }
        }
        "is_empty" => match &compare_value {
            serde_json::Value::Array(arr) => arr.is_empty(),
            serde_json::Value::String(s) => s.is_empty(),
            serde_json::Value::Null => true,
            _ => false,
        },
        "not_empty" => match &compare_value {
            serde_json::Value::Array(arr) => !arr.is_empty(),
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Null => false,
            _ => true,
        },
        "truthy" => is_truthy(&compare_value),
        "falsy" => !is_truthy(&compare_value),
        _ => false,
    }
}

fn compare_numbers(a: &serde_json::Value, b: &serde_json::Value, f: fn(f64, f64) -> bool) -> bool {
    let a_num = value_to_f64(a);
    let b_num = value_to_f64(b);
    match (a_num, b_num) {
        (Some(a), Some(b)) => f(a, b),
        _ => false,
    }
}

pub(super) fn value_to_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn is_truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(_) => true,
    }
}

// ─── Loop condition evaluation ───────────────────────────────────────────────

/// Check if the loop condition is still true (should continue iterating).
pub fn evaluate_loop_condition(config: &LoopConfig, context: &RunContext) -> bool {
    let Some(ref condition) = config.condition else {
        return true;
    };

    let condition_obj = match serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(
        serde_json::Value::Object(condition.as_object().cloned().unwrap_or_default()),
    ) {
        Ok(m) => m,
        Err(_) => return true,
    };

    let input_value = condition_obj
        .get("input_ref")
        .and_then(|v| v.as_str())
        .map(|path| resolve_json_path(path, context))
        .unwrap_or(serde_json::Value::Null);

    let mut when = serde_json::Map::new();
    if let Some(path) = condition_obj.get("path") {
        when.insert("path".to_string(), path.clone());
    }
    if let Some(op) = condition_obj.get("op") {
        when.insert("op".to_string(), op.clone());
    }
    if let Some(value) = condition_obj.get("value") {
        when.insert("value".to_string(), value.clone());
    }

    evaluate_condition(&serde_json::Value::Object(when), &input_value)
}
