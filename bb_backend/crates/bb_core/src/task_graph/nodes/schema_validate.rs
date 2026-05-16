use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::task_graph::definition::types::{TaskGraphError, TaskGraphNode};
use crate::task_graph::nodes::eval;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode};

const RUNTIME_NAME: &str = "schema_validate";
const DEFAULT_VALUE_KEY: &str = "value";
const MAX_ERRORS: usize = 80;

#[derive(Debug, Clone, Deserialize)]
struct SchemaValidateConfig {
    #[serde(default)]
    inputs: Option<Value>,
    schema: Value,
    #[serde(default = "default_value_key")]
    value_key: String,
    #[serde(default)]
    value_keys: Vec<String>,
    #[serde(default)]
    source: Option<SchemaSourceConfig>,
    #[serde(default)]
    fail_on_invalid: bool,
    #[serde(default)]
    artifact_path: Option<String>,
    #[serde(default)]
    schema_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchemaSourceConfig {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

pub(crate) fn execute_schema_validate_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: SchemaValidateConfig =
        serde_json::from_value(node.config.clone()).map_err(|source| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source,
        })?;

    let inputs = eval::resolve_node_inputs(
        config.inputs.as_ref(),
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
    );

    let value = resolve_source_value(&config, &inputs, run);
    let mut errors = Vec::new();
    validate_json_schema(&value, &config.schema, "$", &mut errors);
    let valid = errors.is_empty();

    let artifact_path = if valid {
        write_artifact_if_requested(&config, &inputs, &value, opts, run)?
    } else {
        None
    };

    let output = json!({
        "valid": valid,
        "schema_name": config.schema_name,
        "errors": errors,
        "data": value,
        "artifact_path": artifact_path,
        "artifact_type": "json",
    });

    if valid || !config.fail_on_invalid {
        Ok(outcome(
            node,
            NodeRunStatus::Succeeded,
            Some(output),
            None,
            if valid {
                "schema validation passed".to_string()
            } else {
                "schema validation failed; continuing for repair".to_string()
            },
        ))
    } else {
        let message = validation_message(output.get("errors"));
        Ok(outcome(
            node,
            NodeRunStatus::Failed,
            Some(output),
            Some(NodeError {
                code: "invalid_schema".to_string(),
                message: message.clone(),
            }),
            message,
        ))
    }
}

fn default_value_key() -> String {
    DEFAULT_VALUE_KEY.to_string()
}

fn resolve_source_value(
    config: &SchemaValidateConfig,
    inputs: &eval::NodeInputs,
    run: &TaskGraphRun,
) -> Value {
    if let Some(source) = &config.source {
        if source.kind.as_deref() == Some("node_outputs_by_prefix") {
            let prefix = source.prefix.as_deref().unwrap_or_default();
            let exclude = source.exclude.iter().collect::<BTreeSet<_>>();
            let mut keys = run
                .context
                .node_outputs
                .keys()
                .filter(|key| key.starts_with(prefix) && !exclude.contains(key))
                .cloned()
                .collect::<Vec<_>>();
            keys.sort();

            let mut values = Map::new();
            for key in keys {
                if let Some(value) = run.context.node_outputs.get(&key) {
                    values.insert(key, value.clone());
                }
            }
            return Value::Object(values);
        }
    }

    config
        .value_keys
        .iter()
        .map(|key| key.trim())
        .chain(std::iter::once(config.value_key.trim()))
        .find_map(|key| inputs.get(key).filter(|value| !value.is_null()).cloned())
        .or_else(|| inputs.get(DEFAULT_VALUE_KEY).cloned())
        .unwrap_or(Value::Null)
}

fn write_artifact_if_requested(
    config: &SchemaValidateConfig,
    inputs: &eval::NodeInputs,
    value: &Value,
    opts: &RunnerOptions,
    run: &TaskGraphRun,
) -> Result<Option<String>, TaskGraphError> {
    let Some(template) = config.artifact_path.as_deref() else {
        return Ok(None);
    };
    let rendered = eval::render_prompt_template(
        template,
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
        Some(&Value::Object(inputs.clone())),
    );
    let trimmed = rendered.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut path = PathBuf::from(trimmed);
    if path.is_relative() {
        path = opts.workspace_root.join(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| TaskGraphError::Parse {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, bytes).map_err(|source| TaskGraphError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(Some(path.display().to_string().replace('\\', "/")))
}

fn validate_json_schema(value: &Value, schema: &Value, path: &str, errors: &mut Vec<String>) {
    if errors.len() >= MAX_ERRORS {
        return;
    }
    let Some(schema_obj) = schema.as_object() else {
        errors.push(format!("{path}: schema must be an object"));
        return;
    };

    if let Some(type_spec) = schema_obj.get("type") {
        if !matches_type(value, type_spec) {
            errors.push(format!(
                "{path}: expected type {}, got {}",
                type_label(type_spec),
                value_type(value)
            ));
            return;
        }
    }

    if let Some(enum_values) = schema_obj.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|candidate| candidate == value) {
            errors.push(format!("{path}: value is not in enum"));
            return;
        }
    }

    if let Some(expected) = schema_obj.get("const") {
        if value != expected {
            errors.push(format!("{path}: value does not match const"));
            return;
        }
    }

    match value {
        Value::Object(object) => validate_object(object, schema_obj, path, errors),
        Value::Array(items) => validate_array(items, schema_obj, path, errors),
        Value::String(text) => validate_string(text, schema_obj, path, errors),
        Value::Number(number) => validate_number(number, schema_obj, path, errors),
        _ => {}
    }
}

fn validate_object(
    object: &Map<String, Value>,
    schema: &Map<String, Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(min_properties) = schema.get("minProperties").and_then(Value::as_u64) {
        if object.len() < min_properties as usize {
            errors.push(format!(
                "{path}: expected at least {min_properties} properties, got {}",
                object.len()
            ));
        }
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(field) {
                errors.push(format!("{path}.{field}: required property is missing"));
            }
        }
    }

    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for (field, field_schema) in &properties {
        if let Some(field_value) = object.get(field) {
            validate_json_schema(field_value, field_schema, &join_path(path, field), errors);
        }
    }

    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            for field in object.keys() {
                if !properties.contains_key(field) {
                    errors.push(format!(
                        "{path}.{field}: additional property is not allowed"
                    ));
                }
            }
        }
        Some(Value::Object(additional_schema)) => {
            for (field, field_value) in object {
                if !properties.contains_key(field) {
                    validate_json_schema(
                        field_value,
                        &Value::Object(additional_schema.clone()),
                        &join_path(path, field),
                        errors,
                    );
                }
            }
        }
        _ => {}
    }
}

fn validate_array(
    items: &[Value],
    schema: &Map<String, Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
        if items.len() < min_items as usize {
            errors.push(format!(
                "{path}: expected at least {min_items} items, got {}",
                items.len()
            ));
        }
    }

    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_json_schema(item, item_schema, &format!("{path}[{index}]"), errors);
        }
    }
}

fn validate_string(text: &str, schema: &Map<String, Value>, path: &str, errors: &mut Vec<String>) {
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        if text.chars().count() < min_length as usize {
            errors.push(format!("{path}: expected minLength {min_length}"));
        }
    }
}

fn validate_number(
    number: &serde_json::Number,
    schema: &Map<String, Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if number.as_f64().is_some_and(|value| value < minimum) {
            errors.push(format!("{path}: expected minimum {minimum}"));
        }
    }
}

fn matches_type(value: &Value, type_spec: &Value) -> bool {
    match type_spec {
        Value::String(name) => matches_single_type(value, name),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .any(|name| matches_single_type(value, name)),
        _ => true,
    }
}

fn matches_single_type(value: &Value, expected: &str) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn type_label(type_spec: &Value) -> String {
    match type_spec {
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "unknown".to_string(),
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn join_path(base: &str, key: &str) -> String {
    if base == "$" {
        format!("$.{key}")
    } else {
        format!("{base}.{key}")
    }
}

fn validation_message(errors: Option<&Value>) -> String {
    let messages = errors
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .take(5)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if messages.is_empty() {
        "schema validation failed".to_string()
    } else {
        format!("schema validation failed: {}", messages.join("; "))
    }
}

fn outcome(
    node: &TaskGraphNode,
    status: NodeRunStatus,
    output: Option<Value>,
    error: Option<NodeError>,
    log_tail: String,
) -> NodeOutcome {
    let now = Utc::now().to_rfc3339();
    let exit_code = if status == NodeRunStatus::Failed {
        Some(1)
    } else {
        Some(0)
    };
    NodeOutcome {
        node_id: node.id.clone(),
        status,
        output,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code,
            error,
            output_artifact: None,
            log_tail: Some(log_tail),
            child_run_id: None,
            runtime: Some(RUNTIME_NAME.to_string()),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_nested_required_array_items() {
        let mut errors = Vec::new();
        validate_json_schema(
            &json!({
                "scout-structure": {
                    "findings": [{
                        "title": "Graph",
                        "summary": "summary",
                        "source_anchors": ["src/lib.rs:1"]
                    }]
                }
            }),
            &json!({
                "type": "object",
                "minProperties": 1,
                "additionalProperties": {
                    "type": "object",
                    "required": ["findings"],
                    "properties": {
                        "findings": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "required": ["title", "summary", "source_anchors"],
                                "properties": {
                                    "title": {"type": "string", "minLength": 1},
                                    "summary": {"type": "string", "minLength": 1},
                                    "source_anchors": {
                                        "type": "array",
                                        "minItems": 1,
                                        "items": {"type": "string", "minLength": 1}
                                    }
                                }
                            }
                        }
                    }
                }
            }),
            "$",
            &mut errors,
        );

        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn reports_schema_errors() {
        let mut errors = Vec::new();
        validate_json_schema(
            &json!({"review_status": "NOPE"}),
            &json!({
                "type": "object",
                "required": ["needs_repair", "review_status"],
                "properties": {
                    "needs_repair": {"type": "boolean"},
                    "review_status": {"type": "string", "enum": ["PASS", "WARN", "FAIL"]}
                }
            }),
            "$",
            &mut errors,
        );

        assert!(errors.iter().any(|error| error.contains("needs_repair")));
        assert!(errors.iter().any(|error| error.contains("enum")));
    }
}
