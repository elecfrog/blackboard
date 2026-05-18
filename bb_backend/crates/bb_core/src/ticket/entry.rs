use crate::TicketEntry;

use super::frontmatter::{
    extract_attachments_field, extract_extra_fields, parse_ticket_frontmatter,
};
use super::spec::{
    parse_ticket_json_document, parse_ticket_spec_field, TICKET_SPEC_FRONTMATTER_KEY,
};
use super::{validate_ticket_status, REQUIRED_METADATA_FIELDS};

pub fn ticket_entry_from_content(name: String, content: &str) -> TicketEntry {
    if name.ends_with(".json") {
        return json_ticket_entry_from_content(name, content);
    }

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
                metadata_error = Some(message);
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
    let spec =
        if !frontmatter_has_error && frontmatter.fields.contains_key(TICKET_SPEC_FRONTMATTER_KEY) {
            match parse_ticket_spec_field(&frontmatter.fields) {
                Ok(spec) => Some(spec),
                Err(message) => {
                    metadata_warnings.push(message);
                    None
                }
            }
        } else {
            None
        };

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
        attachments: extract_attachments_field(&frontmatter.fields),
        spec,
        extra: extract_extra_fields(&frontmatter.fields),
        metadata_error,
        metadata_warnings,
    }
}

fn json_ticket_entry_from_content(name: String, content: &str) -> TicketEntry {
    let path = format!("tickets/{name}");
    let filename_id = ticket_id_from_name(&name);
    match parse_ticket_json_document(content) {
        Ok(document) => {
            let mut metadata_warnings = Vec::new();
            if let Some(filename_id) = filename_id.as_deref() {
                if filename_id != document.id {
                    metadata_warnings.push(format!(
                        "json id `{}` does not match filename id `{filename_id}`",
                        document.id
                    ));
                }
            }
            if validate_ticket_status(&document.status).is_err() {
                metadata_warnings.push(format!("invalid ticket status `{}`", document.status));
            }
            TicketEntry {
                name,
                path,
                id: Some(document.id.clone()),
                lane: Some(document.lane.clone()),
                title: Some(document.title.clone()),
                status: Some(document.status.clone()),
                created_at: Some(document.created_at.clone()),
                updated_at: Some(document.updated_at.clone()),
                attachments: document.attachments.clone(),
                spec: Some(document.spec()),
                extra: document.extra,
                metadata_error: None,
                metadata_warnings,
            }
        }
        Err(message) => TicketEntry {
            name,
            path,
            id: filename_id,
            lane: None,
            title: None,
            status: None,
            created_at: None,
            updated_at: None,
            attachments: Vec::new(),
            spec: None,
            extra: Default::default(),
            metadata_error: Some(message),
            metadata_warnings: Vec::new(),
        },
    }
}

fn ticket_id_from_name(name: &str) -> Option<String> {
    name.trim_end_matches(".md")
        .trim_end_matches(".json")
        .split('-')
        .find(|segment| is_six_digit_id(segment))
        .map(ToString::to_string)
}

pub fn is_six_digit_id(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|ch| ch.is_ascii_digit())
}
