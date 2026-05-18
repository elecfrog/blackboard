use crate::InboxError;

pub fn validate_search_query(query: &str) -> Result<&str, InboxError> {
    if query.trim().is_empty() {
        return Err(InboxError::InvalidInput("query is required".to_string()));
    }
    Ok(query)
}

pub fn matching_lines(content: &str, needle: &str) -> Vec<(usize, String)> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.to_lowercase().contains(needle) {
                Some((index + 1, line.trim().to_string()))
            } else {
                None
            }
        })
        .collect()
}
