use crate::{InboxError, InboxNoteInput};

pub(super) fn validate_input(input: &InboxNoteInput) -> Result<(), InboxError> {
    if input.source.trim().is_empty() {
        return Err(InboxError::InvalidInput("source is required".to_string()));
    }
    if input.topic.trim().is_empty() {
        return Err(InboxError::InvalidInput("topic is required".to_string()));
    }
    Ok(())
}
