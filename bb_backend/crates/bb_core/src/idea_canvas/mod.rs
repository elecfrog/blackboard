//! Idea Canvas CRUD and sticky note persistence for a single project.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::{
    Blackboard, CreateIdeaCanvasInput, CreateStickyNoteInput, IdeaCanvasDeleteResult,
    IdeaCanvasDetail, IdeaCanvasEntry, IdeaCanvasIndex, IdeaCanvasMaintenance,
    IdeaCanvasWriteResult, InboxError, MaintenanceCheck, MaintenanceState, StickyNote,
    StickyNoteDeleteResult, StickyNoteWriteResult, UpdateIdeaCanvasInput, UpdateStickyNoteInput,
};

const CONSISTENCY_CHECKS: [&str; 3] = ["id", "filename", "duplicates"];
const DEFAULT_NOTE_COLOR: &str = "yellow";

impl Blackboard {
    fn idea_canvas_dir(&self) -> Result<PathBuf, InboxError> {
        let dir = self.root().join("idea_canvases");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|source| InboxError::Io {
                path: dir.clone(),
                source,
            })?;
        }
        let metadata = fs::symlink_metadata(&dir).map_err(|source| InboxError::Io {
            path: dir.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(InboxError::InvalidInput(
                "idea_canvases directory is a symlink".to_string(),
            ));
        }
        Ok(dir)
    }

    pub fn list_idea_canvases(&self) -> Result<IdeaCanvasIndex, InboxError> {
        let mut canvases = Vec::new();
        for canvas in self.read_canvas_files()? {
            canvases.push(IdeaCanvasEntry {
                id: canvas.id,
                title: canvas.title,
                note_count: canvas.notes.len(),
                created_at: canvas.created_at,
                updated_at: canvas.updated_at,
            });
        }
        canvases.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.title.cmp(&b.title)));
        Ok(IdeaCanvasIndex {
            generated_at: now_timestamp(),
            canvases,
        })
    }

    pub fn read_idea_canvas_index(&self) -> Result<IdeaCanvasIndex, InboxError> {
        let index_path = self.root().join("__idea_canvases__.json");
        match fs::read_to_string(&index_path) {
            Ok(content) => serde_json::from_str(&content).or_else(|_| {
                self.rebuild_idea_canvas_index()?;
                read_json_index(index_path)
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.rebuild_idea_canvas_index()?;
                read_json_index(index_path)
            }
            Err(source) => Err(InboxError::Io {
                path: index_path,
                source,
            }),
        }
    }

    pub fn rebuild_idea_canvas_index(&self) -> Result<(), InboxError> {
        let index = self.list_idea_canvases()?;
        let json = serde_json::to_string_pretty(&index).map_err(|err| {
            InboxError::InvalidInput(format!("failed to serialize idea canvas index: {err}"))
        })?;
        let index_path = self.root().join("__idea_canvases__.json");
        write_atomic(&index_path, &json)?;
        Ok(())
    }

    pub fn read_idea_canvas(&self, id: &str) -> Result<IdeaCanvasDetail, InboxError> {
        let id = validate_canvas_id(id)?;
        let path = self.canvas_path(id)?;
        if !path.exists() {
            return Err(InboxError::NotFound(format!("idea canvas not found: {id}")));
        }
        self.read_canvas_file(&path)
    }

    pub fn create_idea_canvas(
        &self,
        input: CreateIdeaCanvasInput,
    ) -> Result<IdeaCanvasWriteResult, InboxError> {
        let title = validate_title(&input.title)?;
        let id = match input.id {
            Some(id) => validate_canvas_id(&id)?.to_string(),
            None => self.allocate_canvas_id(title)?,
        };
        let path = self.canvas_path(&id)?;
        if path.exists() {
            return Err(InboxError::TicketWriteConflict(format!(
                "idea canvas already exists: {id}"
            )));
        }
        let now = now_timestamp();
        let canvas = IdeaCanvasDetail {
            id,
            title: title.to_string(),
            created_at: now.clone(),
            updated_at: now,
            notes: Vec::new(),
        };
        self.write_canvas(&canvas)?;
        Ok(IdeaCanvasWriteResult {
            canvas,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    pub fn update_idea_canvas(
        &self,
        input: UpdateIdeaCanvasInput,
    ) -> Result<IdeaCanvasWriteResult, InboxError> {
        let mut canvas = self.read_idea_canvas(&input.id)?;
        if let Some(title) = input.title {
            canvas.title = validate_title(&title)?.to_string();
        }
        canvas.updated_at = now_timestamp();
        self.write_canvas(&canvas)?;
        Ok(IdeaCanvasWriteResult {
            canvas,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    pub fn delete_idea_canvas(&self, id: &str) -> Result<IdeaCanvasDeleteResult, InboxError> {
        let id = validate_canvas_id(id)?.to_string();
        let path = self.canvas_path(&id)?;
        if !path.exists() {
            return Err(InboxError::NotFound(format!("idea canvas not found: {id}")));
        }
        fs::remove_file(&path).map_err(|source| InboxError::Io { path, source })?;
        Ok(IdeaCanvasDeleteResult {
            id,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    pub fn create_sticky_note(
        &self,
        canvas_id: &str,
        input: CreateStickyNoteInput,
    ) -> Result<StickyNoteWriteResult, InboxError> {
        let mut canvas = self.read_idea_canvas(canvas_id)?;
        validate_point(input.x, input.y)?;
        let now = now_timestamp();
        let note = StickyNote {
            id: allocate_note_id(&canvas),
            text: validate_note_text(&input.text)?,
            x: input.x,
            y: input.y,
            color: normalize_color(input.color)?,
            created_at: now.clone(),
            updated_at: now,
        };
        canvas.notes.push(note.clone());
        canvas.updated_at = now_timestamp();
        self.write_canvas(&canvas)?;
        Ok(StickyNoteWriteResult {
            canvas,
            note,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    pub fn update_sticky_note(
        &self,
        input: UpdateStickyNoteInput,
    ) -> Result<StickyNoteWriteResult, InboxError> {
        let mut canvas = self.read_idea_canvas(&input.canvas_id)?;
        let note_id = validate_note_id(&input.note_id)?.to_string();
        let index = canvas
            .notes
            .iter()
            .position(|note| note.id == note_id)
            .ok_or_else(|| InboxError::NotFound(format!("sticky note not found: {note_id}")))?;
        if let Some(text) = input.text {
            canvas.notes[index].text = validate_note_text(&text)?;
        }
        if input.x.is_some() || input.y.is_some() {
            let x = input.x.unwrap_or(canvas.notes[index].x);
            let y = input.y.unwrap_or(canvas.notes[index].y);
            validate_point(x, y)?;
            canvas.notes[index].x = x;
            canvas.notes[index].y = y;
        }
        if input.color.is_some() {
            canvas.notes[index].color = normalize_color(input.color)?;
        }
        canvas.notes[index].updated_at = now_timestamp();
        let note = canvas.notes[index].clone();
        canvas.updated_at = now_timestamp();
        self.write_canvas(&canvas)?;
        Ok(StickyNoteWriteResult {
            canvas,
            note,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    pub fn delete_sticky_note(
        &self,
        canvas_id: &str,
        note_id: &str,
    ) -> Result<StickyNoteDeleteResult, InboxError> {
        let mut canvas = self.read_idea_canvas(canvas_id)?;
        let note_id = validate_note_id(note_id)?.to_string();
        let before = canvas.notes.len();
        canvas.notes.retain(|note| note.id != note_id);
        if canvas.notes.len() == before {
            return Err(InboxError::NotFound(format!(
                "sticky note not found: {note_id}"
            )));
        }
        canvas.updated_at = now_timestamp();
        self.write_canvas(&canvas)?;
        Ok(StickyNoteDeleteResult {
            canvas_id: canvas.id,
            note_id,
            maintenance: self.idea_canvas_maintenance(),
        })
    }

    fn read_canvas_files(&self) -> Result<Vec<IdeaCanvasDetail>, InboxError> {
        let dir = self.idea_canvas_dir()?;
        let mut canvases = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|source| InboxError::Io {
            path: dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| InboxError::Io {
                path: dir.clone(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| InboxError::Io {
                    path: entry.path(),
                    source,
                })?
                .is_file()
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".json") {
                continue;
            }
            canvases.push(self.read_canvas_file(&entry.path())?);
        }
        Ok(canvases)
    }

    fn canvas_path(&self, id: &str) -> Result<PathBuf, InboxError> {
        let id = validate_canvas_id(id)?;
        Ok(self.idea_canvas_dir()?.join(format!("{id}.json")))
    }

    fn read_canvas_file(&self, path: &Path) -> Result<IdeaCanvasDetail, InboxError> {
        let dir = self.idea_canvas_dir()?;
        let canonical = crate::canonicalize(path)?;
        if !canonical.starts_with(&dir) {
            return Err(InboxError::InvalidName(crate::path_to_string(path)));
        }
        let content = fs::read_to_string(&canonical).map_err(|source| InboxError::Io {
            path: canonical.clone(),
            source,
        })?;
        let canvas: IdeaCanvasDetail = serde_json::from_str(&content).map_err(|err| {
            InboxError::InvalidInput(format!(
                "invalid idea canvas JSON {}: {err}",
                path.display()
            ))
        })?;
        validate_canvas_id(&canvas.id)?;
        validate_title(&canvas.title)?;
        for note in &canvas.notes {
            validate_note_id(&note.id)?;
            validate_note_text(&note.text)?;
            validate_point(note.x, note.y)?;
            normalize_color(Some(note.color.clone()))?;
        }
        Ok(canvas)
    }

    fn write_canvas(&self, canvas: &IdeaCanvasDetail) -> Result<(), InboxError> {
        validate_canvas_id(&canvas.id)?;
        let path = self.canvas_path(&canvas.id)?;
        let json = serde_json::to_string_pretty(canvas).map_err(|err| {
            InboxError::InvalidInput(format!("failed to serialize idea canvas: {err}"))
        })?;
        write_atomic(&path, &json)?;
        let _ = self.rebuild_idea_canvas_index();
        Ok(())
    }

    fn allocate_canvas_id(&self, title: &str) -> Result<String, InboxError> {
        let base = crate::slug_segment(title, "canvas");
        let base = validate_canvas_id(&base).unwrap_or("canvas");
        let mut candidate = base.to_string();
        let mut suffix = 2;
        while self.canvas_path(&candidate)?.exists() {
            candidate = format!("{base}-{suffix}");
            suffix += 1;
        }
        Ok(candidate)
    }

    fn idea_canvas_maintenance(&self) -> IdeaCanvasMaintenance {
        let export = match self.rebuild_idea_canvas_index() {
            Ok(()) => MaintenanceState {
                status: "completed".to_string(),
                mode: "__idea_canvases__.json".to_string(),
                message: None,
            },
            Err(err) => MaintenanceState {
                status: "failed".to_string(),
                mode: "__idea_canvases__.json".to_string(),
                message: Some(format!("index rebuild failed: {err}")),
            },
        };
        IdeaCanvasMaintenance {
            consistency: self.idea_canvas_consistency(),
            export,
        }
    }

    fn idea_canvas_consistency(&self) -> MaintenanceCheck {
        let mut errors = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for canvas in self.read_canvas_files().unwrap_or_default() {
            if !seen.insert(canvas.id.clone()) {
                errors.push(format!("duplicate canvas id {}", canvas.id));
            }
        }
        MaintenanceCheck {
            status: if errors.is_empty() {
                "passed".to_string()
            } else {
                "failed".to_string()
            },
            checks: CONSISTENCY_CHECKS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            errors,
        }
    }
}

fn read_json_index(path: PathBuf) -> Result<IdeaCanvasIndex, InboxError> {
    let content = fs::read_to_string(&path).map_err(|source| InboxError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&content)
        .map_err(|err| InboxError::InvalidInput(format!("__idea_canvases__.json invalid: {err}")))
}

fn write_atomic(path: &PathBuf, content: &str) -> Result<(), InboxError> {
    let tmp_path = path.with_extension(format!("json.{}.tmp", std::process::id()));
    fs::write(&tmp_path, content.as_bytes()).map_err(|source| InboxError::Io {
        path: tmp_path.clone(),
        source,
    })?;
    fs::rename(&tmp_path, path).map_err(|source| InboxError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(())
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn validate_title(value: &str) -> Result<&str, InboxError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 120 || trimmed.chars().any(|ch| ch == '\0') {
        return Err(InboxError::InvalidInput(
            "canvas title must be 1..120 characters".to_string(),
        ));
    }
    Ok(trimmed)
}

fn validate_canvas_id(value: &str) -> Result<&str, InboxError> {
    validate_slug("canvas id", value, 80)
}

fn validate_note_id(value: &str) -> Result<&str, InboxError> {
    validate_slug("note id", value, 80)
}

fn validate_slug<'a>(label: &str, value: &'a str, max_len: usize) -> Result<&'a str, InboxError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value || trimmed.len() > max_len {
        return Err(InboxError::InvalidInput(format!("{label} is invalid")));
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(InboxError::InvalidInput(format!("{label} is invalid")));
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(InboxError::InvalidInput(format!("{label} is invalid")));
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
        return Err(InboxError::InvalidInput(format!("{label} is invalid")));
    }
    Ok(trimmed)
}

fn validate_note_text(value: &str) -> Result<String, InboxError> {
    if value.len() > 4_000 || value.chars().any(|ch| ch == '\0') {
        return Err(InboxError::InvalidInput(
            "sticky note text is invalid".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn validate_point(x: f64, y: f64) -> Result<(), InboxError> {
    if !x.is_finite() || !y.is_finite() || x < -100_000.0 || y < -100_000.0 {
        return Err(InboxError::InvalidInput(
            "sticky note position is invalid".to_string(),
        ));
    }
    Ok(())
}

fn normalize_color(value: Option<String>) -> Result<String, InboxError> {
    let value = value.unwrap_or_else(|| DEFAULT_NOTE_COLOR.to_string());
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty()
        || trimmed.len() > 32
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '#')
    {
        return Err(InboxError::InvalidInput(
            "sticky note color is invalid".to_string(),
        ));
    }
    Ok(trimmed)
}

fn allocate_note_id(canvas: &IdeaCanvasDetail) -> String {
    let mut max = 0;
    for note in &canvas.notes {
        if let Some(counter) = note
            .id
            .strip_prefix("note-")
            .and_then(|value| value.parse().ok())
        {
            max = max.max(counter);
        }
    }
    format!("note-{:04}", max + 1)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::{Blackboard, CreateIdeaCanvasInput, CreateStickyNoteInput, UpdateStickyNoteInput};

    fn fixture() -> (TempDir, Blackboard) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("inbox")).unwrap();
        fs::create_dir_all(root.join("tickets")).unwrap();
        fs::write(
            root.join("__project__.json"),
            r##"{
  "name": "blackboard",
  "type": "tool",
  "lanes": [
    { "id": "bbp", "label": "Product", "color": "#7c3aed", "description": "", "status": "active" }
  ]
}"##,
        )
        .unwrap();
        let board = Blackboard::open_with_name(root, "blackboard").unwrap();
        (temp, board)
    }

    #[test]
    fn create_canvas_and_edit_sticky_note() {
        let (_temp, board) = fixture();
        let created = board
            .create_idea_canvas(CreateIdeaCanvasInput {
                title: "Today".to_string(),
                id: None,
            })
            .unwrap();
        assert_eq!(created.canvas.title, "Today");

        let note = board
            .create_sticky_note(
                &created.canvas.id,
                CreateStickyNoteInput {
                    text: "first thought".to_string(),
                    x: 120.0,
                    y: 80.0,
                    color: Some("yellow".to_string()),
                },
            )
            .unwrap();
        assert_eq!(note.note.text, "first thought");

        let updated = board
            .update_sticky_note(UpdateStickyNoteInput {
                canvas_id: created.canvas.id.clone(),
                note_id: note.note.id.clone(),
                text: Some("moved thought".to_string()),
                x: Some(240.0),
                y: Some(160.0),
                color: None,
            })
            .unwrap();
        assert_eq!(updated.note.text, "moved thought");
        assert_eq!(updated.note.x, 240.0);

        let list = board.read_idea_canvas_index().unwrap();
        assert_eq!(list.canvases.len(), 1);
        assert_eq!(list.canvases[0].note_count, 1);
    }
}
