//! Code-owned JSON Schema generation.

use serde_json::{json, Map, Value};

use crate::ticket;

pub struct GeneratedSchema {
    pub file_name: &'static str,
    pub value: Value,
}

pub fn generated_schemas() -> Vec<GeneratedSchema> {
    vec![GeneratedSchema {
        file_name: "ticket.schema.json",
        value: ticket_schema(),
    }]
}

pub fn ticket_schema() -> Value {
    let mut schema = ticket::ticket_json_document_schema();
    apply_ticket_contract(&mut schema);
    schema
}

fn apply_ticket_contract(schema: &mut Value) {
    let root = object_mut(schema);
    root.insert(
        "$schema".to_string(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert(
        "$id".to_string(),
        json!("https://blackboard.local/schemas/ticket.schema.json"),
    );
    root.insert("title".to_string(), json!("Blackboard JSON Ticket"));
    root.insert("additionalProperties".to_string(), json!(false));

    set_required(
        schema,
        &[
            "schema_version",
            "id",
            "lane",
            "title",
            "summary",
            "stories",
            "risks",
            "progress_record",
            "attachments",
            "status",
            "created_at",
            "updated_at",
        ],
    );

    property_mut(schema, "schema_version").insert("const".to_string(), json!(1));
    property_mut(schema, "id").insert("pattern".to_string(), json!("^[0-9]{6}$"));
    property_mut(schema, "lane").insert("pattern".to_string(), json!("^[a-z][a-z0-9-]{1,31}$"));
    set_min_length(property_mut(schema, "title"));
    set_min_length(property_mut(schema, "summary"));
    property_mut(schema, "summary").insert(
        "description".to_string(),
        json!("当前有效行为定义的一句话摘要，不记录过程。"),
    );
    property_mut(schema, "stories").insert("minItems".to_string(), json!(1));
    property_mut(schema, "status").insert("enum".to_string(), json!(ticket::TICKET_STATUSES));
    property_mut(schema, "created_at")
        .insert("pattern".to_string(), json!("^\\d{4}-\\d{2}-\\d{2}$"));
    property_mut(schema, "updated_at")
        .insert("pattern".to_string(), json!("^\\d{4}-\\d{2}-\\d{2}$"));

    let extra = property_mut(schema, "extra");
    extra.insert(
        "propertyNames".to_string(),
        json!({
            "not": {
                "const": "attachments"
            }
        }),
    );
    extra.insert(
        "additionalProperties".to_string(),
        json!({
            "type": "string"
        }),
    );

    apply_story_contract(schema);
    apply_risk_contract(schema);
    apply_progress_record_contract(schema);
    apply_attachment_contract(schema);
}

fn apply_story_contract(schema: &mut Value) {
    let story = definition_mut(schema, "TicketStory");
    story.insert("additionalProperties".to_string(), json!(false));
    set_required_on(story, &["id", "given", "when", "then"]);
    property_mut_in(story, "id").remove("default");
    property_mut_in(story, "id").insert("format".to_string(), json!("uuid"));
    property_mut_in(story, "id").insert(
        "description".to_string(),
        json!("系统生成的 story id；LLM 不应手写语义 id。"),
    );
    set_text_property(story, "given", "前置条件。");
    set_text_property(story, "when", "用户或系统触发的行为。");
    set_text_property(story, "then", "可观察、可验收的期望结果。");
    set_optional_text_property(
        story,
        "sample",
        "可选示例，指导 LLM/用户如何填写本 story；不是验收事实。",
    );
}

fn apply_risk_contract(schema: &mut Value) {
    let risk = definition_mut(schema, "TicketRisk");
    risk.insert("additionalProperties".to_string(), json!(false));
    set_required_on(risk, &["id", "description"]);
    property_mut_in(risk, "id").insert("pattern".to_string(), json!("^[a-z0-9][a-z0-9_-]{0,79}$"));
    set_min_length(property_mut_in(risk, "description"));
    set_optional_min_length(property_mut_in(risk, "mitigation"));
    set_optional_min_length(property_mut_in(risk, "status"));
}

fn apply_progress_record_contract(schema: &mut Value) {
    let record = definition_mut(schema, "TicketProgressRecord");
    record.insert("additionalProperties".to_string(), json!(false));
    set_required_on(record, &["summary"]);
    set_optional_min_length(property_mut_in(record, "at"));
    set_min_length(property_mut_in(record, "summary"));
    if let Some(items) = property_mut_in(record, "evidence")
        .get_mut("items")
        .and_then(Value::as_object_mut)
    {
        items.insert("minLength".to_string(), json!(1));
    }
}

fn apply_attachment_contract(schema: &mut Value) {
    let attachment = definition_mut(schema, "TicketAttachment");
    attachment.insert("additionalProperties".to_string(), json!(false));
    set_required_on(attachment, &["kind", "target"]);
    set_min_length(property_mut_in(attachment, "kind"));
    set_min_length(property_mut_in(attachment, "target"));
    set_optional_min_length(property_mut_in(attachment, "label"));
    set_optional_min_length(property_mut_in(attachment, "description"));
}

fn object_mut(value: &mut Value) -> &mut Map<String, Value> {
    value
        .as_object_mut()
        .expect("schema root must be an object")
}

fn set_required(schema: &mut Value, fields: &[&str]) {
    set_required_on(object_mut(schema), fields);
}

fn set_required_on(schema: &mut Map<String, Value>, fields: &[&str]) {
    schema.insert(
        "required".to_string(),
        Value::Array(fields.iter().map(|field| json!(field)).collect()),
    );
}

fn property_mut<'a>(schema: &'a mut Value, name: &str) -> &'a mut Map<String, Value> {
    property_mut_in(object_mut(schema), name)
}

fn property_mut_in<'a>(
    schema: &'a mut Map<String, Value>,
    name: &str,
) -> &'a mut Map<String, Value> {
    schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(name))
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("schema property `{name}` must exist"))
}

fn definition_mut<'a>(schema: &'a mut Value, name: &str) -> &'a mut Map<String, Value> {
    schema
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .and_then(|defs| defs.get_mut(name))
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("schema definition `{name}` must exist"))
}

fn set_text_property(schema: &mut Map<String, Value>, name: &str, description: &str) {
    let property = property_mut_in(schema, name);
    set_min_length(property);
    property.insert("description".to_string(), json!(description));
}

fn set_optional_text_property(schema: &mut Map<String, Value>, name: &str, description: &str) {
    let property = property_mut_in(schema, name);
    set_optional_min_length(property);
    property.insert("description".to_string(), json!(description));
}

fn set_min_length(property: &mut Map<String, Value>) {
    property.insert("minLength".to_string(), json!(1));
}

fn set_optional_min_length(property: &mut Map<String, Value>) {
    if let Some(array) = property.get_mut("anyOf").and_then(Value::as_array_mut) {
        for item in array {
            if item.get("type").and_then(Value::as_str) == Some("string") {
                if let Some(item) = item.as_object_mut() {
                    item.insert("minLength".to_string(), json!(1));
                }
            }
        }
    } else {
        property.insert("minLength".to_string(), json!(1));
    }
}
