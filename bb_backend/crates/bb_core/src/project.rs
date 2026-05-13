//! Project metadata, lane CRUD, and project index logic for a single project.

use crate::{
    ArchiveLaneResult, Blackboard, InboxError, LaneDef, ProjectBoardViewSettings, ProjectIndex,
    ProjectMeta,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Lane lifecycle states.
pub(crate) const LANE_STATUSES: [&str; 2] = ["active", "archived"];

// ─── impl Blackboard: project/lane methods ───────────────────────────────────

impl Blackboard {
    /// Path to this project's `__project__.json`.
    pub fn project_meta_path(&self) -> std::path::PathBuf {
        self.root().join("__project__.json")
    }

    /// Read and parse `__project__.json`.
    pub fn read_project_meta(&self) -> Result<ProjectMeta, InboxError> {
        read_project_meta(&self.project_meta_path())
    }

    /// Serialize and atomically write a `ProjectMeta` (as JSON).
    pub fn write_project_meta(&self, meta: &ProjectMeta) -> Result<(), InboxError> {
        let path = self.project_meta_path();
        let rendered =
            serde_json::to_string_pretty(meta).map_err(|err| InboxError::InvalidProjectMeta {
                path: path.clone(),
                message: format!("failed to serialize __project__.json: {err}"),
            })?;
        crate::write_file_atomic(&path, &rendered)
    }

    /// Return all lanes defined on the project, regardless of status.
    pub fn list_lanes(&self) -> Result<Vec<LaneDef>, InboxError> {
        Ok(self.read_project_meta()?.lanes)
    }

    /// Create or update a lane by id.
    pub fn upsert_lane(&self, def: LaneDef) -> Result<LaneDef, InboxError> {
        let validated = validate_lane_def(&def)?;
        let mut meta = self.read_project_meta()?;
        if let Some(existing) = meta.lanes.iter_mut().find(|lane| lane.id == validated.id) {
            *existing = validated.clone();
        } else {
            meta.lanes.push(validated.clone());
        }
        self.write_project_meta(&meta)?;
        let _ = self.rebuild_project_index();
        Ok(validated)
    }

    /// Archive a lane.
    pub fn archive_lane(&self, id: &str) -> Result<ArchiveLaneResult, InboxError> {
        let id = validate_lane_id(id)?.to_string();
        let mut meta = self.read_project_meta()?;
        let lane = match meta.lanes.iter_mut().find(|lane| lane.id == id) {
            Some(lane) => {
                lane.status = "archived".to_string();
                lane.clone()
            }
            None => {
                return Err(InboxError::InvalidInput(format!(
                    "lane `{id}` is not defined on this project"
                )))
            }
        };
        self.write_project_meta(&meta)?;
        let _ = self.rebuild_project_index();

        let affected = self
            .list_tickets()?
            .tickets
            .into_iter()
            .filter(|entry| entry.lane.as_deref() == Some(id.as_str()))
            .count();
        Ok(ArchiveLaneResult {
            lane,
            affected_ticket_count: affected,
        })
    }

    /// Persist BoardView preferences in `__project__.json`.
    pub fn update_board_view(
        &self,
        settings: ProjectBoardViewSettings,
    ) -> Result<ProjectBoardViewSettings, InboxError> {
        let settings = validate_board_view_settings(settings)?;
        let mut meta = self.read_project_meta()?;
        meta.board_view = settings.clone();
        self.write_project_meta(&meta)?;
        Ok(settings)
    }

    /// Rebuild the persistent project index (`__project__.json`).
    /// Since `__project__.json` is now the single source of truth for both
    /// metadata and index, this simply re-writes the current meta as JSON
    /// (normalizing format / filling defaults).
    pub fn rebuild_project_index(&self) -> Result<(), InboxError> {
        let meta = self.read_project_meta()?;
        self.write_project_meta(&meta)
    }

    /// Read the persisted project index. Rebuilds if missing.
    pub fn read_project_index(&self) -> Result<ProjectIndex, InboxError> {
        let index_path = self.root().join("__project__.json");
        match fs::read_to_string(&index_path) {
            Ok(content) => serde_json::from_str(&content).or_else(|_| {
                self.rebuild_project_index()?;
                let fresh = fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                    path: index_path.clone(),
                    source,
                })?;
                serde_json::from_str(&fresh).map_err(|err| {
                    InboxError::InvalidInput(format!("__project__.json invalid: {err}"))
                })
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.rebuild_project_index()?;
                let content = fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                    path: index_path,
                    source,
                })?;
                serde_json::from_str(&content).map_err(|err| {
                    InboxError::InvalidInput(format!("__project__.json invalid: {err}"))
                })
            }
            Err(source) => Err(InboxError::Io {
                path: index_path,
                source,
            }),
        }
    }
}

// ─── Free helper functions ───────────────────────────────────────────────────

pub(crate) fn read_project_meta(path: &Path) -> Result<ProjectMeta, InboxError> {
    let content = fs::read_to_string(path).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str::<ProjectMeta>(&content).map_err(|err| InboxError::InvalidProjectMeta {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

pub fn validate_board_view_settings(
    settings: ProjectBoardViewSettings,
) -> Result<ProjectBoardViewSettings, InboxError> {
    let mut hidden = BTreeSet::new();
    for status in settings.hidden_statuses {
        crate::ticket::validate_ticket_status(&status)?;
        hidden.insert(status);
    }
    if hidden.len() >= crate::ticket::TICKET_STATUSES.len() {
        return Err(InboxError::InvalidInput(
            "board_view.hidden_statuses cannot hide every status column".to_string(),
        ));
    }

    let hidden_statuses = crate::ticket::TICKET_STATUSES
        .iter()
        .filter(|status| hidden.contains(**status))
        .map(|status| status.to_string())
        .collect();

    Ok(ProjectBoardViewSettings { hidden_statuses })
}

/// Validate a lane identifier shape.
pub fn validate_lane_id(id: &str) -> Result<&str, InboxError> {
    let trimmed = id.trim();
    if trimmed != id {
        return Err(InboxError::InvalidInput(format!(
            "lane id must not have surrounding whitespace: `{id}`"
        )));
    }
    if trimmed.len() < 2 || trimmed.len() > 32 {
        return Err(InboxError::InvalidInput(format!(
            "lane id length must be 2..=32 chars: `{id}`"
        )));
    }
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_lowercase() => {}
        _ => {
            return Err(InboxError::InvalidInput(format!(
                "lane id must start with a lowercase letter: `{id}`"
            )))
        }
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
        return Err(InboxError::InvalidInput(format!(
            "lane id body must match [a-z0-9-]: `{id}`"
        )));
    }
    Ok(trimmed)
}

/// Resolve `lane` to an entry of the project's declared lane catalog.
pub fn validate_ticket_lane<'a>(
    lane: &str,
    catalog: &'a [LaneDef],
) -> Result<&'a LaneDef, InboxError> {
    let id = validate_lane_id(lane)?;
    let entry = catalog.iter().find(|entry| entry.id == id).ok_or_else(|| {
        InboxError::InvalidInput(format!(
            "lane `{id}` is not defined in __project__.json; add it via upsert_lane"
        ))
    })?;
    if entry.status != "active" {
        return Err(InboxError::InvalidInput(format!(
            "lane `{id}` is `{}`; new tickets must cite an active lane",
            entry.status
        )));
    }
    Ok(entry)
}

/// Validate a user-supplied `LaneDef` ahead of persisting it.
pub fn validate_lane_def(def: &LaneDef) -> Result<LaneDef, InboxError> {
    let id = validate_lane_id(&def.id)?.to_string();
    let label = crate::ticket::validate_required_string("lane label", &def.label)?.to_string();
    let status = if def.status.is_empty() {
        "active".to_string()
    } else {
        if !LANE_STATUSES.contains(&def.status.as_str()) {
            return Err(InboxError::InvalidInput(format!(
                "lane status must be one of {:?}: `{}`",
                LANE_STATUSES, def.status
            )));
        }
        def.status.clone()
    };
    Ok(LaneDef {
        id,
        label,
        color: def.color.clone(),
        description: def.description.clone(),
        status,
    })
}
