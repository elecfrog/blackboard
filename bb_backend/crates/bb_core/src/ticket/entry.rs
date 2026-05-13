use crate::TicketEntry;

use super::frontmatter::{extract_extra_fields, parse_ticket_frontmatter};
use super::{validate_ticket_status, REQUIRED_METADATA_FIELDS};

pub(crate) fn ticket_entry_from_content(name: String, content: &str) -> TicketEntry {
    let path = format!("tickets/{name}");
    let filename_id = ticket_id_from_name(&name);
    let frontmatter = parse_ticket_frontmatter(content);
    let mut metadata_error = frontmatter.error;
    let frontmatter_has_error = metadata_error.is_some();
    let mut metadata_warnings = Vec::new();

    for field in REQUIRED_METADATA_FIELDS {
        if !frontmatter.fields.contains_key(field) {
            if field == "lane" && frontmatter.fields.contains_key("family") {
                continue;
            }
            metadata_warnings.push(format!("missing frontmatter field `{field}`"));
        }
    }

    let frontmatter_id = frontmatter.fields.get("id").cloned();
    let id = match frontmatter_id.as_deref() {
        Some(value) if !frontmatter_has_error && is_six_digit_id(value) => {
            if let Some(filename_id) = filename_id.as_deref() {
                if filename_id != value {
                    metadata_warnings.push(format!(
                        "frontmatter id `{value}` does not match filename id `{filename_id}`"
                    ));
                }
            }
            Some(value.to_string())
        }
        Some(value) if !frontmatter_has_error => {
            let message = format!("invalid frontmatter id `{value}`");
            if metadata_error.is_none() {
                metadata_error = Some(message.clone());
            } else {
                metadata_warnings.push(message);
            }
            filename_id
        }
        Some(value) => {
            metadata_warnings.push(format!(
                "frontmatter id `{value}` ignored because frontmatter has parse error"
            ));
            filename_id
        }
        None => filename_id,
    };

    let status_str = frontmatter.fields.get("status").cloned();
    if let Some(status) = status_str.as_deref() {
        if !frontmatter_has_error && validate_ticket_status(status).is_err() {
            let message = format!("invalid frontmatter status `{status}`");
            if metadata_error.is_none() {
                metadata_error = Some(message);
            } else {
                metadata_warnings.push(message);
            }
        }
    }

    TicketEntry {
        name,
        path,
        id,
        lane: frontmatter
            .fields
            .get("lane")
            .cloned()
            .or_else(|| frontmatter.fields.get("family").cloned()),
        title: frontmatter.fields.get("title").cloned(),
        status: status_str,
        created_at: frontmatter.fields.get("created_at").cloned(),
        updated_at: frontmatter.fields.get("updated_at").cloned(),
        extra: extract_extra_fields(&frontmatter.fields),
        metadata_error,
        metadata_warnings,
    }
}

fn ticket_id_from_name(name: &str) -> Option<String> {
    name.trim_end_matches(".md")
        .split('-')
        .find(|segment| is_six_digit_id(segment))
        .map(ToString::to_string)
}

pub(crate) fn is_six_digit_id(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|ch| ch.is_ascii_digit())
}
