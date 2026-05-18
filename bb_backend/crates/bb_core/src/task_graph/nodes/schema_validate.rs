use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::fs_util::resolve_slash;
use crate::task_graph::definition::types::{TaskGraphError, TaskGraphNode};
use crate::task_graph::nodes::eval;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, ArtifactContentType, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};

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
    artifact_scope: Option<String>,
    #[serde(default)]
    artifact_name: Option<String>,
    #[serde(default)]
    schema_name: Option<String>,
    #[serde(default)]
    repair: Option<SchemaRepairConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchemaSourceConfig {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    ids: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchemaRepairConfig {
    #[serde(default)]
    kind: Option<String>,
}

pub fn execute_schema_validate_node(
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

    let raw_value = resolve_source_value(&config, &inputs, run);
    let (value, repair_notes) = repair_source_value(&config, raw_value);
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
        "repair_notes": repair_notes,
        "data": value,
        "artifact_path": artifact_path,
        "artifact_type": "json",
    });
    let has_repair_notes = output
        .get("repair_notes")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());

    if valid || !config.fail_on_invalid {
        Ok(outcome(
            node,
            NodeRunStatus::Succeeded,
            Some(output),
            None,
            if valid {
                if has_repair_notes {
                    "schema validation passed after deterministic repair".to_string()
                } else {
                    "schema validation passed".to_string()
                }
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

fn repair_source_value(config: &SchemaValidateConfig, value: Value) -> (Value, Vec<String>) {
    match config
        .repair
        .as_ref()
        .and_then(|repair| repair.kind.as_deref())
    {
        Some("code_research_scout_outputs" | "external_research_scout_outputs") => {
            repair_research_scout_outputs(value)
        }
        _ => (value, vec![]),
    }
}

fn repair_research_scout_outputs(value: Value) -> (Value, Vec<String>) {
    let Value::Object(map) = value else {
        return (value, vec![]);
    };

    let mut notes = Vec::new();
    let mut repaired = Map::new();
    for (scout_id, scout_output) in map {
        let (normalized, scout_notes) = normalize_scout_output(&scout_id, scout_output);
        notes.extend(scout_notes);
        repaired.insert(scout_id, normalized);
    }

    (Value::Object(repaired), notes)
}

fn normalize_scout_output(scout_id: &str, value: Value) -> (Value, Vec<String>) {
    match value {
        Value::Object(mut object) => {
            let mut notes = Vec::new();
            if let Some(findings) = object.remove("findings") {
                let (normalized, finding_notes) = normalize_findings(scout_id, findings);
                object.insert("findings".to_string(), normalized);
                notes.extend(finding_notes);
            } else if looks_like_flat_finding(&object) {
                let finding = normalize_finding_object(scout_id, Value::Object(object.clone()))
                    .unwrap_or_else(|| {
                        json!({
                            "title": scout_id,
                            "summary": "Scout returned an incomplete flat finding.",
                            "source_anchors": []
                        })
                    });
                object.insert("findings".to_string(), Value::Array(vec![finding]));
                notes.push(format!("{scout_id}: wrapped flat finding into findings[]"));
            } else {
                object.insert("findings".to_string(), Value::Array(vec![]));
                notes.push(format!(
                    "{scout_id}: inserted empty findings[] for metadata-only scout output"
                ));
            }

            ensure_string_array(&mut object, "risks");
            ensure_string_array(&mut object, "coverage_notes");
            (Value::Object(object), notes)
        }
        Value::String(text) => {
            let summary = text.trim();
            if summary.is_empty() {
                (
                    json!({
                        "findings": [],
                        "risks": [],
                        "coverage_notes": ["Scout returned an empty string."]
                    }),
                    vec![format!(
                        "{scout_id}: converted empty string output to empty findings[]"
                    )],
                )
            } else {
                (
                    json!({
                        "findings": [{
                            "title": scout_id,
                            "summary": summary,
                            "source_anchors": []
                        }],
                        "risks": [],
                        "coverage_notes": ["Scout returned a string; wrapped as one finding."]
                    }),
                    vec![format!("{scout_id}: wrapped string output as one finding")],
                )
            }
        }
        Value::Null => (
            json!({
                "findings": [],
                "risks": [],
                "coverage_notes": ["Scout output was null."]
            }),
            vec![format!(
                "{scout_id}: converted null output to empty findings[]"
            )],
        ),
        other => (
            json!({
                "findings": [{
                    "title": scout_id,
                    "summary": prompt_value_for_repair(&other),
                    "source_anchors": []
                }],
                "risks": [],
                "coverage_notes": ["Scout returned a non-object value; wrapped as one finding."]
            }),
            vec![format!(
                "{scout_id}: wrapped non-object output as one finding"
            )],
        ),
    }
}

fn looks_like_flat_finding(object: &Map<String, Value>) -> bool {
    object.contains_key("title")
        || object.contains_key("summary")
        || object.contains_key("source_anchors")
}

fn normalize_findings(scout_id: &str, value: Value) -> (Value, Vec<String>) {
    let mut notes = Vec::new();
    let findings = match value {
        Value::Array(items) => items,
        Value::Object(_) | Value::String(_) => {
            notes.push(format!(
                "{scout_id}: wrapped non-array findings into findings[]"
            ));
            vec![value]
        }
        _ => {
            notes.push(format!("{scout_id}: replaced invalid findings with []"));
            vec![]
        }
    };

    let mut normalized = Vec::new();
    for finding in findings {
        if let Some(finding) = normalize_finding_object(scout_id, finding) {
            normalized.push(finding);
        } else {
            notes.push(format!("{scout_id}: dropped empty finding item"));
        }
    }

    (Value::Array(normalized), notes)
}

fn normalize_finding_object(scout_id: &str, value: Value) -> Option<Value> {
    match value {
        Value::Object(mut object) => {
            let title = object
                .remove("title")
                .and_then(string_value)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| scout_id.to_string());
            let summary = object
                .remove("summary")
                .and_then(string_value)
                .filter(|value| !value.trim().is_empty())?;
            let source_anchors = object
                .remove("source_anchors")
                .map_or_else(|| Value::Array(vec![]), string_array_value);
            Some(json!({
                "title": title,
                "summary": summary,
                "source_anchors": source_anchors,
            }))
        }
        Value::String(text) => {
            let summary = text.trim();
            if summary.is_empty() {
                None
            } else {
                Some(json!({
                    "title": scout_id,
                    "summary": summary,
                    "source_anchors": []
                }))
            }
        }
        _ => None,
    }
}

fn ensure_string_array(object: &mut Map<String, Value>, key: &str) {
    let value = object
        .remove(key)
        .map_or_else(|| Value::Array(vec![]), string_array_value);
    object.insert(key.to_string(), value);
}

fn string_array_value(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .filter_map(string_value)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(Value::String)
                .collect(),
        ),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                Value::Array(vec![])
            } else {
                Value::Array(vec![Value::String(text.to_string())])
            }
        }
        _ => Value::Array(vec![]),
    }
}

fn string_value(value: Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn prompt_value_for_repair(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other)
            .unwrap_or_else(|_| "Unserializable scout output".to_string()),
    }
}

fn resolve_source_value(
    config: &SchemaValidateConfig,
    inputs: &eval::NodeInputs,
    run: &TaskGraphRun,
) -> Value {
    if let Some(source) = &config.source {
        if source.kind.as_deref() == Some("node_outputs_by_ids") {
            let exclude = source.exclude.iter().cloned().collect::<BTreeSet<_>>();
            let mut values = Map::new();
            for id in source
                .ids
                .iter()
                .map(|id| id.trim())
                .filter(|id| !id.is_empty() && !exclude.contains(*id))
            {
                values.insert(
                    id.to_string(),
                    run.context
                        .node_outputs
                        .get(id)
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
            return Value::Object(values);
        }

        if source.kind.as_deref() == Some("node_outputs_by_prefix") {
            let prefix = source.prefix.as_deref().unwrap_or_default();
            let exclude = source.exclude.iter().cloned().collect::<BTreeSet<_>>();
            let mut keys = run
                .context
                .node_outputs
                .keys()
                .filter(|key| key.starts_with(prefix) && !exclude.contains((*key).as_str()))
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
    if config.artifact_scope.as_deref() == Some("run") {
        let artifact_id = config
            .artifact_name
            .as_deref()
            .map(trim_json_extension)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "{}-output",
                    node_safe_name(config.schema_name.as_deref()).trim_matches('-')
                )
            });
        let content =
            serde_json::to_string_pretty(value).map_err(|source| TaskGraphError::Parse {
                path: PathBuf::from(&artifact_id),
                source,
            })?;
        let artifact = run_state::write_artifact(
            &opts.workspace_root,
            &opts.project,
            &opts.run_id,
            &artifact_id,
            &content,
            ArtifactContentType::Json,
        )?;
        let path = opts
            .workspace_root
            .join("runtime")
            .join("task_graph_runs")
            .join(&opts.project)
            .join(&opts.run_id)
            .join(&artifact.path);
        return Ok(Some(resolve_slash(&path)));
    }

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
    Ok(Some(resolve_slash(&path)))
}

fn trim_json_extension(value: &str) -> String {
    value.trim().trim_end_matches(".json").to_string()
}

fn node_safe_name(value: Option<&str>) -> String {
    value
        .unwrap_or("schema-validate")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn validate_json_schema(value: &Value, schema: &Value, path: &str, errors: &mut Vec<String>) {
    if errors.len() >= MAX_ERRORS {
        return;
    }
    let Some(schema_obj) = schema.as_object() else {
        errors.push(format!("{path}: schema must be an object"));
        return;
    };

    if let Some(any_of) = schema_obj.get("anyOf").and_then(Value::as_array) {
        if any_of.is_empty() {
            errors.push(format!("{path}: anyOf must contain at least 1 schema"));
            return;
        }

        let mut first_errors = Vec::new();
        for candidate in any_of {
            let mut candidate_errors = Vec::new();
            validate_json_schema(value, candidate, path, &mut candidate_errors);
            if candidate_errors.is_empty() {
                return;
            }
            if first_errors.is_empty() {
                first_errors = candidate_errors;
            }
        }

        let reason = first_errors
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .join("; ");
        errors.push(if reason.is_empty() {
            format!("{path}: did not match anyOf")
        } else {
            format!("{path}: did not match anyOf ({reason})")
        });
        return;
    }

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

const fn value_type(value: &Value) -> &'static str {
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

    #[test]
    fn supports_any_of_schema_variants() {
        let mut errors = Vec::new();
        validate_json_schema(
            &json!({
                "title": "NodeOutcome",
                "summary": "Execution output",
                "source_anchors": ["src/lib.rs:1"]
            }),
            &json!({
                "anyOf": [
                    {
                        "type": "object",
                        "required": ["findings"],
                        "properties": {
                            "findings": {"type": "array", "minItems": 1}
                        }
                    },
                    {
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
                ]
            }),
            "$",
            &mut errors,
        );

        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn source_by_ids_collects_exact_node_outputs_and_marks_missing() {
        let config = SchemaValidateConfig {
            inputs: None,
            schema: json!({}),
            value_key: default_value_key(),
            value_keys: vec![],
            source: Some(SchemaSourceConfig {
                kind: Some("node_outputs_by_ids".to_string()),
                prefix: None,
                ids: vec!["scout-a".to_string(), "missing-scout".to_string()],
                exclude: vec![],
            }),
            fail_on_invalid: false,
            artifact_path: None,
            artifact_scope: None,
            artifact_name: None,
            schema_name: None,
            repair: None,
        };
        let mut node_outputs = Map::new();
        node_outputs.insert("scout-a".to_string(), json!({"findings": []}));
        node_outputs.insert("scout-b".to_string(), json!({"findings": []}));
        let run = TaskGraphRun {
            id: "run-test".to_string(),
            project: "blackboard".to_string(),
            graph_ref: crate::task_graph::run_state::GraphRef {
                scope: crate::task_graph::definition::types::TaskGraphScope::Project,
                id: "graph-test".to_string(),
                version: 1,
            },
            status: crate::task_graph::run_state::RunStatus::Running,
            created_at: "2026-05-20T00:00:00Z".to_string(),
            queued_at: None,
            queue_deadline_at: None,
            started_at: None,
            updated_at: "2026-05-20T00:00:00Z".to_string(),
            completed_at: None,
            current_superstep: 0,
            last_checkpoint_id: None,
            pregel_checkpoint: None,
            current_graph_revision: 0,
            active_nodes: vec![],
            paused: None,
            context: crate::task_graph::run_state::RunContext {
                input: json!({}),
                node_outputs,
                branch_decisions: vec![],
                loop_iterations: vec![],
                loop_stack: vec![],
                completed_branches: Default::default(),
            },
            parent_run_id: None,
            checkpoint_ns: None,
        };

        let value = resolve_source_value(&config, &Map::new(), &run);

        assert_eq!(value["scout-a"], json!({"findings": []}));
        assert!(value["missing-scout"].is_null());
        assert!(value.get("scout-b").is_none());
    }

    #[test]
    fn repairs_research_scout_outputs_for_metadata_only_and_string_values() {
        let config = SchemaValidateConfig {
            inputs: None,
            schema: json!({}),
            value_key: default_value_key(),
            value_keys: vec![],
            source: None,
            fail_on_invalid: false,
            artifact_path: None,
            artifact_scope: None,
            artifact_name: None,
            schema_name: Some("code_research_scout_outputs".to_string()),
            repair: Some(SchemaRepairConfig {
                kind: Some("code_research_scout_outputs".to_string()),
            }),
        };

        let (value, notes) = repair_source_value(
            &config,
            json!({
                "metadata-only": {
                    "coverage_notes": ["looked at validation"],
                    "risks": ["no findings"]
                },
                "string-output": "plain scout summary",
                "normal": {
                    "findings": [{
                        "title": "Runtime",
                        "summary": "Pregel runtime exists",
                        "source_anchors": []
                    }]
                }
            }),
        );

        assert!(!notes.is_empty());
        assert_eq!(value["metadata-only"]["findings"], json!([]));
        assert_eq!(
            value["string-output"]["findings"][0]["summary"],
            json!("plain scout summary")
        );
        assert_eq!(value["normal"]["findings"][0]["source_anchors"], json!([]));

        let external_config = SchemaValidateConfig {
            schema_name: Some("external_research_scout_outputs".to_string()),
            repair: Some(SchemaRepairConfig {
                kind: Some("external_research_scout_outputs".to_string()),
            }),
            ..config
        };

        let (external_value, external_notes) = repair_source_value(
            &external_config,
            json!({
                "metadata-only": {
                    "coverage_notes": ["external source was inaccessible"],
                    "risks": ["source unavailable"]
                }
            }),
        );

        assert!(!external_notes.is_empty());
        assert_eq!(external_value["metadata-only"]["findings"], json!([]));
    }
}
