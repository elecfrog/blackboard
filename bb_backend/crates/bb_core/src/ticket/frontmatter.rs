use std::collections::BTreeMap;

use crate::{FrontmatterExtra, InboxError, TicketAttachment};

use super::{validate_required_string, CORE_FRONTMATTER_FIELDS};

#[derive(Debug, Default)]
pub struct ParsedFrontmatter {
    pub fields: BTreeMap<String, String>,
    pub error: Option<String>,
}

pub fn parse_ticket_frontmatter(content: &str) -> ParsedFrontmatter {
    let mut lines = content.lines();
    let Some(first) = lines.next() else {
        return ParsedFrontmatter {
            error: Some("missing leading +++ frontmatter".to_string()),
            ..ParsedFrontmatter::default()
        };
    };

    if first.trim() != "+++" {
        return ParsedFrontmatter {
            error: Some("missing leading +++ frontmatter".to_string()),
            ..ParsedFrontmatter::default()
        };
    }

    let mut fields = BTreeMap::new();
    let mut error = None;
    let mut closed = false;

    for (index, line) in lines.enumerate() {
        let trimmed = line.trim();
        if trimmed == "+++" {
            closed = true;
            break;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match parse_frontmatter_assignment(trimmed) {
            Ok((key, value)) => {
                fields.insert(key, value);
            }
            Err(message) if error.is_none() => {
                error = Some(format!("frontmatter line {}: {message}", index + 2));
            }
            Err(_) => {}
        }
    }

    if !closed {
        fields.clear();
        if error.is_none() {
            error = Some("unterminated +++ frontmatter".to_string());
        }
    }

    ParsedFrontmatter { fields, error }
}

pub fn parse_frontmatter_assignment(line: &str) -> Result<(String, String), String> {
    let (key, value) = line
        .split_once('=')
        .ok_or_else(|| "expected `key = \"value\"`".to_string())?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err("invalid key".to_string());
    }

    let value = value.trim();
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return Err("expected quoted string value".to_string());
    }

    Ok((
        key.to_string(),
        unescape_quoted_value(&value[1..value.len() - 1]),
    ))
}

pub fn unescape_quoted_value(value: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            result.push(match ch {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            result.push(ch);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

pub fn escape_quoted_value(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

pub fn split_ticket_frontmatter(
    content: &str,
) -> Result<(BTreeMap<String, String>, &str), InboxError> {
    let mut offset = 0;
    let mut lines = content.split_inclusive('\n');
    let first = lines
        .next()
        .ok_or_else(|| InboxError::InvalidInput("ticket is empty".to_string()))?;
    if first.trim() != "+++" {
        return Err(InboxError::InvalidInput(
            "ticket is missing leading +++ frontmatter".to_string(),
        ));
    }
    offset += first.len();
    let frontmatter_start = offset;

    for line in lines {
        if line.trim() == "+++" {
            let block = &content[frontmatter_start..offset];
            let mut fields = BTreeMap::new();
            for (index, line) in block.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let (key, value) = parse_frontmatter_assignment(trimmed).map_err(|message| {
                    InboxError::InvalidInput(format!("frontmatter line {}: {message}", index + 2))
                })?;
                fields.insert(key, value);
            }
            let body_start = offset + line.len();
            return Ok((fields, &content[body_start..]));
        }
        offset += line.len();
    }

    Err(InboxError::InvalidInput(
        "ticket has unterminated +++ frontmatter".to_string(),
    ))
}

pub fn render_frontmatter(fields: &BTreeMap<String, String>) -> String {
    const CORE_ORDER: [&str; 7] = [
        "id",
        "lane",
        "title",
        "created_at",
        "updated_at",
        "status",
        "ticket_spec",
    ];
    let mut rendered = String::from("+++\n");
    for key in CORE_ORDER {
        if let Some(value) = fields.get(key) {
            rendered.push_str(key);
            rendered.push_str(" = \"");
            rendered.push_str(&escape_quoted_value(value));
            rendered.push_str("\"\n");
        }
    }
    for (key, value) in fields {
        if CORE_ORDER.contains(&key.as_str()) {
            continue;
        }
        rendered.push_str(key);
        rendered.push_str(" = \"");
        rendered.push_str(&escape_quoted_value(value));
        rendered.push_str("\"\n");
    }
    rendered.push_str("+++\n");
    rendered
}

pub(super) fn reject_core_frontmatter_key(key: &str) -> Result<(), InboxError> {
    if CORE_FRONTMATTER_FIELDS.contains(&key) {
        return Err(InboxError::InvalidInput(format!(
            "`{key}` is a core frontmatter field and cannot be set via `extra` or `remove`"
        )));
    }
    Ok(())
}

pub(super) fn reject_extra_frontmatter_key(key: &str) -> Result<(), InboxError> {
    reject_core_frontmatter_key(key)?;
    if key == "owner" {
        return Err(InboxError::InvalidInput(
            "`owner` is a legacy frontmatter field; use `assignee` or another explicit extra key"
                .to_string(),
        ));
    }
    if key == "attachments" {
        return Err(InboxError::InvalidInput(
            "`attachments` is a top-level ticket field; use the structured attachments field"
                .to_string(),
        ));
    }
    Ok(())
}

pub(super) fn sanitize_extra_for_write(
    extra: &FrontmatterExtra,
) -> Result<FrontmatterExtra, InboxError> {
    let mut out = FrontmatterExtra::new();
    for (key, value) in extra {
        reject_extra_frontmatter_key(key)?;
        validate_required_string("extra key", key)?;
        out.insert(key.clone(), value.clone());
    }
    Ok(out)
}

pub fn extract_extra_fields(fields: &BTreeMap<String, String>) -> FrontmatterExtra {
    let mut out = FrontmatterExtra::new();
    for (key, value) in fields {
        if key == "attachments" {
            continue;
        }
        if !CORE_FRONTMATTER_FIELDS.contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
    out
}

pub fn extract_attachments_field(fields: &BTreeMap<String, String>) -> Vec<TicketAttachment> {
    fields
        .get("attachments")
        .and_then(|raw| serde_json::from_str::<Vec<TicketAttachment>>(raw).ok())
        .unwrap_or_default()
}
