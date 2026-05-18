use std::collections::BTreeMap;
use std::fs;

use crate::{
    validate_lane_id, Blackboard, InboxError, MaintenanceCheck, MaintenanceState, TicketList,
    TicketMaintenance,
};

use super::{is_six_digit_id, split_ticket_frontmatter, TicketIdLock, CONSISTENCY_CHECKS};

impl Blackboard {
    pub(super) fn allocate_ticket_id(&self) -> Result<String, InboxError> {
        let _lock = TicketIdLock::acquire(self.root().join("__tickets__.json.lock"))?;
        let current = self.read_ticket_index_current_counter().unwrap_or(0);
        let max_existing = self.max_existing_ticket_id()?;
        let next = current.max(max_existing) + 1;
        if next > 999_999 {
            return Err(InboxError::InvalidInput(
                "ticket id counter is exhausted".to_string(),
            ));
        }
        let id = format!("{next:06}");
        Ok(id)
    }

    fn read_ticket_index_current_counter(&self) -> Result<u64, InboxError> {
        let index_path = self.root().join("__tickets__.json");
        let content = match fs::read_to_string(&index_path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(source) => {
                return Err(InboxError::Io {
                    path: index_path,
                    source,
                })
            }
        };
        let index: TicketList = serde_json::from_str(&content).map_err(|err| {
            InboxError::InvalidInput(format!("invalid __tickets__.json counter source: {err}"))
        })?;
        let trimmed = index.current_counter.trim();
        if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(InboxError::InvalidInput(format!(
                "invalid __tickets__.json current_counter: {trimmed}"
            )));
        }
        trimmed.parse::<u64>().map_err(|_| {
            InboxError::InvalidInput(format!(
                "invalid __tickets__.json current_counter: {trimmed}"
            ))
        })
    }

    fn max_existing_ticket_id(&self) -> Result<u64, InboxError> {
        let mut max_id = 0;
        for entry in self.list_tickets()?.tickets {
            if let Some(id) = entry.id {
                if let Ok(value) = id.parse::<u64>() {
                    max_id = max_id.max(value);
                }
            }
        }
        let deprecated_dir = self.root().join("tickets").join("_deprecated");
        if deprecated_dir.exists() {
            let metadata =
                fs::symlink_metadata(&deprecated_dir).map_err(|source| InboxError::Io {
                    path: deprecated_dir.clone(),
                    source,
                })?;
            if metadata.file_type().is_symlink() {
                return Err(InboxError::InvalidInput(
                    "tickets/_deprecated directory is a symlink".to_string(),
                ));
            }
            for entry in fs::read_dir(&deprecated_dir).map_err(|source| InboxError::Io {
                path: deprecated_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| InboxError::Io {
                    path: deprecated_dir.clone(),
                    source,
                })?;
                let file_type = entry.file_type().map_err(|source| InboxError::Io {
                    path: entry.path(),
                    source,
                })?;
                if !file_type.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if super::validate_ticket_name(&name).is_err() {
                    continue;
                }
                let content =
                    fs::read_to_string(entry.path()).map_err(|source| InboxError::Io {
                        path: entry.path(),
                        source,
                    })?;
                if let Some(id) = super::ticket_entry_from_content(name, &content).id {
                    if let Ok(value) = id.parse::<u64>() {
                        max_id = max_id.max(value);
                    }
                }
            }
        }
        Ok(max_id)
    }

    /// Rebuild the persistent ticket index (`__tickets__.json`).
    pub fn rebuild_ticket_index(&self) -> Result<(), InboxError> {
        let mut tickets = self.list_tickets()?.tickets;
        for ticket in &mut tickets {
            ticket.spec = None;
        }
        let current_counter = self.read_ticket_index_current_counter().unwrap_or(0);
        let max_ticket_id = self.max_existing_ticket_id()?;
        let effective_counter = current_counter.max(max_ticket_id);
        let counter_str = if effective_counter == 0 {
            "000000".to_string()
        } else {
            format!("{effective_counter:06}")
        };
        let index = TicketList {
            current_counter: counter_str,
            tickets,
        };
        let json = serde_json::to_string_pretty(&index).map_err(|err| {
            InboxError::InvalidInput(format!("failed to serialize ticket index: {err}"))
        })?;
        let index_path = self.root().join("__tickets__.json");
        let tmp_path = self
            .root()
            .join(format!(".__tickets__.json.{}.tmp", std::process::id()));
        fs::write(&tmp_path, json.as_bytes()).map_err(|source| InboxError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        fs::rename(&tmp_path, &index_path).map_err(|source| InboxError::Io {
            path: index_path,
            source,
        })?;
        Ok(())
    }

    /// Read the persisted ticket index from `__tickets__.json`.
    pub fn read_ticket_index(&self) -> Result<TicketList, InboxError> {
        let index_path = self.root().join("__tickets__.json");
        match fs::read_to_string(&index_path) {
            Ok(content) => {
                let index = serde_json::from_str::<TicketList>(&content).map_err(|err| {
                    InboxError::InvalidInput(format!(
                        "corrupted __tickets__.json, rebuilding: {err}"
                    ))
                });
                match index {
                    Ok(index) if self.ticket_index_needs_counter_bootstrap(&index)? => {
                        self.rebuild_ticket_index()?;
                        let fresh =
                            fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                                path: index_path.clone(),
                                source,
                            })?;
                        serde_json::from_str(&fresh).map_err(|err| {
                            InboxError::InvalidInput(format!(
                                "__tickets__.json still invalid after bootstrap rebuild: {err}"
                            ))
                        })
                    }
                    Ok(index) => Ok(index),
                    Err(_) => {
                        self.rebuild_ticket_index()?;
                        let fresh =
                            fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                                path: index_path.clone(),
                                source,
                            })?;
                        serde_json::from_str(&fresh).map_err(|err| {
                            InboxError::InvalidInput(format!(
                                "__tickets__.json still invalid after rebuild: {err}"
                            ))
                        })
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.rebuild_ticket_index()?;
                let content = fs::read_to_string(&index_path).map_err(|source| InboxError::Io {
                    path: index_path,
                    source,
                })?;
                serde_json::from_str(&content).map_err(|err| {
                    InboxError::InvalidInput(format!(
                        "__tickets__.json invalid after rebuild: {err}"
                    ))
                })
            }
            Err(source) => Err(InboxError::Io {
                path: index_path,
                source,
            }),
        }
    }

    fn ticket_index_needs_counter_bootstrap(&self, index: &TicketList) -> Result<bool, InboxError> {
        let counter = index.current_counter.trim();
        if counter.is_empty() || !counter.chars().all(|ch| ch.is_ascii_digit()) {
            return Ok(true);
        }
        if counter.parse::<u64>().unwrap_or(0) != 0 {
            return Ok(false);
        }
        Ok(self.max_existing_ticket_id()? > 0)
    }

    pub(crate) fn ticket_maintenance(&self) -> TicketMaintenance {
        let export = match self.rebuild_ticket_index() {
            Ok(()) => MaintenanceState {
                status: "completed".to_string(),
                mode: "__tickets__.json".to_string(),
                message: None,
            },
            Err(err) => MaintenanceState {
                status: "failed".to_string(),
                mode: "__tickets__.json".to_string(),
                message: Some(format!("index rebuild failed: {err}")),
            },
        };
        let _ = self.rebuild_inbox_index();
        let _ = self.rebuild_project_index();
        TicketMaintenance {
            consistency: self.ticket_consistency(),
            export,
            embedding: MaintenanceState {
                status: "external_embedding_pending".to_string(),
                mode: "not_implemented".to_string(),
                message: Some(
                    "qmd embedding is reported structurally; agents do not run shell maintenance"
                        .to_string(),
                ),
            },
        }
    }

    fn ticket_consistency(&self) -> MaintenanceCheck {
        match self.ticket_consistency_errors() {
            Ok(errors) if errors.is_empty() => MaintenanceCheck {
                status: "passed".to_string(),
                checks: CONSISTENCY_CHECKS
                    .iter()
                    .map(|check| check.to_string())
                    .collect(),
                errors: vec![],
            },
            Ok(errors) => MaintenanceCheck {
                status: "failed".to_string(),
                checks: CONSISTENCY_CHECKS
                    .iter()
                    .map(|check| check.to_string())
                    .collect(),
                errors,
            },
            Err(err) => MaintenanceCheck {
                status: "error".to_string(),
                checks: CONSISTENCY_CHECKS
                    .iter()
                    .map(|check| check.to_string())
                    .collect(),
                errors: vec![format!("consistency check failed: {err}")],
            },
        }
    }

    fn ticket_consistency_errors(&self) -> Result<Vec<String>, InboxError> {
        let mut errors = Vec::new();
        let mut seen = BTreeMap::<String, String>::new();
        let mut max_id = 0;
        let lane_catalog = self
            .read_project_meta()
            .map(|m| m.lanes)
            .unwrap_or_default();
        for entry in self.list_tickets()?.tickets {
            let content = match fs::read_to_string(self.root().join(&entry.path)) {
                Ok(content) => content,
                Err(source) => {
                    return Err(InboxError::Io {
                        path: self.root().join(&entry.path),
                        source,
                    })
                }
            };
            if entry.name.ends_with(".json") {
                if let Some(error) = entry.metadata_error.clone() {
                    errors.push(format!("invalid JSON ticket in {}: {error}", entry.path));
                    continue;
                }
                let id = entry.id.clone().unwrap_or_default();
                if !is_six_digit_id(&id) {
                    errors.push(format!("invalid id in {}: {}", entry.path, id));
                    continue;
                }
                if let Ok(value) = id.parse::<u64>() {
                    max_id = max_id.max(value);
                }
                let lane = entry.lane.clone().unwrap_or_default();
                if validate_lane_id(&lane).is_err() {
                    errors.push(format!("invalid lane in {}: {}", entry.path, lane));
                } else if !lane_catalog.iter().any(|def| def.id == lane) {
                    errors.push(format!(
                        "lane `{lane}` in {} is not defined in __project__.json",
                        entry.path
                    ));
                }
                let new_prefix = format!("{id}-");
                if !entry.name.starts_with(&new_prefix) {
                    errors.push(format!("filename does not match id in {}", entry.path));
                }
                if let Some(previous) = seen.insert(id.clone(), entry.path.clone()) {
                    errors.push(format!(
                        "duplicate ticket id {id}: {previous}, {}",
                        entry.path
                    ));
                }
                continue;
            }
            let fields = match split_ticket_frontmatter(&content) {
                Ok((fields, _)) => fields,
                Err(err) => {
                    errors.push(format!("invalid frontmatter in {}: {err}", entry.path));
                    continue;
                }
            };

            let id = fields.get("id").cloned().unwrap_or_default();
            if !is_six_digit_id(&id) {
                errors.push(format!("invalid id in {}: {}", entry.path, id));
                continue;
            }
            if let Ok(value) = id.parse::<u64>() {
                max_id = max_id.max(value);
            }
            let lane = fields
                .get("lane")
                .cloned()
                .or_else(|| fields.get("family").cloned())
                .unwrap_or_default();
            if validate_lane_id(&lane).is_err() {
                errors.push(format!("invalid lane in {}: {}", entry.path, lane));
            } else if !lane_catalog.iter().any(|def| def.id == lane) {
                errors.push(format!(
                    "lane `{lane}` in {} is not defined in __project__.json",
                    entry.path
                ));
            }
            let new_prefix = format!("{id}-");
            let legacy_prefix = format!("{lane}-{id}-");
            if !entry.name.starts_with(&new_prefix) && !entry.name.starts_with(&legacy_prefix) {
                errors.push(format!("filename does not match id in {}", entry.path));
            }
            if let Some(previous) = seen.insert(id.clone(), entry.path.clone()) {
                errors.push(format!(
                    "duplicate ticket id {id}: {previous}, {}",
                    entry.path
                ));
            }
            if fields.contains_key("owner")
                || content.lines().any(|line| line.starts_with("owner = \""))
            {
                errors.push(format!("legacy owner field in {}", entry.path));
            }
        }
        let counter = self.read_ticket_index_current_counter().unwrap_or(0);
        if counter < max_id {
            errors.push(format!(
                "__tickets__.json current_counter {counter:06} is lower than max ticket id {max_id:06}"
            ));
        }
        Ok(errors)
    }
}

// ─── Free helper functions ───────────────────────────────────────────────────
