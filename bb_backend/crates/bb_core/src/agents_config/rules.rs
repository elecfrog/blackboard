/// YAML frontmatter header prepended to `CodeBuddy` Rules files.
const RULES_HEADER: &str = "\
---
description: Blackboard cross-Agent rules
globs:
alwaysApply: true
type: always
---
";

/// Build the full content to write for a Rules-type connector.
pub(super) fn wrap_rules_body(source: &[u8]) -> Vec<u8> {
    let mut out = RULES_HEADER.as_bytes().to_vec();
    out.extend_from_slice(source);
    out
}

/// Extract the body (everything after the closing `---` of the YAML
/// frontmatter) from a `CodeBuddy` Rules file. If no valid frontmatter is
/// found, the entire content is returned as-is.
pub(super) fn extract_rules_body(content: &str) -> &str {
    // A CodeBuddy Rules frontmatter starts with "---\n" and ends with the
    // next standalone "---". We look for the second `---` on its own line.
    if !content.starts_with("---") {
        return content;
    }
    // Find the closing "---" after the opening one
    let after_first = &content[3..];
    if let Some(pos) = after_first.find("---") {
        let body_start = 3 + pos + 3; // skip opening ---, content, closing ---
        if body_start < content.len() {
            return content[body_start..].trim_start_matches('\n');
        }
    }
    content
}
