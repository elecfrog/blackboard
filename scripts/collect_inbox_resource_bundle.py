#!/usr/bin/env python3
"""Load the next JSON inbox notes as a ResourceBundle for an LLM cleanup pass.

This intentionally avoids deciding where notes belong. The graph/LLM layer owns
semantic judgment; this loader only provides the concrete resources for the
current loop iteration.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from cleanup_inbox_batch import (
    inbox_note_paths,
    load_active_tickets,
    load_inbox_document,
    render_inbox_projection,
    resolve_data_root,
    resolve_repo_root,
)
from resource_bundle_contract import apply_resource_bundle_contract


def configure_stdio() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def load_note(index: int, path: Path, project_root: Path, content_limit: int) -> dict[str, Any]:
    document = load_inbox_document(path) or {}
    content = render_inbox_projection(document)
    stat = path.stat()
    return {
        "id": f"note_{index + 1}",
        "name": path.name,
        "path": f"inbox/{path.name}",
        "absolute_path": str(path.resolve()).replace("\\", "/"),
        "relative_path": str(path.relative_to(project_root)).replace("\\", "/"),
        "size": stat.st_size,
        "document": document,
        "content": content[:content_limit],
        "content_truncated": len(content) > content_limit,
    }


def ticket_entry(ticket_id: str, ticket: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": ticket_id,
        "lane": str(ticket.get("lane", "")),
        "title": str(ticket.get("title", "")),
        "summary": str(ticket.get("summary", "")),
        "status": str(ticket.get("status", "")),
        "updated_at": str(ticket.get("updated_at", "")),
    }


def build_bundle(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    project_root = data_root / "projects" / args.project
    inbox_dir = project_root / "inbox"
    active_tickets = load_active_tickets(project_root / "tickets")

    notes = inbox_note_paths(inbox_dir)
    selected_notes = notes[: args.batch_count]

    bundle = {
        "task": {
            "project": args.project,
            "goal": "Process exactly this loop iteration's JSON inbox resources. Decide which selected notes can be condensed into existing active tickets.",
            "batch_count": args.batch_count,
        },
        "data_sources": {
            "inbox": {
                "root": f"projects/{args.project}/inbox",
                "format": "schema_version=1 JSON inbox document",
                "note_count": len(notes),
                "selected_note_count": len(selected_notes),
                "remaining_unselected_count": max(0, len(notes) - len(selected_notes)),
                "notes": [
                    load_note(index, note, project_root, args.content_limit)
                    for index, note in enumerate(selected_notes)
                ],
            },
            "tickets": {
                "active_ticket_count": len(active_tickets),
                "active_tickets": [
                    ticket_entry(ticket_id, ticket)
                    for ticket_id, (_path, ticket) in sorted(active_tickets.items())
                ],
            },
        },
        "contracts": {
            "decision_limit": args.batch_count,
            "allowed_actions": [
                "append_note_to_existing_active_ticket",
                "delete_note_only_after_successful_append",
                "retain_unclear_note",
            ],
            "match_signals": [
                "ticket id in note filename or path",
                "ticket id in note JSON fields or display projection",
                "ticket title or feature name",
                "source paths, components, code areas, and project context",
            ],
            "forbidden_actions": [
                "create_ticket",
                "append_to_archived_ticket",
                "move_ticket_to_done",
                "delete_without_append",
            ],
            "output_schema": {
                "processed": "boolean",
                "decisions": [
                    {
                        "note_id": "stable note id from data_sources.inbox.notes, for example note_1",
                        "ticket_id": "six-digit active ticket id",
                        "confidence": "high",
                        "reason": "direct evidence summary",
                    }
                ],
                "retained": [
                    {
                        "note_id": "stable note id from data_sources.inbox.notes, for example note_2",
                        "reason": "why not safe to clean now",
                    }
                ],
                "continue": "boolean",
            },
        },
    }
    return apply_resource_bundle_contract(bundle, "inbox_cleanup_resource_bundle")


def main() -> int:
    configure_stdio()

    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--batch-count", type=int, default=5)
    parser.add_argument("--content-limit", type=int, default=4000)
    args = parser.parse_args()
    args.batch_count = max(1, min(args.batch_count, 50))
    args.content_limit = max(400, min(args.content_limit, 12000))

    print(json.dumps(build_bundle(args), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
