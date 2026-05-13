use chrono::{SecondsFormat, Utc};

use crate::InboxNoteInput;

pub fn render_note(input: &InboxNoteInput) -> String {
    let title = input.title.as_deref().unwrap_or("Inbox Note").trim();
    let time = input
        .time
        .clone()
        .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    let project = input.project.as_deref().unwrap_or("-").trim();

    format!(
        "# {title}\n\n时间: {time}\n来源: {}\n项目: {project}\n\n## 做了什么\n\n{}\n\n## 验证了什么\n\n{}\n\n## 下一步\n\n{}\n\n## 相关位置\n\n{}\n",
        input.source.trim(),
        render_lines(&input.done),
        render_lines(&input.validation),
        render_lines(&input.next_step),
        render_lines(&input.related_locations),
    )
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
