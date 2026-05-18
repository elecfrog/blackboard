use serde_json::{json, Value};

use super::current_tool_profile;

fn ticket_spec_schema() -> Value {
    json!({
        "type": "object",
        "description": "Strict BDD ticket definition. Title stays in the top-level ticket title field; this object owns summary, stories, risks, and progress_record.",
        "properties": {
            "summary": { "type": "string", "minLength": 1 },
            "stories": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$", "description": "Optional on input. The backend generates it when omitted; LLMs should preserve existing ids and should not invent semantic ids." },
                        "given": { "type": "string", "minLength": 1 },
                        "when": { "type": "string", "minLength": 1 },
                        "then": { "type": "string", "minLength": 1 },
                        "sample": { "type": "string", "minLength": 1, "description": "Optional local example showing how this story should be filled; guidance only, not acceptance truth." }
                    },
                    "required": ["given", "when", "then"],
                    "additionalProperties": false
                }
            },
            "risks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "pattern": "^[a-z0-9][a-z0-9_-]{0,79}$" },
                        "description": { "type": "string", "minLength": 1 },
                        "mitigation": { "type": "string", "minLength": 1 },
                        "status": { "type": "string", "minLength": 1 }
                    },
                    "required": ["id", "description"],
                    "additionalProperties": false
                }
            },
            "progress_record": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "at": { "type": "string", "minLength": 1 },
                        "summary": { "type": "string", "minLength": 1 },
                        "evidence": { "type": "array", "items": { "type": "string", "minLength": 1 } }
                    },
                    "required": ["summary"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["summary", "stories", "risks", "progress_record"],
        "additionalProperties": false
    })
}

fn ticket_attachment_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "minLength": 1 },
            "target": { "type": "string", "minLength": 1 },
            "label": { "type": "string", "minLength": 1 },
            "description": { "type": "string", "minLength": 1 }
        },
        "required": ["kind", "target"],
        "additionalProperties": false
    })
}

pub fn tools_list() -> Value {
    let project_field = json!({
        "type": "string",
        "pattern": "^[^./\\\\][^./\\\\]{0,63}$",
        "description": "Target project name from the Blackboard workspace registry."
    });
    let ticket_spec_field = ticket_spec_schema();
    let ticket_attachment_field = ticket_attachment_schema();
    let ticket_extra_field = json!({
        "type": "object",
        "propertyNames": {
            "not": { "const": "attachments" }
        },
        "additionalProperties": { "type": "string" }
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
                "description": "List schema_version=1 JSON inbox document names under the project's inbox.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field },
                    "required": ["project"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_inbox_note",
                "description": "Read one schema_version=1 JSON inbox document from the project's inbox by filename.",
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
                "description": "Delete one JSON inbox document from the project's inbox by filename after its facts have been reflected in ticket records. Daemon-backed typed tool; agents must prefer this over shell/file removal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "name": { "type": "string" },
                        "reason": { "type": "string", "minLength": 1 }
                    },
                    "required": ["project", "name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "archive_inbox_note",
                "description": "Archive one JSON inbox document by moving it to inbox/archive after it no longer needs active inbox attention. Daemon-backed typed tool; agents must prefer this over shell/file moves.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "name": { "type": "string" },
                        "reason": { "type": "string", "minLength": 1 }
                    },
                    "required": ["project", "name", "reason"],
                    "additionalProperties": false
                }
            },
            {
                "name": "list_tickets",
                "description": "List ticket files under the project's tickets/. New tickets are JSON; legacy Markdown tickets are listed only as migration inputs. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field },
                    "required": ["project"],
                    "additionalProperties": false
                }
            },
            {
                "name": "read_ticket",
                "description": "Read one ticket file by filename within a project. Read-only.",
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
                "description": "Read one ticket by strict six-digit ID within a project. Read-only.",
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
                "description": "Create one JSON BDD ticket from structured fields under the specified project. Allocates the next project-local ID from existing tickets.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": project_field,
                        "lane": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,31}$" },
                        "title": { "type": "string", "minLength": 1 },
                        "status": { "type": "string", "enum": ["todo", "in_progress", "blocked", "review", "done", "archived"] },
                        "spec": ticket_spec_field,
                        "attachments": { "type": "array", "items": ticket_attachment_field },
                        "slug": { "type": "string" },
                        "extra": ticket_extra_field,
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
                    "required": ["project", "lane", "title", "status", "spec", "attachments"],
                    "additionalProperties": false
                }
            },
            {
                "name": "update_ticket",
                "description": "Update controlled ticket fields by six-digit ID within a project. For legacy Markdown tickets, setting frontmatter.spec migrates the ticket to JSON.",
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
                                "spec": ticket_spec_field,
                                "attachments": { "type": "array", "items": ticket_attachment_field },
                                "extra": ticket_extra_field,
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
                "description": "Create a slim JSON inbox handoff and append completion/validation bullets to the ticket.",
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
                "description": "Search direct JSON inbox documents under a project's inbox using the system display projection. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field, "query": { "type": "string", "minLength": 1 } },
                    "required": ["project", "query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "search_tickets",
                "description": "Search direct ticket files under a project's tickets/. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "project": project_field, "query": { "type": "string", "minLength": 1 } },
                    "required": ["project", "query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "create_inbox_note",
                "description": "Create one schema_version=1 JSON inbox handoff document under the project's inbox.",
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
                        "related_locations": { "type": "array", "items": { "type": "string" } },
                        "related_tickets": { "type": "array", "items": { "type": "string" } },
                        "attachments": { "type": "array", "items": ticket_attachment_field },
                        "extra": {
                            "type": "object",
                            "additionalProperties": { "type": "string" }
                        }
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
                    "properties": { "id": { "type": "string", "enum": ["codex", "codebuddy", "opencode", "pi"] } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "disconnect_agent_connector",
                "description": "Delete the Agent connector target file(s).",
                "inputSchema": {
                    "type": "object",
                    "properties": { "id": { "type": "string", "enum": ["codex", "codebuddy", "opencode", "pi"] } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            }
        ]
    });
    let profile = current_tool_profile();
    if let Some(tools) = result.get_mut("tools").and_then(Value::as_array_mut) {
        tools.retain(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| profile.allows(name))
        });
    }
    result
}
