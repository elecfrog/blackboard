use crate::common::search::{matching_lines, validate_search_query};
use crate::{Blackboard, InboxError, NoteSearchMatch, NoteSearchResult};

impl Blackboard {
    pub fn search_notes(&self, query: &str) -> Result<NoteSearchResult, InboxError> {
        let query = validate_search_query(query)?;
        let needle = query.to_lowercase();
        let mut matches = Vec::new();

        for entry in self.list_notes()? {
            let note = self.read_note(&entry.name)?;
            for (line, snippet) in matching_lines(&note.content, &needle) {
                matches.push(NoteSearchMatch {
                    filename: note.name.clone(),
                    path: format!("inbox/{}", note.name),
                    line,
                    snippet,
                });
            }
        }

        Ok(NoteSearchResult { matches })
    }
}
