use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    FrontmatterExtra, InboxError, TicketAttachment, TicketProgressRecord, TicketRisk, TicketSpec,
    TicketStory,
};

pub const TICKET_SPEC_FRONTMATTER_KEY: &str = "ticket_spec";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketJsonDocument {
    pub schema_version: u32,
    pub id: String,
    pub lane: String,
    pub title: String,
    pub summary: String,
    pub stories: Vec<TicketStory>,
    pub risks: Vec<TicketRisk>,
    pub progress_record: Vec<TicketProgressRecord>,
    pub attachments: Vec<TicketAttachment>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub extra: FrontmatterExtra,
}

#[cfg(feature = "schema")]
pub fn ticket_json_document_schema() -> serde_json::Value {
    schemars::schema_for!(TicketJsonDocument).to_value()
}

impl TicketJsonDocument {
    pub(crate) fn spec(&self) -> TicketSpec {
        TicketSpec {
            summary: self.summary.clone(),
            stories: self.stories.clone(),
            risks: self.risks.clone(),
            progress_record: self.progress_record.clone(),
        }
    }

    pub(crate) fn set_spec(&mut self, spec: TicketSpec) {
        self.summary = spec.summary;
        self.stories = spec.stories;
        self.risks = spec.risks;
        self.progress_record = spec.progress_record;
    }
}

pub fn normalize_ticket_spec(mut spec: TicketSpec) -> Result<TicketSpec, InboxError> {
    validate_text("spec.summary", &spec.summary)?;
    if spec.stories.is_empty() {
        return Err(InboxError::InvalidInput(
            "spec.stories requires at least one Given/When/Then story".to_string(),
        ));
    }
    for story in &mut spec.stories {
        if story.id.trim().is_empty() {
            story.id = uuid::Uuid::new_v4().to_string();
        }
    }
    validate_ticket_spec(&spec)?;
    Ok(spec)
}

#[allow(clippy::too_many_arguments)]
pub fn render_ticket_json_document(
    id: &str,
    lane: &str,
    title: &str,
    created_at: &str,
    updated_at: &str,
    status: &str,
    spec: TicketSpec,
    attachments: Vec<TicketAttachment>,
    extra: FrontmatterExtra,
) -> Result<String, InboxError> {
    let spec = normalize_ticket_spec(spec)?;
    validate_ticket_attachments(&attachments)?;
    let document = TicketJsonDocument {
        schema_version: 1,
        id: id.to_string(),
        lane: lane.to_string(),
        title: title.to_string(),
        summary: spec.summary,
        stories: spec.stories,
        risks: spec.risks,
        progress_record: spec.progress_record,
        attachments,
        status: status.to_string(),
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
        extra,
    };
    serialize_ticket_json_document(&document)
}

pub fn parse_ticket_json_document(content: &str) -> Result<TicketJsonDocument, String> {
    let document = serde_json::from_str::<TicketJsonDocument>(content)
        .map_err(|err| format!("invalid JSON ticket: {err}"))?;
    if document.schema_version != 1 {
        return Err(format!(
            "unsupported ticket schema_version `{}`",
            document.schema_version
        ));
    }
    validate_ticket_spec(&document.spec()).map_err(|err| err.to_string())?;
    validate_ticket_attachments(&document.attachments).map_err(|err| err.to_string())?;
    Ok(document)
}

pub fn serialize_ticket_json_document(document: &TicketJsonDocument) -> Result<String, InboxError> {
    validate_ticket_spec(&document.spec())?;
    validate_ticket_attachments(&document.attachments)?;
    let mut json = serde_json::to_string_pretty(document).map_err(|err| {
        InboxError::InvalidInput(format!("ticket JSON could not be serialized: {err}"))
    })?;
    json.push('\n');
    Ok(json)
}

pub fn serialize_ticket_spec(spec: &TicketSpec) -> Result<String, InboxError> {
    validate_ticket_spec(spec)?;
    serde_json::to_string(spec).map_err(|err| {
        InboxError::InvalidInput(format!("ticket spec could not be serialized: {err}"))
    })
}

pub fn parse_ticket_spec_field(fields: &BTreeMap<String, String>) -> Result<TicketSpec, String> {
    let raw = fields
        .get(TICKET_SPEC_FRONTMATTER_KEY)
        .ok_or_else(|| format!("missing frontmatter field `{TICKET_SPEC_FRONTMATTER_KEY}`"))?;
    let spec = serde_json::from_str::<TicketSpec>(raw)
        .map_err(|err| format!("invalid `{TICKET_SPEC_FRONTMATTER_KEY}` JSON: {err}"))?;
    validate_ticket_spec(&spec).map_err(|err| err.to_string())?;
    Ok(spec)
}

pub fn validate_ticket_spec(spec: &TicketSpec) -> Result<(), InboxError> {
    validate_text("spec.summary", &spec.summary)?;
    if spec.stories.is_empty() {
        return Err(InboxError::InvalidInput(
            "spec.stories requires at least one Given/When/Then story".to_string(),
        ));
    }
    for story in &spec.stories {
        validate_story(story)?;
    }
    for risk in &spec.risks {
        validate_risk(risk)?;
    }
    for record in &spec.progress_record {
        validate_progress_record(record)?;
    }
    Ok(())
}

fn validate_story(story: &TicketStory) -> Result<(), InboxError> {
    validate_story_id("spec.stories[].id", &story.id)?;
    validate_text("spec.stories[].given", &story.given)?;
    validate_text("spec.stories[].when", &story.when)?;
    validate_text("spec.stories[].then", &story.then)?;
    validate_optional_text("spec.stories[].sample", story.sample.as_deref())?;
    Ok(())
}

fn validate_risk(risk: &TicketRisk) -> Result<(), InboxError> {
    validate_risk_id("spec.risks[].id", &risk.id)?;
    validate_text("spec.risks[].description", &risk.description)?;
    validate_optional_text("spec.risks[].mitigation", risk.mitigation.as_deref())?;
    validate_optional_text("spec.risks[].status", risk.status.as_deref())?;
    Ok(())
}

fn validate_progress_record(record: &TicketProgressRecord) -> Result<(), InboxError> {
    validate_optional_text("spec.progress_record[].at", record.at.as_deref())?;
    validate_text("spec.progress_record[].summary", &record.summary)?;
    for item in &record.evidence {
        validate_text("spec.progress_record[].evidence[]", item)?;
    }
    Ok(())
}

pub fn validate_ticket_attachments(attachments: &[TicketAttachment]) -> Result<(), InboxError> {
    for attachment in attachments {
        validate_text("attachments[].kind", &attachment.kind)?;
        validate_text("attachments[].target", &attachment.target)?;
        validate_optional_text("attachments[].label", attachment.label.as_deref())?;
        validate_optional_text(
            "attachments[].description",
            attachment.description.as_deref(),
        )?;
    }
    Ok(())
}

fn validate_story_id(field: &str, value: &str) -> Result<(), InboxError> {
    validate_text(field, value)?;
    if !is_uuid_like(value) {
        return Err(InboxError::InvalidInput(format!(
            "{field} must be a system-generated UUID"
        )));
    }
    Ok(())
}

fn validate_risk_id(field: &str, value: &str) -> Result<(), InboxError> {
    validate_text(field, value)?;
    if value.len() > 80 || !is_semantic_token(value) {
        return Err(InboxError::InvalidInput(format!(
            "{field} must match [a-z0-9][a-z0-9_-]{{0,79}}"
        )));
    }
    Ok(())
}

fn is_uuid_like(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lengths = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(lengths)
        .all(|(part, len)| part.len() == len && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn is_semantic_token(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn validate_text(field: &str, value: &str) -> Result<(), InboxError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(InboxError::InvalidInput(format!("{field} is required")));
    }
    if trimmed != value {
        return Err(InboxError::InvalidInput(format!(
            "{field} must not have surrounding whitespace"
        )));
    }
    if value.len() > 4000 || value.chars().any(|ch| ch == '\0') {
        return Err(InboxError::InvalidInput(format!("{field} is invalid")));
    }
    Ok(())
}

fn validate_optional_text(field: &str, value: Option<&str>) -> Result<(), InboxError> {
    if let Some(value) = value {
        validate_text(field, value)?;
    }
    Ok(())
}

pub fn render_ticket_spec_body(spec: &TicketSpec) -> String {
    let mut body = String::new();
    body.push_str("# Summary\n\n");
    body.push_str(&spec.summary);
    body.push_str("\n\n# Stories\n\n");

    for story in &spec.stories {
        body.push_str("## ");
        body.push_str(&story.id);
        body.push_str("\n\n");
        body.push_str("- Given: ");
        body.push_str(&story.given);
        body.push('\n');
        body.push_str("- When: ");
        body.push_str(&story.when);
        body.push('\n');
        body.push_str("- Then: ");
        body.push_str(&story.then);
        body.push_str("\n\n");
        if let Some(sample) = story.sample.as_deref() {
            body.push_str("Sample: ");
            body.push_str(sample);
            body.push_str("\n\n");
        }
    }

    body.push_str("# Risks\n\n");
    if spec.risks.is_empty() {
        body.push_str("- None\n\n");
    } else {
        for risk in &spec.risks {
            body.push_str("- ");
            body.push_str(&risk.id);
            body.push_str(": ");
            body.push_str(&risk.description);
            body.push('\n');
            if let Some(mitigation) = risk.mitigation.as_deref() {
                body.push_str("  - Mitigation: ");
                body.push_str(mitigation);
                body.push('\n');
            }
            if let Some(status) = risk.status.as_deref() {
                body.push_str("  - Status: ");
                body.push_str(status);
                body.push('\n');
            }
        }
        body.push('\n');
    }

    body.push_str("# Progress Record\n\n");
    if spec.progress_record.is_empty() {
        body.push_str("- No progress recorded yet.\n");
    } else {
        for record in &spec.progress_record {
            body.push_str("- ");
            if let Some(at) = record.at.as_deref() {
                body.push_str(at);
                body.push_str(": ");
            }
            body.push_str(&record.summary);
            body.push('\n');
            for evidence in &record.evidence {
                body.push_str("  - Evidence: ");
                body.push_str(evidence);
                body.push('\n');
            }
        }
    }
    body
}
