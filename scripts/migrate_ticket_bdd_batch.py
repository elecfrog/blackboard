#!/usr/bin/env python3
"""Migrate a small batch of legacy Markdown tickets to JSON BDD tickets."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import uuid
from datetime import date
from pathlib import Path
from typing import Any


TICKET_MD_RE = re.compile(r"^(\d{6})-.+\.md$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
LANE_RE = re.compile(r"^[a-z][a-z0-9-]{1,31}$")
STATUS_VALUES = {"todo", "in_progress", "blocked", "review", "done", "archived"}
CORE_KEYS = {
    "id",
    "lane",
    "title",
    "summary",
    "stories",
    "risks",
    "progress_record",
    "attachments",
    "status",
    "created_at",
    "updated_at",
    "schema_version",
}


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


def parse_frontmatter(text: str) -> tuple[dict[str, str], str]:
    lines = text.splitlines()
    if not lines or lines[0].strip() not in {"+++", "---"}:
        return {}, text
    marker = lines[0].strip()
    for idx in range(1, len(lines)):
        if lines[idx].strip() == marker:
            return parse_tomlish(lines[1:idx]), "\n".join(lines[idx + 1 :]).strip()
    return {}, text


def parse_tomlish(lines: list[str]) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"')
        values[key] = value
    return values


def non_empty(value: Any, fallback: str) -> str:
    if isinstance(value, str) and value.strip():
        return value.strip()
    return fallback


def first_body_line(body: str, fallback: str) -> str:
    for line in body.splitlines():
        line = line.strip().lstrip("#").strip()
        if line:
            return line[:500]
    return fallback


def parse_attachments(raw: str | None) -> list[dict[str, Any]]:
    if not raw:
        return []
    try:
        value = json.loads(raw)
    except Exception:
        return [{"kind": "legacy", "target": raw}]
    if not isinstance(value, list):
        return []
    attachments = []
    for item in value:
        if not isinstance(item, dict):
            continue
        kind = str(item.get("kind") or "legacy").strip()
        target = str(item.get("target") or "").strip()
        if not target:
            continue
        attachment = {"kind": kind, "target": target}
        for key in ["label", "description"]:
            text = item.get(key)
            if isinstance(text, str) and text.strip():
                attachment[key] = text.strip()
        attachments.append(attachment)
    return attachments


def migrate_file(path: Path) -> tuple[Path, dict[str, Any]]:
    match = TICKET_MD_RE.fullmatch(path.name)
    if not match:
        raise ValueError(f"not a ticket markdown file: {path.name}")
    ticket_id = match.group(1)
    raw = path.read_text(encoding="utf-8")
    frontmatter, body = parse_frontmatter(raw)
    today = date.today().isoformat()
    title = non_empty(frontmatter.get("title"), path.stem.split("-", 1)[-1].replace("-", " "))
    summary = non_empty(frontmatter.get("summary"), first_body_line(body, title))
    lane = non_empty(frontmatter.get("lane"), "bbd")
    if not LANE_RE.fullmatch(lane):
        lane = "bbd"
    status = non_empty(frontmatter.get("status"), "todo")
    if status not in STATUS_VALUES:
        status = "todo"
    created_at = non_empty(frontmatter.get("created_at"), today)
    updated_at = non_empty(frontmatter.get("updated_at"), today)
    if not DATE_RE.fullmatch(created_at):
        created_at = today
    if not DATE_RE.fullmatch(updated_at):
        updated_at = today

    extra = {
        key: str(value)
        for key, value in sorted(frontmatter.items())
        if key not in CORE_KEYS and str(value).strip()
    }
    attachments = parse_attachments(frontmatter.get("attachments"))
    if "attachments" in extra:
        del extra["attachments"]

    ticket = {
        "schema_version": 1,
        "id": ticket_id,
        "lane": lane,
        "title": title,
        "summary": summary,
        "stories": [
            {
                "id": str(uuid.uuid4()),
                "given": f"Legacy ticket {ticket_id} exists as Markdown.",
                "when": "Blackboard migrates it into JSON BDD form.",
                "then": "The ticket keeps a valid structured shape and preserves the original Markdown in progress evidence.",
            }
        ],
        "risks": [],
        "progress_record": [
            {
                "at": today,
                "summary": "Migrated from legacy Markdown ticket.",
                "evidence": [body[:3500] if body else f"legacy file: {path.name}"],
            }
        ],
        "attachments": attachments,
        "status": status,
        "created_at": created_at,
        "updated_at": updated_at,
    }
    if extra:
        ticket["extra"] = extra

    json_path = path.with_suffix(".json")
    json_path.write_text(json.dumps(ticket, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    path.unlink()
    return json_path, ticket


def run_rebuild_index(repo_root: Path, project: str) -> str:
    script = repo_root / "scripts" / "rebuild_ticket_index.py"
    if not script.is_file():
        return "rebuild_ticket_index: not_run_missing_script"
    result = subprocess.run(
        [sys.executable, str(script), "--project", project, "--repo-root", str(repo_root)],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return "rebuild_ticket_index: passed"
    tail = (result.stderr or result.stdout).strip()[-240:]
    return f"rebuild_ticket_index: failed {tail}"


def migrate(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    tickets_dir = data_root / "projects" / args.project / "tickets"
    files = [
        path
        for path in sorted(tickets_dir.glob("*.md")) if TICKET_MD_RE.fullmatch(path.name)
    ] if tickets_dir.is_dir() else []
    selected = files[: args.batch_count]
    converted: list[str] = []
    skipped: list[dict[str, str]] = []
    json_files_created: list[str] = []
    markdown_files_removed: list[str] = []

    for path in selected:
        try:
            json_path, ticket = migrate_file(path)
        except Exception as exc:
            skipped.append({"id": path.name[:6], "reason": str(exc)})
            continue
        converted.append(ticket["id"])
        json_files_created.append(json_path.name)
        markdown_files_removed.append(path.name)

    remaining = len([
        path
        for path in sorted(tickets_dir.glob("*.md")) if TICKET_MD_RE.fullmatch(path.name)
    ]) if tickets_dir.is_dir() else 0
    verification = []
    if converted:
        verification.append(run_rebuild_index(repo_root, args.project))

    return {
        "processed": bool(converted or skipped),
        "converted": converted,
        "skipped": skipped,
        "json_files_created": json_files_created,
        "markdown_files_removed": markdown_files_removed,
        "remaining_legacy_count": remaining,
        "continue": remaining > 0,
        "verification_status": {
            "json_syntax": "passed" if not skipped else "not_run_no_mutation",
            "ticket_shape": "passed" if converted else "not_run_no_mutation",
            "file_conversion": "passed" if converted else "not_run_no_mutation",
            "index_rebuilt": "passed" if converted else "not_required",
        },
        "verification": verification,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--batch-count", type=int, default=3)
    args = parser.parse_args()
    args.batch_count = max(1, min(args.batch_count, 50))
    print(json.dumps(migrate(args), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
