#!/usr/bin/env python3
"""Deterministically consolidate high-confidence JSON inbox notes into tickets.

This script is intentionally conservative: it only processes notes that mention
exactly one active six-digit ticket id. Ambiguous notes stay in inbox.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TICKET_ID_RE = re.compile(r"(?<!\d)(\d{6})(?!\d)")
SHORT_TICKET_ID_RE = re.compile(r"(?<![A-Za-z0-9])(\d{3})(?![A-Za-z0-9])")
TICKET_FILE_RE = re.compile(r"^(\d{6})-.+\.json$")
INBOX_FILE_RE = re.compile(r"^.+\.json$")


def resolve_repo_root(value: str) -> Path:
    path = Path(value).resolve()
    if (path / ".bb_template" / "projects").is_dir() or (path / ".bb" / "projects").is_dir():
        return path
    if path.name in {".bb_template", ".bb"}:
        return path.parent
    return path


def resolve_data_root(repo_root: Path) -> Path:
    if (repo_root / ".bb_template" / "projects").is_dir():
        return repo_root / ".bb_template"
    if (repo_root / ".bb" / "projects").is_dir():
        return repo_root / ".bb"
    return repo_root


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    text = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def compact_text(text: str, limit: int = 1200) -> str:
    lines = [line.strip() for line in text.replace("\r\n", "\n").splitlines()]
    compact = "\n".join(line for line in lines if line)
    if len(compact) <= limit:
        return compact
    return compact[: limit - 3] + "..."


def first_excerpt(text: str, limit: int = 160) -> str:
    for line in text.replace("\r\n", "\n").splitlines():
        line = line.strip()
        if line:
            return line[:limit]
    return ""


def load_inbox_document(path: Path) -> dict[str, Any] | None:
    if not INBOX_FILE_RE.fullmatch(path.name):
        return None
    try:
        data = load_json(path)
    except Exception:
        return None
    if not isinstance(data, dict) or data.get("schema_version") != 1:
        return None
    return data


def inbox_note_paths(inbox_dir: Path) -> list[Path]:
    if not inbox_dir.is_dir():
        return []
    paths: list[Path] = []
    for path in sorted(inbox_dir.glob("*.json")):
        if path.is_file() and load_inbox_document(path) is not None:
            paths.append(path)
    return paths


def render_inbox_projection(document: dict[str, Any]) -> str:
    def lines(name: str) -> list[str]:
        value = document.get(name, [])
        if not isinstance(value, list):
            return []
        return [str(item).strip() for item in value if str(item).strip()]

    rendered: list[str] = [
        f"Title: {document.get('title', '')}",
        f"Time: {document.get('time', '')}",
        f"Source: {document.get('source', '')}",
        f"Project: {document.get('project', '')}",
        f"Topic: {document.get('topic', '')}",
    ]
    for label, key in [
        ("Done", "done"),
        ("Validation", "validation"),
        ("Next step", "next_step"),
        ("Related locations", "related_locations"),
        ("Related tickets", "related_tickets"),
    ]:
        items = lines(key)
        if items:
            rendered.append(f"{label}:")
            rendered.extend(f"- {item}" for item in items)
    return "\n".join(line for line in rendered if line.strip())


def ticket_id_from_name(path: Path) -> str | None:
    match = TICKET_FILE_RE.fullmatch(path.name)
    return match.group(1) if match else None


def load_active_tickets(tickets_dir: Path) -> dict[str, tuple[Path, dict[str, Any]]]:
    tickets: dict[str, tuple[Path, dict[str, Any]]] = {}
    if not tickets_dir.is_dir():
        return tickets
    for path in sorted(tickets_dir.iterdir()):
        ticket_id = ticket_id_from_name(path)
        if not ticket_id:
            continue
        try:
            data = load_json(path)
        except Exception:
            continue
        if not isinstance(data, dict):
            continue
        resolved_id = str(data.get("id") or ticket_id)
        if data.get("status") == "archived":
            continue
        tickets[resolved_id] = (path, data)
    return tickets


def ticket_ids_from_filename(name: str) -> list[str]:
    ids = set(TICKET_ID_RE.findall(name))
    for short_id in SHORT_TICKET_ID_RE.findall(name):
        ids.add(f"{int(short_id):06d}")
    return sorted(ids)


def evaluate_note(note: Path, active_tickets: dict[str, tuple[Path, dict[str, Any]]]) -> tuple[str | None, str]:
    filename_ids = ticket_ids_from_filename(note.name)
    filename_active_ids = [ticket_id for ticket_id in filename_ids if ticket_id in active_tickets]
    if len(filename_active_ids) == 1:
        return filename_active_ids[0], "matched explicit active ticket id in inbox filename"
    if len(filename_active_ids) > 1:
        return None, f"expected exactly one active ticket id in filename, found {filename_active_ids}"

    document = load_inbox_document(note)
    if document is None:
        return None, "not a valid schema_version=1 JSON inbox document"
    text = render_inbox_projection(document)
    ids = sorted(set(TICKET_ID_RE.findall(text)))
    active_ids = [ticket_id for ticket_id in ids if ticket_id in active_tickets]
    if not ids:
        if filename_ids:
            return None, f"filename ticket id tokens are not active: {filename_ids}"
        return None, "no explicit six-digit ticket id"
    if len(active_ids) != 1:
        return None, f"expected exactly one active ticket id, found {active_ids or ids}"
    return active_ids[0], "matched explicit active ticket id"


def append_note_to_ticket(
    ticket_path: Path,
    ticket: dict[str, Any],
    note: Path,
    note_text: str,
    reason: str | None = None,
) -> None:
    now = datetime.now(timezone.utc)
    ticket.setdefault("progress_record", [])
    if not isinstance(ticket["progress_record"], list):
        ticket["progress_record"] = []
    evidence = [
        f"inbox/{note.name}",
        compact_text(note_text),
    ]
    if reason:
        evidence.append(f"cleanup reason: {reason}")
    ticket["progress_record"].append(
        {
            "at": now.date().isoformat(),
            "summary": f"Consolidated inbox note `{note.name}`.",
            "evidence": evidence,
        }
    )
    ticket["updated_at"] = now.date().isoformat()
    write_json(ticket_path, ticket)


def rebuild_inbox_index(project_root: Path) -> None:
    inbox_dir = project_root / "inbox"
    notes = []
    for path in inbox_note_paths(inbox_dir):
        stat = path.stat()
        document = load_inbox_document(path)
        if document is None:
            continue
        text = render_inbox_projection(document)
        modified_at = datetime.fromtimestamp(stat.st_mtime, timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"
        )
        notes.append(
            {
                "name": path.name,
                "size": stat.st_size,
                "modified_at": modified_at,
                "excerpt": first_excerpt(text),
                "title": str(document.get("title", "")),
                "time": str(document.get("time", "")),
                "source": str(document.get("source", "")),
                "topic": str(document.get("topic", "")),
            }
        )
    write_json(project_root / "__inbox__.json", {"notes": notes})


def run_check_ticket_ids(repo_root: Path, project: str) -> str:
    script = repo_root / "scripts" / "check_ticket_ids.py"
    if not script.is_file():
        return "check_ticket_ids: not_run_missing_script"
    result = subprocess.run(
        [sys.executable, str(script), "--project", project],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return "check_ticket_ids: passed"
    tail = (result.stderr or result.stdout).strip()[-240:]
    return f"check_ticket_ids: failed {tail}"


def run_rebuild_ticket_index(repo_root: Path, project: str) -> str:
    script = repo_root / "scripts" / "rebuild_ticket_index.py"
    if not script.is_file():
        return "ticket index: not_run_missing_script"
    result = subprocess.run(
        [sys.executable, str(script), "--project", project, "--repo-root", str(repo_root)],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return "ticket index: rebuilt"
    tail = (result.stderr or result.stdout).strip()[-240:]
    return f"ticket index: failed {tail}"


def cleanup(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    project_root = data_root / "projects" / args.project
    inbox_dir = project_root / "inbox"
    tickets_dir = project_root / "tickets"

    active_tickets = load_active_tickets(tickets_dir)
    notes = inbox_note_paths(inbox_dir)
    deleted: list[str] = []
    retained: list[dict[str, str]] = []
    tickets_updated: list[str] = []

    for note in notes:
        if len(deleted) >= args.batch_count:
            break
        ticket_id, reason = evaluate_note(note, active_tickets)
        if not ticket_id:
            retained.append({"note": note.name, "reason": reason})
            continue
        ticket_path, ticket = active_tickets[ticket_id]
        note_document = load_inbox_document(note) or {}
        note_text = render_inbox_projection(note_document)
        append_note_to_ticket(ticket_path, ticket, note, note_text)
        note.unlink()
        deleted.append(note.name)
        if ticket_id not in tickets_updated:
            tickets_updated.append(ticket_id)

    active_tickets = load_active_tickets(tickets_dir)
    remaining_high_confidence = 0
    for note in inbox_note_paths(inbox_dir):
        ticket_id, _ = evaluate_note(note, active_tickets)
        if ticket_id:
            remaining_high_confidence += 1

    rebuild_inbox_index(project_root)
    verification = ["inbox index: rebuilt"]
    if deleted:
        verification.append(run_rebuild_ticket_index(repo_root, args.project))
        verification.append(run_check_ticket_ids(repo_root, args.project))

    return {
        "processed": bool(deleted),
        "deleted": deleted,
        "retained": retained[: args.retained_limit],
        "tickets_updated": tickets_updated,
        "remaining_note_count": len(inbox_note_paths(inbox_dir)),
        "remaining_high_confidence_count": remaining_high_confidence,
        "continue": remaining_high_confidence > 0,
        "verification": verification,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--batch-count", type=int, default=5)
    parser.add_argument("--retained-limit", type=int, default=20)
    args = parser.parse_args()
    args.batch_count = max(1, min(args.batch_count, 50))

    result = cleanup(args)
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
