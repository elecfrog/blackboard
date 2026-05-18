use crate::{FrontmatterExtra, TicketAttachment, TicketBodySections};

use super::spec::render_ticket_json_document;

pub(super) struct TicketRenderInput<'a> {
    pub id: &'a str,
    pub lane: &'a str,
    pub title: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub status: &'a str,
    pub spec_json: &'a str,
    pub attachments: &'a [TicketAttachment],
    pub extra: &'a FrontmatterExtra,
    pub sections: &'a TicketBodySections,
}

pub(super) fn render_ticket(input: TicketRenderInput<'_>) -> String {
    let spec = serde_json::from_str(input.spec_json).expect("validated ticket spec serializes");
    let content = render_ticket_json_document(
        input.id,
        input.lane,
        input.title,
        input.created_at,
        input.updated_at,
        input.status,
        spec,
        input.attachments.to_vec(),
        input.extra.clone(),
    )
    .expect("validated JSON ticket serializes");

    if input.sections.progress.is_empty()
        && input.sections.record.is_empty()
        && input.sections.next_step.is_empty()
    {
        return content;
    }

    let mut document =
        super::spec::parse_ticket_json_document(&content).expect("new JSON ticket parses");
    for line in &input.sections.progress {
        document.progress_record.push(crate::TicketProgressRecord {
            at: None,
            summary: line.trim().to_string(),
            evidence: Vec::new(),
        });
    }
    for line in &input.sections.record {
        document.progress_record.push(crate::TicketProgressRecord {
            at: None,
            summary: format!("Record: {}", line.trim()),
            evidence: Vec::new(),
        });
    }
    for line in &input.sections.next_step {
        document.progress_record.push(crate::TicketProgressRecord {
            at: None,
            summary: format!("Next step: {}", line.trim()),
            evidence: Vec::new(),
        });
    }
    super::spec::serialize_ticket_json_document(&document)
        .expect("new JSON ticket with sections serializes")
}

pub(super) fn render_ticket_section(lines: &[String]) -> String {
    let rendered: Vec<String> = lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| format!("- {line}"))
        .collect();
    rendered.join("\n")
}

pub(super) fn append_ticket_body_sections(
    body: &str,
    progress: &[String],
    record: &[String],
    next_step: &[String],
) -> String {
    let body = append_markdown_section(body, "Progress Record", progress);
    let body = append_markdown_section(&body, "Record", record);
    append_markdown_section(&body, "Next Step", next_step)
}

fn append_markdown_section(body: &str, heading: &str, lines: &[String]) -> String {
    let addition = render_ticket_section(lines);
    if addition.is_empty() {
        return body.to_string();
    }

    let heading_line = format!("# {heading}");
    let mut body_lines: Vec<String> = body.lines().map(ToString::to_string).collect();
    if let Some(start) = body_lines
        .iter()
        .position(|line| line.trim() == heading_line)
    {
        let mut end = body_lines.len();
        for (index, line) in body_lines.iter().enumerate().skip(start + 1) {
            if line.starts_with("# ") {
                end = index;
                break;
            }
        }
        let insert_at = if end > start + 1 && body_lines[end - 1].trim().is_empty() {
            end - 1
        } else {
            end
        };
        let mut insert_lines = Vec::new();
        if insert_at > start + 1 && !body_lines[insert_at - 1].trim().is_empty() {
            insert_lines.push(String::new());
        }
        insert_lines.extend(addition.lines().map(ToString::to_string));
        if insert_at < body_lines.len() && !body_lines[insert_at].trim().is_empty() {
            insert_lines.push(String::new());
        }
        body_lines.splice(insert_at..insert_at, insert_lines);
        let mut rendered = body_lines.join("\n");
        if body.ends_with('\n') {
            rendered.push('\n');
        }
        return rendered;
    }

    let mut rendered = body.trim_end().to_string();
    if !rendered.is_empty() {
        rendered.push_str("\n\n");
    }
    rendered.push_str(&heading_line);
    rendered.push_str("\n\n");
    rendered.push_str(&addition);
    rendered.push('\n');
    rendered
}
