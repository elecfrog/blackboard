use chrono::{SecondsFormat, Utc};

use crate::{InboxError, InboxJsonDocument, InboxNoteInput};

pub const INBOX_SCHEMA_VERSION: u32 = 1;

pub fn document_from_input(input: InboxNoteInput) -> Result<InboxJsonDocument, InboxError> {
    let document = InboxJsonDocument {
        schema_version: INBOX_SCHEMA_VERSION,
        title: input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Inbox Note")
            .to_string(),
        time: input
            .time
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
        source: input.source,
        project: input.project.unwrap_or_default(),
        topic: input.topic,
        done: trim_lines(input.done),
        validation: trim_lines(input.validation),
        next_step: trim_lines(input.next_step),
        related_locations: trim_lines(input.related_locations),
        related_tickets: trim_lines(input.related_tickets),
        attachments: input.attachments,
        extra: input.extra,
    };
    validate_inbox_document(&document)?;
    Ok(document)
}

pub fn parse_inbox_json_document(content: &str) -> Result<InboxJsonDocument, String> {
    let document = serde_json::from_str::<InboxJsonDocument>(content)
        .map_err(|err| format!("invalid JSON inbox note: {err}"))?;
    if document.schema_version != INBOX_SCHEMA_VERSION {
        return Err(format!(
            "unsupported inbox schema_version `{}`",
            document.schema_version
        ));
    }
    validate_inbox_document(&document).map_err(|err| err.to_string())?;
    Ok(document)
}

pub fn serialize_inbox_json_document(document: &InboxJsonDocument) -> Result<String, InboxError> {
    validate_inbox_document(document)?;
    let mut json = serde_json::to_string_pretty(document).map_err(|err| {
        InboxError::InvalidInput(format!("inbox JSON could not be serialized: {err}"))
    })?;
    json.push('\n');
    Ok(json)
}

pub fn validate_inbox_document(document: &InboxJsonDocument) -> Result<(), InboxError> {
    if document.schema_version != INBOX_SCHEMA_VERSION {
        return Err(InboxError::InvalidInput(format!(
            "unsupported inbox schema_version `{}`",
            document.schema_version
        )));
    }
    validate_text("title", &document.title)?;
    validate_text("time", &document.time)?;
    validate_text("source", &document.source)?;
    validate_text("project", &document.project)?;
    validate_text("topic", &document.topic)?;
    validate_lines("done", &document.done)?;
    validate_lines("validation", &document.validation)?;
    validate_lines("next_step", &document.next_step)?;
    validate_lines("related_locations", &document.related_locations)?;
    validate_lines("related_tickets", &document.related_tickets)?;
    crate::ticket::validate_ticket_attachments(&document.attachments)?;
    for (key, value) in &document.extra {
        validate_text("extra key", key)?;
        validate_text("extra value", value)?;
    }
    Ok(())
}

#[must_use]
pub fn render_note(document: &InboxJsonDocument) -> String {
    format!(
        "# {title}\n\n时间: {time}\n来源: {source}\n项目: {project}\n主题: {topic}\n\n## 做了什么\n\n{done}\n\n## 验证了什么\n\n{validation}\n\n## 下一步\n\n{next_step}\n\n## 相关位置\n\n{related_locations}\n\n## 相关 ticket\n\n{related_tickets}\n",
        title = document.title.trim(),
        time = document.time.trim(),
        source = document.source.trim(),
        project = document.project.trim(),
        topic = document.topic.trim(),
        done = render_lines(&document.done),
        validation = render_lines(&document.validation),
        next_step = render_lines(&document.next_step),
        related_locations = render_lines(&document.related_locations),
        related_tickets = render_lines(&document.related_tickets),
    )
}

#[must_use]
pub fn inbox_excerpt(document: &InboxJsonDocument) -> String {
    for line in document
        .done
        .iter()
        .chain(document.validation.iter())
        .chain(document.next_step.iter())
        .chain(document.related_locations.iter())
        .chain(document.related_tickets.iter())
    {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return truncate_excerpt(trimmed);
        }
    }
    truncate_excerpt(document.title.trim())
}

#[cfg(feature = "schema")]
pub fn inbox_json_document_schema() -> serde_json::Value {
    schemars::schema_for!(InboxJsonDocument).to_value()
}

fn trim_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn validate_lines(field: &str, lines: &[String]) -> Result<(), InboxError> {
    for line in lines {
        validate_text(&format!("{field}[]"), line)?;
    }
    Ok(())
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

fn truncate_excerpt(value: &str) -> String {
    if value.chars().count() <= 140 {
        return value.to_string();
    }
    let excerpt: String = value.chars().take(140).collect();
    format!("{excerpt}...")
}

fn render_lines(lines: &[String]) -> String {
    let rendered: Vec<String> = lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| format!("- {line}"))
        .collect();

    if rendered.is_empty() {
        "- （未填写）".to_string()
    } else {
        rendered.join("\n")
    }
}
