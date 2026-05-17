use serde_json::{json, Value};

use super::is_bbpm_daemon;

pub(crate) fn tools_list() -> Value {
    let project_field = json!({
        "type": "string",
        "pattern": "^[^./\\\\][^./\\\\]{0,63}$",
        "description": "Target project name from the Blackboard workspace registry."
    });
    let mut result = json!({
        "tools": [
            {
                "name": "list_projects",
                "description": "List projects visible for the CURRENT machine from the Blackboard workspace registry. Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "find_work_context",
                "description": "Find open ticket and optional inbox/ticket search context across one project or every project before starting work.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "query": { "type": "string" }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "list_agents",
                "description": "List registered Blackboard agents from <bb-root>/agents/agents.toml. Without project, returns the global registry; with project, returns agents assignable to that project.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field },
                    "additionalProperties": false
                }
            },
            {
                "name": "upsert_agent",
                "description": "Create or replace one registered Blackboard agent in <bb-root>/agents/agents.toml.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]{0,63}$" },
                        "display_name": { "type": "string", "minLength": 1 },
                        "kind": { "type": "string", "minLength": 1 },
                        "runtime": { "type": "string" },
                        "scope": { "type": "string", "enum": ["global", "project"] },
                        "status": { "type": "string", "enum": ["active", "inactive", "archived"] },
                        "assignable": { "type": "boolean" },
                        "distribute": { "type": "boolean" },
                        "source_path": { "type": "string" },
                        "roles": { "type": "array", "items": { "type": "string" } },
                        "description": { "type": "string" },
                        "model": { "type": "string" },
                        "instructions": { "type": "string" },
                        "instructions_path": { "type": "string" },
                        "custom_env": { "type": "object", "additionalProperties": { "type": "string" } },
                        "custom_args": { "type": "array", "items": { "type": "string" } },
                        "max_concurrent_tasks": { "type": "integer", "minimum": 1 },
                        "mcp_servers": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]{0,63}$" },
                                    "transport": { "type": "string", "enum": ["stdio", "sse"] },
                                    "command": { "type": "string" },
                                    "args": { "type": "array", "items": { "type": "string" } },
                                    "url": { "type": "string" },
                                    "env": { "type": "object", "additionalProperties": { "type": "string" } }
                                },
                                "required": ["name", "transport"]
                            }
                        },
                        "skills": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["id", "display_name", "kind"],
                    "additionalProperties": false
                }
            },
            {
                "name": "list_inbox_notes",
                "description": "List Markdown note names under the project's inbox.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field },
                    "required": ["project"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_inbox_note",
                "description": "Read one Markdown note from the project's inbox by filename.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "name": { "type": "string" }
                    },
                    "required": ["project", "name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "delete_inbox_note",
                "description": "Delete one Markdown note from the project's inbox by filename after its facts have been reflected in ticket records.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "name": { "type": "string" }
                    },
                    "required": ["project", "name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "list_tickets",
                "description": "List Markdown ticket files and parsed frontmatter metadata under the project's tickets/. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field },
                    "required": ["project"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_ticket",
                "description": "Read one Markdown ticket by filename within a project. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "name": { "type": "string" }
                    },
                    "required": ["project", "name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_ticket_by_id",
                "description": "Read one Markdown ticket by strict six-digit ID within a project. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" }
                    },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "create_ticket",
                "description": "Create one ticket from structured fields under the specified project. Allocates the next project-local ID from existing tickets and writes Markdown under the project tickets directory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "lane": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,31}$" },
                        "title": { "type": "string", "minLength": 1 },
                        "status": { "type": "string", "enum": ["todo", "in_progress", "blocked", "review", "done", "archived"] },
                        "slug": { "type": "string" },
                        "extra": { "type": "object", "additionalProperties": { "type": "string" } },
                        "sections": {
                            "type": "object",
                            "properties": {
                                "progress": { "type": "array", "items": { "type": "string" } },
                                "record": { "type": "array", "items": { "type": "string" } },
                                "next_step": { "type": "array", "items": { "type": "string" } }
                            },
                            "additionalProperties": false
                        }
                    },
                    "required": ["project", "lane", "title", "status"],
                    "additionalProperties": false
                }
            },
            {
                "name": "update_ticket",
                "description": "Update controlled ticket frontmatter fields by six-digit ID within a project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" },
                        "frontmatter": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string", "minLength": 1 },
                                "status": { "type": "string", "enum": ["todo", "in_progress", "blocked", "review", "done", "archived"] },
                                "lane": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,31}$" },
                                "extra": { "type": "object", "additionalProperties": { "type": "string" } },
                                "remove": { "type": "array", "items": { "type": "string" } }
                            },
                            "additionalProperties": false
                        }
                    },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "deprecate_ticket",
                "description": "Move one active ticket by six-digit ID into tickets/_deprecated and remove active dependency/attachment references to it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" }
                    },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "append_ticket_sections",
                "description": "Append structured bullets to an existing ticket body without sending raw Markdown.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" },
                        "progress": { "type": "array", "items": { "type": "string" } },
                        "record": { "type": "array", "items": { "type": "string" } },
                        "next_step": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "begin_ticket_work",
                "description": "Mark a ticket as in_progress, optionally assign an agent, and append a short progress bullet.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" },
                        "agent": { "type": "string" },
                        "note": { "type": "string" }
                    },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "complete_handoff",
                "description": "Create a slim inbox handoff and append completion/validation bullets to the ticket.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[0-9]{6}$" },
                        "source": { "type": "string" },
                        "topic": { "type": "string" },
                        "title": { "type": "string" },
                        "done": { "type": "array", "items": { "type": "string" } },
                        "validation": { "type": "array", "items": { "type": "string" } },
                        "next_step": { "type": "array", "items": { "type": "string" } },
                        "related_locations": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["project", "id", "source", "topic"],
                    "additionalProperties": false
                }
            },
            {
                "name": "board_summary",
                "description": "Summarize a project's ticket counts by status and lane. Read-only.",
                "inputSchema": { "type": "object", "properties": { "project": project_field }, "required": ["project"], "additionalProperties": false }
            },
            {
                "name": "list_lanes",
                "description": "List all lane definitions for a project, including archived ones. Read-only.",
                "inputSchema": { "type": "object", "properties": { "project": project_field }, "required": ["project"], "additionalProperties": false }
            },
            {
                "name": "upsert_lane",
                "description": "Create or update a lane in a project's __project__.json.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,31}$" },
                        "label": { "type": "string", "minLength": 1 },
                        "color": { "type": "string" },
                        "description": { "type": "string" },
                        "status": { "type": "string", "enum": ["active", "archived"] }
                    },
                    "required": ["project", "id", "label"],
                    "additionalProperties": false
                }
            },
            {
                "name": "archive_lane",
                "description": "Mark a lane as archived in __project__.json.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field, "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,31}$" } },
                    "required": ["project", "id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "upsert_project_agent",
                "description": "Register or update an agent membership for a project in agents.toml.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "agent": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]{0,63}$" },
                        "role": { "type": "string" },
                        "lanes": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["project", "agent"],
                    "additionalProperties": false
                }
            },
            {
                "name": "remove_project_agent",
                "description": "Remove one project agent membership from agents.toml. Idempotent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "agent": { "type": "string", "pattern": "^[a-z0-9][a-z0-9-]{0,63}$" }
                    },
                    "required": ["project", "agent"],
                    "additionalProperties": false
                }
            },
            {
                "name": "search_notes",
                "description": "Search direct Markdown notes under a project's inbox. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field, "query": { "type": "string", "minLength": 1 } },
                    "required": ["project", "query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "search_tickets",
                "description": "Search direct Markdown tickets under a project's tickets/. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field, "query": { "type": "string", "minLength": 1 } },
                    "required": ["project", "query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "create_inbox_note",
                "description": "Create one structured Markdown handoff note under the project's inbox.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "title": { "type": "string" },
                        "time": { "type": "string" },
                        "source": { "type": "string" },
                        "topic": { "type": "string" },
                        "done": { "type": "array", "items": { "type": "string" } },
                        "validation": { "type": "array", "items": { "type": "string" } },
                        "next_step": { "type": "array", "items": { "type": "string" } },
                        "related_locations": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["project", "source", "topic"],
                    "additionalProperties": false
                }
            },
            {
                "name": "list_agent_connectors",
                "description": "List supported Agent connectors and their per-target sync state. Workspace-scoped, no project arg. Read-only.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "sync_agent_connector",
                "description": "Overwrite Agent connector target file(s) with the canonical source.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "id": { "type": "string", "enum": ["codex", "codebuddy", "opencode"] } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "disconnect_agent_connector",
                "description": "Delete the Agent connector target file(s).",
                "inputSchema": {
                    "type": "object",
                    "properties": { "id": { "type": "string", "enum": ["codex", "codebuddy", "opencode"] } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            }
        ]
    });
    if !is_bbpm_daemon() {
        if let Some(tools) = result.get_mut("tools").and_then(Value::as_array_mut) {
            tools.retain(|tool| {
                tool.get("name").and_then(Value::as_str) != Some("delete_inbox_note")
            });
        }
    }
    result
}
