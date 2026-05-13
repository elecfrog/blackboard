use std::collections::BTreeMap;

use crate::{FrontmatterExtra, TicketBodySections};

use super::frontmatter::render_frontmatter;

pub(super) struct TicketRenderInput<'a> {
    pub id: &'a str,
    pub lane: &'a str,
    pub title: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub status: &'a str,
    pub extra: &'a FrontmatterExtra,
    pub sections: &'a TicketBodySections,
}

pub(super) fn render_ticket(input: TicketRenderInput<'_>) -> String {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_string(), input.id.to_string());
    fields.insert("lane".to_string(), input.lane.to_string());
    fields.insert("title".to_string(), input.title.to_string());
    fields.insert("created_at".to_string(), input.created_at.to_string());
    fields.insert("updated_at".to_string(), input.updated_at.to_string());
    fields.insert("status".to_string(), input.status.to_string());
    for (key, value) in input.extra {
        fields.insert(key.clone(), value.clone());
    }

    format!(
        "{}\n# 当前进展\n\n{}\n\n# 记录\n\n{}\n\n# 下一步\n\n{}\n",
        render_frontmatter(&fields),
        render_ticket_section(&input.sections.progress),
        render_ticket_section(&input.sections.record),
        render_ticket_section(&input.sections.next_step),
    )
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
    let body = append_markdown_section(body, "当前进展", progress);
    let body = append_markdown_section(&body, "记录", record);
    append_markdown_section(&body, "下一步", next_step)
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
