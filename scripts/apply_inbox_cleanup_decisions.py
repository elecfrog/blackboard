#!/usr/bin/env python3
"""Apply LLM JSON inbox cleanup decisions with deterministic safety checks."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

from cleanup_inbox_batch import (
    append_note_to_ticket,
    inbox_note_paths,
    load_active_tickets,
    load_inbox_document,
    rebuild_inbox_index,
    render_inbox_projection,
    resolve_data_root,
    resolve_repo_root,
    run_check_ticket_ids,
    run_rebuild_ticket_index,
)


def configure_stdio() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def load_decision_json(raw: str) -> dict[str, Any]:
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError("decision JSON must be an object")
    return value


def load_resource_bundle(raw: str) -> dict[str, Any]:
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError("resource bundle JSON must be an object")
    return value


def load_json_arg_or_node_output(
    *,
    raw: str | None,
    repo_root: Path,
    project: str,
    run_id: str | None,
    node_id: str | None,
    field: str,
) -> dict[str, Any]:
    if raw:
        value = json.loads(raw)
    else:
        if not run_id or not node_id:
            raise ValueError(f"{field} requires raw JSON or a node output reference")
        output_path = (
            repo_root
            / ".bb_template"
            / "runtime"
            / "task_graph_runs"
            / project
            / run_id
            / "node_outputs"
            / f"{node_id}.json"
        )
        value = json.loads(output_path.read_text(encoding="utf-8"))

    if not isinstance(value, dict):
        raise ValueError(f"{field} JSON must be an object")
    return value


def extract_decision(value: dict[str, Any]) -> dict[str, Any]:
    data = value.get("data")
    if isinstance(data, dict):
        return data
    return value


def extract_resource_bundle(value: dict[str, Any]) -> dict[str, Any]:
    stdout_json = value.get("stdout_json")
    if isinstance(stdout_json, dict):
        return stdout_json
    return value


def bundle_notes_by_id(bundle: dict[str, Any]) -> dict[str, str]:
    inbox = (
        bundle.get("data_sources", {})
        .get("inbox", {})
        if isinstance(bundle.get("data_sources"), dict)
        else {}
    )
    notes = inbox.get("notes", []) if isinstance(inbox, dict) else []
    result: dict[str, str] = {}
    for note in notes:
        if not isinstance(note, dict):
            continue
        note_id = str(note.get("id", "")).strip()
        name = str(note.get("name", "")).strip()
        if note_id and name:
            result[note_id] = name
    return result


def run_qmd_embed(repo_root: Path) -> str:
    try:
        result = subprocess.run(
            ["qmd", "embed"],
            cwd=repo_root,
            text=True,
            capture_output=True,
            check=False,
        )
    except FileNotFoundError:
        return "qmd embed: skipped, qmd command not found"
    if result.returncode == 0:
        return "qmd embed: passed"
    tail = (result.stderr or result.stdout).strip()[-240:]
    return f"qmd embed: failed {tail}"


def apply(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    project_root = data_root / "projects" / args.project
    inbox_dir = project_root / "inbox"
    tickets_dir = project_root / "tickets"

    decision = extract_decision(
        load_json_arg_or_node_output(
            raw=args.decisions_json,
            repo_root=repo_root,
            project=args.project,
            run_id=args.run_id or os.environ.get("BB_TASK_GRAPH_RUN"),
            node_id=args.decisions_node_output,
            field="decision",
        )
    )
    bundle = extract_resource_bundle(
        load_json_arg_or_node_output(
            raw=args.resource_bundle_json,
            repo_root=repo_root,
            project=args.project,
            run_id=args.run_id or os.environ.get("BB_TASK_GRAPH_RUN"),
            node_id=args.resource_bundle_node_output,
            field="resource bundle",
        )
    )
    note_names_by_id = bundle_notes_by_id(bundle)
    active_tickets = load_active_tickets(tickets_dir)

    deleted: list[str] = []
    tickets_updated: list[str] = []
    rejected: list[dict[str, str]] = []

    for item in decision.get("decisions", []):
        if len(deleted) >= args.batch_count:
            break
        if not isinstance(item, dict):
            rejected.append({"note": "", "reason": "decision item must be an object"})
            continue
        note_id = str(item.get("note_id", "")).strip()
        note_name = note_names_by_id.get(note_id) or str(item.get("note", "")).strip()
        ticket_id = str(item.get("ticket_id", "")).strip()
        reason = str(item.get("reason", "")).strip()
        confidence = str(item.get("confidence", "")).strip()

        if confidence != "high":
            rejected.append({"note": note_name or note_id, "reason": "confidence is not high"})
            continue
        if ticket_id not in active_tickets:
            rejected.append({"note": note_name or note_id, "reason": f"ticket {ticket_id} is not active"})
            continue
        if not note_name:
            rejected.append({"note": note_id, "reason": "decision did not reference a known note_id"})
            continue
        note_path = inbox_dir / note_name
        if note_path.parent != inbox_dir or not note_path.is_file():
            rejected.append({"note": note_name, "reason": "note file does not exist in inbox"})
            continue
        note_document = load_inbox_document(note_path)
        if note_document is None:
            rejected.append({"note": note_name, "reason": "note is not a valid schema_version=1 JSON inbox document"})
            continue

        ticket_path, ticket = active_tickets[ticket_id]
        note_text = render_inbox_projection(note_document)
        append_note_to_ticket(ticket_path, ticket, note_path, note_text, reason=reason)
        note_path.unlink()
        deleted.append(note_name)
        if ticket_id not in tickets_updated:
            tickets_updated.append(ticket_id)

    rebuild_inbox_index(project_root)
    verification = ["inbox index: rebuilt"]
    if deleted:
        verification.append(run_rebuild_ticket_index(repo_root, args.project))
        verification.append(run_check_ticket_ids(repo_root, args.project))
        verification.append(run_qmd_embed(repo_root))

    remaining_note_count = len(inbox_note_paths(inbox_dir))
    # Keep the graph moving only when this pass made progress. That avoids
    # infinite loops if the LLM asks to continue but the executor rejects all work.
    continue_signal = bool(deleted) and bool(decision.get("continue"))

    return {
        "processed": bool(deleted),
        "deleted": deleted,
        "retained": decision.get("retained", []),
        "rejected": rejected,
        "tickets_updated": tickets_updated,
        "remaining_note_count": remaining_note_count,
        "continue": continue_signal,
        "verification": verification,
    }


def main() -> int:
    configure_stdio()

    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--batch-count", type=int, default=5)
    parser.add_argument("--decisions-json")
    parser.add_argument("--resource-bundle-json")
    parser.add_argument("--decisions-node-output")
    parser.add_argument("--resource-bundle-node-output")
    parser.add_argument("--run-id")
    args = parser.parse_args()
    args.batch_count = max(1, min(args.batch_count, 50))

    print(json.dumps(apply(args), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
