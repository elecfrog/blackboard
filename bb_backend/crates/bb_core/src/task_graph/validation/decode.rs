use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::task_graph::definition::types::{TaskGraphDefinition, TaskGraphValidationError};
use crate::task_graph::definition::upgrade::upgrade_graph;

use super::validate_graph;

pub fn parse_json_source(
    source: &[u8],
    path: &str,
) -> Result<Value, Vec<TaskGraphValidationError>> {
    serde_json::from_slice(source).map_err(|err| {
        vec![TaskGraphValidationError {
            path: path.to_string(),
            code: "invalid_json".to_string(),
            message: format!("Invalid JSON: {err}"),
        }]
    })
}

pub fn validate_graph_source(
    source: &[u8],
) -> Result<TaskGraphDefinition, Vec<TaskGraphValidationError>> {
    let value = parse_json_source(source, "$")?;
    validate_graph_value(value)
}

pub fn validate_graph_value(
    value: Value,
) -> Result<TaskGraphDefinition, Vec<TaskGraphValidationError>> {
    validate_graph_value_at(value, "")
}

pub fn validate_graph_value_at(
    value: Value,
    path_prefix: &str,
) -> Result<TaskGraphDefinition, Vec<TaskGraphValidationError>> {
    let mut graph = decode_graph_value_at(value, path_prefix)?;
    upgrade_graph(&mut graph);

    let errors = validate_graph(&graph);
    if errors.is_empty() {
        Ok(graph)
    } else {
        Err(prefix_validation_errors(path_prefix, errors))
    }
}

pub fn decode_graph_value_at(
    value: Value,
    path_prefix: &str,
) -> Result<TaskGraphDefinition, Vec<TaskGraphValidationError>> {
    let mut errors = Vec::new();
    validate_graph_shape(&value, normalized_path(path_prefix), &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    decode_value(value, normalized_path(path_prefix))
}

pub fn prefix_validation_errors(
    path_prefix: &str,
    errors: Vec<TaskGraphValidationError>,
) -> Vec<TaskGraphValidationError> {
    let prefix = normalized_path(path_prefix);
    if prefix.is_empty() {
        return errors;
    }

    errors
        .into_iter()
        .map(|mut error| {
            error.path = join_path(prefix, &error.path);
            error
        })
        .collect()
}

fn decode_value<T: DeserializeOwned>(
    value: Value,
    path: &str,
) -> Result<T, Vec<TaskGraphValidationError>> {
    serde_json::from_value(value).map_err(|err| {
        vec![TaskGraphValidationError {
            path: path_for_message(path),
            code: "invalid_shape".to_string(),
            message: format!("Task graph definition could not be decoded: {err}"),
        }]
    })
}

fn validate_graph_shape(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(object) = expect_object(value, path, "task graph definition", errors) else {
        return;
    };

    require_u64(object, path, "schema_version", errors);
    require_string(object, path, "id", errors);
    require_enum(object, path, "scope", &["system", "project"], errors);
    require_string(object, path, "title", errors);
    optional_string(object, path, "description", errors);
    require_u64(object, path, "version", errors);
    require_bool(object, path, "readonly", errors);

    if let Some(metadata) = object.get("metadata") {
        validate_metadata(metadata, &join_path(path, "metadata"), errors);
    }
    if let Some(inputs) = object.get("inputs") {
        validate_inputs(inputs, &join_path(path, "inputs"), errors);
    }
    if let Some(nodes) = object.get("nodes") {
        validate_nodes(nodes, &join_path(path, "nodes"), errors);
    } else {
        missing_field(path, "nodes", errors);
    }
    if let Some(edges) = object.get("edges") {
        validate_edges(edges, &join_path(path, "edges"), errors);
    } else {
        missing_field(path, "edges", errors);
    }
}

fn validate_metadata(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(object) = expect_object(value, path, "graph metadata", errors) else {
        return;
    };

    if let Some(run_policy) = object.get("run_policy") {
        validate_run_policy(run_policy, &join_path(path, "run_policy"), errors);
    }
}

fn validate_run_policy(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(object) = expect_object(value, path, "graph run policy", errors) else {
        return;
    };

    optional_bool(object, path, "allow_concurrent_runs", errors);
    optional_u64(object, path, "max_concurrent_runs", errors);
    optional_bool(object, path, "queue_enabled", errors);
    optional_u64(object, path, "max_queue_wait_ms", errors);
}

fn validate_inputs(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(inputs) = expect_array(value, path, "graph inputs", errors) else {
        return;
    };

    for (idx, input) in inputs.iter().enumerate() {
        let item_path = format!("{path}[{idx}]");
        let Some(object) = expect_object(input, &item_path, "graph input", errors) else {
            continue;
        };
        require_string(object, &item_path, "id", errors);
        optional_string(object, &item_path, "label", errors);
        require_string(object, &item_path, "type", errors);
        optional_string(object, &item_path, "reducer", errors);
        optional_string(object, &item_path, "channel_class", errors);
        optional_string(object, &item_path, "description", errors);
        optional_number(object, &item_path, "min", errors);
        optional_number(object, &item_path, "max", errors);
    }
}

fn validate_nodes(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(nodes) = expect_array(value, path, "graph nodes", errors) else {
        return;
    };

    for (idx, node) in nodes.iter().enumerate() {
        let node_path = format!("{path}[{idx}]");
        let Some(object) = expect_object(node, &node_path, "graph node", errors) else {
            continue;
        };

        require_string(object, &node_path, "id", errors);
        require_enum(
            object,
            &node_path,
            "type",
            &[
                "start",
                "end",
                "llm",
                "plan",
                "human_gate",
                "branch",
                "loop",
                "shell",
                "input_var",
                "sub_graph",
                "sub_pipeline",
                "llm_mutation",
                "intent_extract",
                "kb_plan",
                "manifest_merge",
                "schema_validate",
            ],
            errors,
        );
        require_string(object, &node_path, "label", errors);
        optional_string(object, &node_path, "description", errors);

        if !object.contains_key("config") {
            missing_field(&node_path, "config", errors);
        }
        if let Some(position) = object.get("position") {
            validate_position(position, &join_path(&node_path, "position"), errors);
        }
        if let Some(pins) = object.get("pins") {
            validate_pins(pins, &join_path(&node_path, "pins"), errors);
        }
    }
}

fn validate_position(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(object) = expect_object(value, path, "node position", errors) else {
        return;
    };
    require_number(object, path, "x", errors);
    require_number(object, path, "y", errors);
}

fn validate_pins(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(pins) = expect_array(value, path, "node pins", errors) else {
        return;
    };

    for (idx, pin) in pins.iter().enumerate() {
        let pin_path = format!("{path}[{idx}]");
        let Some(object) = expect_object(pin, &pin_path, "node pin", errors) else {
            continue;
        };
        require_string(object, &pin_path, "id", errors);
        require_string(object, &pin_path, "label", errors);
        require_enum(object, &pin_path, "direction", &["in", "out"], errors);
        require_enum(object, &pin_path, "category", &["exec", "data"], errors);
        optional_string(object, &pin_path, "value_type", errors);
        optional_bool(object, &pin_path, "required", errors);
    }
}

fn validate_edges(value: &Value, path: &str, errors: &mut Vec<TaskGraphValidationError>) {
    let Some(edges) = expect_array(value, path, "graph edges", errors) else {
        return;
    };

    for (idx, edge) in edges.iter().enumerate() {
        let edge_path = format!("{path}[{idx}]");
        let Some(object) = expect_object(edge, &edge_path, "graph edge", errors) else {
            continue;
        };
        require_string(object, &edge_path, "id", errors);
        require_string(object, &edge_path, "from", errors);
        require_string(object, &edge_path, "to", errors);
        require_enum(
            object,
            &edge_path,
            "kind",
            &["exec", "control", "data"],
            errors,
        );
        optional_string(object, &edge_path, "label", errors);
        optional_string(object, &edge_path, "from_pin", errors);
        optional_string(object, &edge_path, "to_pin", errors);
        optional_string(object, &edge_path, "source_handle", errors);
        optional_string(object, &edge_path, "target_handle", errors);
    }
}

fn expect_object<'a>(
    value: &'a Value,
    path: &str,
    label: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) -> Option<&'a Map<String, Value>> {
    match value.as_object() {
        Some(object) => Some(object),
        None => {
            errors.push(TaskGraphValidationError {
                path: path_for_message(path),
                code: "invalid_type".to_string(),
                message: format!(
                    "Expected {label} to be an object, got {}",
                    value_kind(value)
                ),
            });
            None
        }
    }
}

fn expect_array<'a>(
    value: &'a Value,
    path: &str,
    label: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) -> Option<&'a Vec<Value>> {
    match value.as_array() {
        Some(array) => Some(array),
        None => {
            errors.push(TaskGraphValidationError {
                path: path_for_message(path),
                code: "invalid_type".to_string(),
                message: format!("Expected {label} to be an array, got {}", value_kind(value)),
            });
            None
        }
    }
}

fn require_string(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match object.get(field) {
        Some(value) if value.is_string() => {}
        Some(value) => invalid_field_type(path, field, "string", value, errors),
        None => missing_field(path, field, errors),
    }
}

fn require_u64(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match object.get(field) {
        Some(value) if value.as_u64().is_some() => {}
        Some(value) => invalid_field_type(path, field, "unsigned integer", value, errors),
        None => missing_field(path, field, errors),
    }
}

fn require_number(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match object.get(field) {
        Some(value) if value.is_number() => {}
        Some(value) => invalid_field_type(path, field, "number", value, errors),
        None => missing_field(path, field, errors),
    }
}

fn require_bool(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match object.get(field) {
        Some(value) if value.is_boolean() => {}
        Some(value) => invalid_field_type(path, field, "boolean", value, errors),
        None => missing_field(path, field, errors),
    }
}

fn require_enum(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    allowed: &[&str],
    errors: &mut Vec<TaskGraphValidationError>,
) {
    match object.get(field) {
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => {}
        Some(Value::String(value)) => errors.push(TaskGraphValidationError {
            path: join_path(path, field),
            code: "invalid_enum_value".to_string(),
            message: format!(
                "Field '{}' value '{}' must be one of: {}",
                join_path(path, field),
                value,
                allowed.join(", ")
            ),
        }),
        Some(value) => invalid_field_type(path, field, "string", value, errors),
        None => missing_field(path, field, errors),
    }
}

fn optional_string(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if let Some(value) = object.get(field) {
        if !value.is_null() && !value.is_string() {
            invalid_field_type(path, field, "string", value, errors);
        }
    }
}

fn optional_number(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if let Some(value) = object.get(field) {
        if !value.is_null() && !value.is_number() {
            invalid_field_type(path, field, "number", value, errors);
        }
    }
}

fn optional_u64(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if let Some(value) = object.get(field) {
        if !value.is_null() && value.as_u64().is_none() {
            invalid_field_type(path, field, "unsigned integer", value, errors);
        }
    }
}

fn optional_bool(
    object: &Map<String, Value>,
    path: &str,
    field: &str,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    if let Some(value) = object.get(field) {
        if !value.is_null() && !value.is_boolean() {
            invalid_field_type(path, field, "boolean", value, errors);
        }
    }
}

fn missing_field(path: &str, field: &str, errors: &mut Vec<TaskGraphValidationError>) {
    errors.push(TaskGraphValidationError {
        path: join_path(path, field),
        code: "missing_field".to_string(),
        message: format!("Missing required field '{}'", join_path(path, field)),
    });
}

fn invalid_field_type(
    path: &str,
    field: &str,
    expected: &str,
    value: &Value,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    errors.push(TaskGraphValidationError {
        path: join_path(path, field),
        code: "invalid_type".to_string(),
        message: format!(
            "Expected field '{}' to be {}, got {}",
            join_path(path, field),
            expected,
            value_kind(value)
        ),
    });
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn normalized_path(path: &str) -> &str {
    path.trim_matches('.')
}

fn path_for_message(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}

fn join_path(base: &str, field: &str) -> String {
    if base.is_empty() || base == "$" {
        field.to_string()
    } else if field.is_empty() {
        base.to_string()
    } else if field.starts_with('[') {
        format!("{base}{field}")
    } else {
        format!("{base}.{field}")
    }
}
