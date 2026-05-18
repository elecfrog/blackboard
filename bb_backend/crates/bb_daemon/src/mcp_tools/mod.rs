use bb_core::{
    agents_config, agents_registry, AgentProfile, AppendTicketSectionsInput, CreateTicketInput,
    DeprecateTicketInput, InboxError, InboxNoteInput, LaneDef, ProjectAgentRegistration,
    ReadTicketByIdInput, ReadTicketInput, SearchInput, TicketFrontmatterPatch, UpdateTicketInput,
    Workspace,
};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

mod schema;
pub use schema::tools_list;

const ALL_TOOL_NAMES: &[&str] = &[
    "list_projects",
    "find_work_context",
    "list_agents",
    "upsert_agent",
    "list_inbox_notes",
    "read_inbox_note",
    "archive_inbox_note",
    "delete_inbox_note",
    "list_tickets",
    "read_ticket",
    "read_ticket_by_id",
    "create_ticket",
    "update_ticket",
    "deprecate_ticket",
    "append_ticket_sections",
    "begin_ticket_work",
    "complete_handoff",
    "board_summary",
    "list_lanes",
    "upsert_lane",
    "archive_lane",
    "upsert_project_agent",
    "remove_project_agent",
    "search_notes",
    "search_tickets",
    "create_inbox_note",
    "list_agent_connectors",
    "sync_agent_connector",
    "disconnect_agent_connector",
];

const AGENT_TOOL_NAMES: &[&str] = &[
    "list_projects",
    "find_work_context",
    "list_inbox_notes",
    "read_inbox_note",
    "read_ticket_by_id",
    "create_ticket",
    "update_ticket",
    "append_ticket_sections",
    "begin_ticket_work",
    "complete_handoff",
    "list_lanes",
    "search_notes",
    "search_tickets",
    "create_inbox_note",
];

const BBPM_TOOL_NAMES: &[&str] = &[
    "list_projects",
    "find_work_context",
    "list_inbox_notes",
    "read_inbox_note",
    "archive_inbox_note",
    "delete_inbox_note",
    "list_tickets",
    "read_ticket",
    "read_ticket_by_id",
    "update_ticket",
    "deprecate_ticket",
    "append_ticket_sections",
    "complete_handoff",
    "board_summary",
    "list_lanes",
    "search_notes",
    "search_tickets",
    "create_inbox_note",
];

const ADMIN_TOOL_NAMES: &[&str] = &[
    "list_projects",
    "list_agents",
    "upsert_agent",
    "board_summary",
    "list_lanes",
    "upsert_lane",
    "archive_lane",
    "upsert_project_agent",
    "remove_project_agent",
    "deprecate_ticket",
    "list_agent_connectors",
    "sync_agent_connector",
    "disconnect_agent_connector",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolProfile {
    Agent,
    Bbpm,
    Admin,
    DevAll,
}

impl ToolProfile {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Bbpm => "bbpm",
            Self::Admin => "admin",
            Self::DevAll => "dev-all",
        }
    }

    pub(crate) fn allows(self, name: &str) -> bool {
        match self {
            Self::Agent => AGENT_TOOL_NAMES.contains(&name),
            Self::Bbpm => BBPM_TOOL_NAMES.contains(&name),
            Self::Admin => ADMIN_TOOL_NAMES.contains(&name),
            Self::DevAll => ALL_TOOL_NAMES.contains(&name),
        }
    }
}

pub fn current_tool_profile() -> ToolProfile {
    if is_bbpm_daemon() {
        return ToolProfile::Bbpm;
    }
    match std::env::var("BB_MCP_TOOL_PROFILE")
        .unwrap_or_default()
        .as_str()
    {
        "bbpm" | "bb-pm" => ToolProfile::Bbpm,
        "admin" => ToolProfile::Admin,
        "dev-all" | "all" => ToolProfile::DevAll,
        _ => ToolProfile::Agent,
    }
}

fn ensure_tool_available(name: &str) -> Result<(), (i64, String)> {
    let profile = current_tool_profile();
    if ALL_TOOL_NAMES.contains(&name) && !profile.allows(name) {
        return Err((
            -32601,
            format!(
                "tool `{name}` is not available for MCP profile `{}`",
                profile.as_str()
            ),
        ));
    }
    Ok(())
}

pub fn handle_tool_call(workspace: &Workspace, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call params.name is required".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    ensure_tool_available(name)?;

    match name {
        "list_projects" => return to_tool_result(workspace.list_projects()),
        "find_work_context" => return find_work_context(workspace, arguments),
        "list_agents" => {
            let project = arguments.get("project").and_then(Value::as_str);
            return project.map_or_else(
                || to_tool_result(agents_registry::list_agents(workspace.root())),
                |project| {
                    to_tool_result(agents_registry::list_project_agents(
                        workspace.root(),
                        project,
                    ))
                },
            );
        }
        "upsert_agent" => {
            let input: AgentProfile = serde_json::from_value(arguments)
                .map_err(|err| (-32602, format!("invalid upsert_agent arguments: {err}")))?;
            return to_tool_result(agents_registry::upsert_agent(workspace.root(), input));
        }
        "list_agent_connectors" => {
            return to_tool_result(agents_config::list_agent_connectors(workspace.root()))
        }
        "sync_agent_connector" => {
            let id = arguments.get("id").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "sync_agent_connector requires arguments.id".to_string(),
                )
            })?;
            return to_tool_result(agents_config::sync_agent_connector(workspace.root(), id));
        }
        "disconnect_agent_connector" => {
            let id = arguments.get("id").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "disconnect_agent_connector requires arguments.id".to_string(),
                )
            })?;
            return to_tool_result(agents_config::disconnect_agent_connector(
                workspace.root(),
                id,
            ));
        }
        _ => {}
    }

    let (project, rest) = extract_project(&arguments)?;
    let board = workspace
        .open_project(&project)
        .map_err(inbox_error_to_tool_error)?;

    match name {
        "list_inbox_notes" => to_tool_result(board.list_notes()),
        "read_inbox_note" => {
            let note_name = rest.get("name").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "read_inbox_note requires arguments.name".to_string(),
                )
            })?;
            to_tool_result(board.read_note(note_name))
        }
        "delete_inbox_note" => {
            let note_name = rest.get("name").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "delete_inbox_note requires arguments.name".to_string(),
                )
            })?;
            to_tool_result(board.delete_note(note_name))
        }
        "archive_inbox_note" => {
            let note_name = rest.get("name").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "archive_inbox_note requires arguments.name".to_string(),
                )
            })?;
            let reason = rest.get("reason").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "archive_inbox_note requires arguments.reason".to_string(),
                )
            })?;
            if reason.trim().is_empty() {
                return Err((
                    -32602,
                    "archive_inbox_note arguments.reason must not be empty".to_string(),
                ));
            }
            let archived = board
                .archive_note(note_name)
                .map_err(inbox_error_to_tool_error)?;
            Ok(to_tool_result_value(json!({
                "name": archived.name,
                "original_path": archived.original_path,
                "archived_name": archived.archived_name,
                "archived_path": archived.archived_path,
                "reason": reason,
            })))
        }
        "list_tickets" => to_tool_result(board.list_tickets()),
        "read_ticket" => {
            let input: ReadTicketInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid read_ticket arguments: {err}")))?;
            to_tool_result(board.read_ticket(&input.name))
        }
        "read_ticket_by_id" => {
            let input: ReadTicketByIdInput = serde_json::from_value(rest).map_err(|err| {
                (
                    -32602,
                    format!("invalid read_ticket_by_id arguments: {err}"),
                )
            })?;
            to_tool_result(board.read_ticket_by_id(&input.id))
        }
        "create_ticket" => {
            let input: CreateTicketInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid create_ticket arguments: {err}")))?;
            to_tool_result(board.create_ticket(input))
        }
        "update_ticket" => {
            let input: UpdateTicketInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid update_ticket arguments: {err}")))?;
            to_tool_result(board.update_ticket(input))
        }
        "deprecate_ticket" => {
            let input: DeprecateTicketInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid deprecate_ticket arguments: {err}")))?;
            to_tool_result(board.deprecate_ticket(input))
        }
        "append_ticket_sections" => {
            let input: AppendTicketSectionsInput = serde_json::from_value(rest).map_err(|err| {
                (
                    -32602,
                    format!("invalid append_ticket_sections arguments: {err}"),
                )
            })?;
            to_tool_result(board.append_ticket_sections(input))
        }
        "begin_ticket_work" => begin_ticket_work(&board, rest),
        "complete_handoff" => complete_handoff(&board, project, rest),
        "board_summary" => to_tool_result(board.board_summary()),
        "list_lanes" => to_tool_result(board.list_lanes()),
        "upsert_lane" => {
            let lane: LaneDef = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid upsert_lane arguments: {err}")))?;
            to_tool_result(board.upsert_lane(lane))
        }
        "archive_lane" => {
            let lane_id = rest
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| (-32602, "archive_lane requires arguments.id".to_string()))?;
            to_tool_result(board.archive_lane(lane_id))
        }
        "upsert_project_agent" => {
            let input: ProjectAgentWriteInput = serde_json::from_value(rest).map_err(|err| {
                (
                    -32602,
                    format!("invalid upsert_project_agent arguments: {err}"),
                )
            })?;
            to_tool_result(agents_registry::upsert_project_agent(
                workspace.root(),
                ProjectAgentRegistration {
                    project,
                    agent: input.agent,
                    role: input.role,
                    lanes: input.lanes,
                },
            ))
        }
        "remove_project_agent" => {
            let agent = rest.get("agent").and_then(Value::as_str).ok_or_else(|| {
                (
                    -32602,
                    "remove_project_agent requires arguments.agent".to_string(),
                )
            })?;
            to_tool_result(agents_registry::remove_project_agent(
                workspace.root(),
                &project,
                agent,
            ))
        }
        "search_notes" => {
            let input: SearchInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid search_notes arguments: {err}")))?;
            to_tool_result(board.search_notes(&input.query))
        }
        "search_tickets" => {
            let input: SearchInput = serde_json::from_value(rest)
                .map_err(|err| (-32602, format!("invalid search_tickets arguments: {err}")))?;
            to_tool_result(board.search_tickets(&input.query))
        }
        "create_inbox_note" => {
            let mut input: InboxNoteInput = serde_json::from_value(rest).map_err(|err| {
                (
                    -32602,
                    format!("invalid create_inbox_note arguments: {err}"),
                )
            })?;
            input.project = Some(project);
            to_tool_result(board.create_note(input))
        }
        other => Err((-32602, format!("unknown tool: {other}"))),
    }
}

fn is_bbpm_daemon() -> bool {
    std::env::var("BB_DAEMON").as_deref() == Ok("1")
        && std::env::var("BB_DAEMON_AGENT").as_deref() == Ok("bb-pm")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FindWorkContextInput {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    query: Option<String>,
}

fn find_work_context(workspace: &Workspace, arguments: Value) -> Result<Value, (i64, String)> {
    let input: FindWorkContextInput = serde_json::from_value(arguments).map_err(|err| {
        (
            -32602,
            format!("invalid find_work_context arguments: {err}"),
        )
    })?;
    let project_names = match input.project {
        Some(project) => vec![project],
        None => workspace
            .list_projects()
            .map_err(inbox_error_to_tool_error)?
            .into_iter()
            .map(|project| project.name)
            .collect(),
    };
    let mut projects = Vec::new();
    for project in project_names {
        let board = workspace
            .open_project(&project)
            .map_err(inbox_error_to_tool_error)?;
        let tickets = board
            .list_tickets()
            .map_err(inbox_error_to_tool_error)?
            .tickets
            .into_iter()
            .filter(|ticket| {
                ticket
                    .status
                    .as_deref()
                    .is_some_and(bb_core::ticket::is_open_ticket_status)
            })
            .map(|ticket| {
                json!({
                    "id": ticket.id.unwrap_or_default(),
                    "lane": ticket.lane.unwrap_or_default(),
                    "title": ticket.title.unwrap_or_else(|| ticket.name.clone()),
                    "status": ticket.status.unwrap_or_default(),
                    "path": ticket.path,
                    "assignee": ticket.extra.get("assignee").cloned().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        let (note_matches, ticket_matches) = match input.query.as_deref() {
            Some(query) if !query.trim().is_empty() => (
                serde_json::to_value(
                    board
                        .search_notes(query)
                        .map_err(inbox_error_to_tool_error)?,
                )
                .expect("search notes serializes"),
                serde_json::to_value(
                    board
                        .search_tickets(query)
                        .map_err(inbox_error_to_tool_error)?,
                )
                .expect("search tickets serializes"),
            ),
            _ => (json!({ "matches": [] }), json!({ "matches": [] })),
        };
        projects.push(json!({
            "project": project,
            "active_tickets": tickets,
            "note_matches": note_matches["matches"].clone(),
            "ticket_matches": ticket_matches["matches"].clone(),
        }));
    }
    Ok(to_tool_result_value(json!({
        "generated_at": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "query": input.query.unwrap_or_default(),
        "projects": projects,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BeginTicketWorkInput {
    id: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

fn begin_ticket_work(
    board: &bb_core::ProjectBoard,
    arguments: Value,
) -> Result<Value, (i64, String)> {
    let input: BeginTicketWorkInput = serde_json::from_value(arguments).map_err(|err| {
        (
            -32602,
            format!("invalid begin_ticket_work arguments: {err}"),
        )
    })?;
    let agent = input
        .agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("agent");
    let mut patch = TicketFrontmatterPatch {
        status: Some("in_progress".to_string()),
        ..TicketFrontmatterPatch::default()
    };
    if agent != "agent" {
        patch
            .extra
            .insert("assignee".to_string(), agent.to_string());
    }
    let _ = board
        .update_ticket(UpdateTicketInput {
            id: input.id.clone(),
            frontmatter: Some(patch),
        })
        .map_err(inbox_error_to_tool_error)?;
    let progress = match input.note {
        Some(note) if !note.trim().is_empty() => format!("{agent} 开始执行：{}", note.trim()),
        _ => format!("{agent} 开始执行本 ticket。"),
    };
    to_tool_result(board.append_ticket_sections(AppendTicketSectionsInput {
        id: input.id,
        progress: vec![progress],
        record: Vec::new(),
        next_step: Vec::new(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteHandoffInput {
    id: String,
    source: String,
    topic: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    done: Vec<String>,
    #[serde(default)]
    validation: Vec<String>,
    #[serde(default)]
    next_step: Vec<String>,
    #[serde(default)]
    related_locations: Vec<String>,
}

fn complete_handoff(
    board: &bb_core::ProjectBoard,
    project: String,
    arguments: Value,
) -> Result<Value, (i64, String)> {
    let input: CompleteHandoffInput = serde_json::from_value(arguments)
        .map_err(|err| (-32602, format!("invalid complete_handoff arguments: {err}")))?;
    let note = board
        .create_note(InboxNoteInput {
            title: input.title,
            time: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)),
            source: input.source.clone(),
            project: Some(project),
            topic: input.topic,
            done: input.done.clone(),
            validation: input.validation.clone(),
            next_step: input.next_step.clone(),
            related_locations: input.related_locations,
            related_tickets: vec![input.id.clone()],
            attachments: Vec::new(),
            extra: BTreeMap::default(),
        })
        .map_err(inbox_error_to_tool_error)?;
    let progress = vec![format!(
        "{} 完成阶段工作，handoff 写入 `{}`。",
        input.source, note.name
    )];
    let mut record = Vec::new();
    for item in input.validation {
        record.push(format!("验证：{item}"));
    }
    let ticket = board
        .append_ticket_sections(AppendTicketSectionsInput {
            id: input.id.clone(),
            progress,
            record,
            next_step: input.next_step,
        })
        .map_err(inbox_error_to_tool_error)?;
    let _ = board.update_ticket(UpdateTicketInput {
        id: input.id,
        frontmatter: Some(TicketFrontmatterPatch {
            status: Some("review".to_string()),
            ..TicketFrontmatterPatch::default()
        }),
    });
    Ok(to_tool_result_value(json!({
        "note": note,
        "ticket": ticket.ticket,
        "maintenance": ticket.maintenance
    })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectAgentWriteInput {
    agent: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    lanes: Vec<String>,
}

fn extract_project(arguments: &Value) -> Result<(String, Value), (i64, String)> {
    let object = arguments
        .as_object()
        .ok_or_else(|| (-32602, "tool arguments must be an object".to_string()))?;
    let project = object
        .get("project")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "missing required project argument".to_string()))?
        .to_string();
    let mut rest: Map<String, Value> = object.clone();
    rest.remove("project");
    Ok((project, Value::Object(rest)))
}

fn to_tool_result<T: serde::Serialize>(
    result: Result<T, InboxError>,
) -> Result<Value, (i64, String)> {
    result
        .map(to_tool_result_value)
        .map_err(inbox_error_to_tool_error)
}

fn to_tool_result_value<T: serde::Serialize>(value: T) -> Value {
    let text = serde_json::to_string(&value).expect("tool results serialize");
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn inbox_error_to_tool_error(err: InboxError) -> (i64, String) {
    match err {
        InboxError::InvalidName(_)
        | InboxError::InvalidInput(_)
        | InboxError::NotFound(_)
        | InboxError::InvalidTicketStatus(_)
        | InboxError::TicketNotFound { .. }
        | InboxError::InvalidTicketId(_)
        | InboxError::TicketIdNotFound(_)
        | InboxError::DuplicateTicketId { .. }
        | InboxError::InvalidTicketFamily(_)
        | InboxError::TicketWriteConflict(_)
        | InboxError::InvalidProjectName(_)
        | InboxError::ProjectNotFound(_)
        | InboxError::InvalidProjectMeta { .. } => (-32602, err.to_string()),
        InboxError::RootNotFound(_)
        | InboxError::MissingInbox(_)
        | InboxError::ProjectsRootMissing(_)
        | InboxError::TicketIdLockTimeout(_)
        | InboxError::Io { .. } => (-32603, err.to_string()),
    }
}
