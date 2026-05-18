//! Shared data types for the bb_core crate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ─── Inbox types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InboxNoteEntry {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InboxNote {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreatedInboxNote {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeletedInboxNote {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InboxNoteInput {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub time: Option<String>,
    pub source: String,
    #[serde(default)]
    pub project: Option<String>,
    pub topic: String,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default)]
    pub validation: Vec<String>,
    #[serde(default)]
    pub next_step: Vec<String>,
    #[serde(default)]
    pub related_locations: Vec<String>,
}

/// Persisted inbox index (`__inbox__.json`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InboxIndex {
    pub notes: Vec<InboxIndexEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InboxIndexEntry {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NoteSearchResult {
    pub matches: Vec<NoteSearchMatch>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NoteSearchMatch {
    pub filename: String,
    pub path: String,
    pub line: usize,
    pub snippet: String,
}

// ─── Ticket types ────────────────────────────────────────────────────────────

/// Frontmatter extension bag.
pub type FrontmatterExtra = BTreeMap<String, String>;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateTicketInput {
    pub lane: String,
    pub title: String,
    pub status: String,
    pub spec: TicketSpec,
    pub attachments: Vec<TicketAttachment>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub extra: FrontmatterExtra,
    #[serde(default)]
    pub sections: Option<TicketBodySections>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketBodySections {
    #[serde(default)]
    pub progress: Vec<String>,
    #[serde(default)]
    pub record: Vec<String>,
    #[serde(default)]
    pub next_step: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketSpec {
    pub summary: String,
    pub stories: Vec<TicketStory>,
    pub risks: Vec<TicketRisk>,
    pub progress_record: Vec<TicketProgressRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketStory {
    #[serde(default)]
    pub id: String,
    pub given: String,
    pub when: String,
    pub then: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketRisk {
    pub id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mitigation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketProgressRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketAttachment {
    pub kind: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppendTicketSectionsInput {
    pub id: String,
    #[serde(default)]
    pub progress: Vec<String>,
    #[serde(default)]
    pub record: Vec<String>,
    #[serde(default)]
    pub next_step: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateTicketInput {
    pub id: String,
    #[serde(default)]
    pub frontmatter: Option<TicketFrontmatterPatch>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DeprecateTicketInput {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TicketFrontmatterPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub lane: Option<String>,
    #[serde(default)]
    pub spec: Option<TicketSpec>,
    #[serde(default)]
    pub attachments: Option<Vec<TicketAttachment>>,
    #[serde(default)]
    pub extra: FrontmatterExtra,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TicketList {
    #[serde(default)]
    pub current_counter: String,
    pub tickets: Vec<TicketEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TicketEntry {
    pub name: String,
    pub path: String,
    pub id: Option<String>,
    pub lane: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub attachments: Vec<TicketAttachment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<TicketSpec>,
    pub extra: FrontmatterExtra,
    pub metadata_error: Option<String>,
    pub metadata_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Ticket {
    pub status: String,
    pub name: String,
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketById {
    pub name: String,
    pub path: String,
    pub content: String,
    pub id: Option<String>,
    pub lane: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub attachments: Vec<TicketAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<TicketSpec>,
    pub extra: FrontmatterExtra,
    pub metadata_error: Option<String>,
    pub metadata_warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadTicketInput {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadTicketByIdInput {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BoardSummary {
    pub total: usize,
    pub by_status: BTreeMap<String, usize>,
    pub by_lane: BTreeMap<String, usize>,
    pub metadata_warning_count: usize,
    pub metadata_error_count: usize,
    pub metadata_warnings: Vec<TicketMetadataDiagnostic>,
    pub metadata_errors: Vec<TicketMetadataDiagnostic>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketWriteResult {
    pub ticket: TicketWriteTicket,
    pub maintenance: TicketMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeprecateTicketResult {
    pub ticket: TicketWriteTicket,
    pub removed_dependency_refs: Vec<String>,
    pub removed_attachment_refs: Vec<String>,
    pub maintenance: TicketMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketWriteTicket {
    pub id: String,
    pub lane: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub file_name: String,
    pub path: String,
    pub attachments: Vec<TicketAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<TicketSpec>,
    pub extra: FrontmatterExtra,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketMaintenance {
    pub consistency: MaintenanceCheck,
    pub export: MaintenanceState,
    pub embedding: MaintenanceState,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MaintenanceCheck {
    pub status: String,
    pub checks: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MaintenanceState {
    pub status: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketMetadataDiagnostic {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SearchInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketSearchResult {
    pub matches: Vec<TicketSearchMatch>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TicketSearchMatch {
    pub filename: String,
    pub path: String,
    pub status: String,
    pub line: usize,
    pub snippet: String,
}

// ─── Idea Canvas types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreateIdeaCanvasInput {
    pub title: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateIdeaCanvasInput {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreateStickyNoteInput {
    #[serde(default)]
    pub text: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateStickyNoteInput {
    pub canvas_id: String,
    pub note_id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IdeaCanvasIndex {
    pub generated_at: String,
    pub canvases: Vec<IdeaCanvasEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IdeaCanvasEntry {
    pub id: String,
    pub title: String,
    pub note_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IdeaCanvasDetail {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub notes: Vec<StickyNote>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StickyNote {
    pub id: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IdeaCanvasWriteResult {
    pub canvas: IdeaCanvasDetail,
    pub maintenance: IdeaCanvasMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StickyNoteWriteResult {
    pub canvas: IdeaCanvasDetail,
    pub note: StickyNote,
    pub maintenance: IdeaCanvasMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IdeaCanvasDeleteResult {
    pub id: String,
    pub maintenance: IdeaCanvasMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StickyNoteDeleteResult {
    pub canvas_id: String,
    pub note_id: String,
    pub maintenance: IdeaCanvasMaintenance,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IdeaCanvasMaintenance {
    pub consistency: MaintenanceCheck,
    pub export: MaintenanceState,
}

// ─── Project types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectEntry {
    pub name: String,
    pub uuid: String,
    pub meta: ProjectMeta,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectsRegistry {
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectRegistryEntry>,
    #[serde(default)]
    pub machines: BTreeMap<String, MachineRegistryEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectRegistryEntry {
    pub uuid: String,
    #[serde(default)]
    pub locations: BTreeMap<String, ProjectLocation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectLocation {
    #[serde(default)]
    pub absolute_path: String,
    #[serde(default)]
    pub relative_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum MachineRegistryEntry {
    Current(String),
    Info(MachineInfo),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MachineInfo {
    #[serde(rename = "OS", default)]
    pub os: String,
    #[serde(
        rename = "OS_VERSION",
        alias = "OS_Version",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub os_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectDirectoryCreate {
    pub name: String,
    pub data_root: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectDirectoryOpen {
    pub name: String,
    pub data_root: String,
}

/// Lane definition persisted in `__project__.json` under `lanes`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LaneDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_lane_status")]
    pub status: String,
}

fn default_lane_status() -> String {
    "active".to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArchiveLaneResult {
    pub lane: LaneDef,
    pub affected_ticket_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Optional project data root. When set in the workspace registry entry
    /// under `projects/<project>/__project__.json`, ticket/inbox/wiki data is
    /// read from this directory instead of the registry directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_host: Option<String>,
    #[serde(default)]
    pub lanes: Vec<LaneDef>,
    #[serde(default, skip_serializing_if = "ProjectBoardViewSettings::is_default")]
    pub board_view: ProjectBoardViewSettings,
}

/// Persisted project index (`__project__.json`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectIndex {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_host: Option<String>,
    #[serde(default)]
    pub lanes: Vec<LaneDef>,
    #[serde(default, skip_serializing_if = "ProjectBoardViewSettings::is_default")]
    pub board_view: ProjectBoardViewSettings,
}

/// Project-scoped BoardView preferences persisted in `__project__.json`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectBoardViewSettings {
    #[serde(default)]
    pub hidden_statuses: Vec<String>,
}

impl ProjectBoardViewSettings {
    pub fn is_default(&self) -> bool {
        self.hidden_statuses.is_empty()
    }
}
