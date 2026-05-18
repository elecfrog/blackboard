#!/usr/bin/env python3
"""Apply deterministic safe repairs for findings produced by audit_tickets.py."""

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


UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
)
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
LANE_RE = re.compile(r"^[a-z][a-z0-9-]{1,31}$")
RISK_ID_RE = re.compile(r"^[a-z0-9][a-z0-9_-]{0,79}$")
STATUS_VALUES = {"todo", "in_progress", "blocked", "review", "done", "archived"}
CORE_KEYS = {
    "schema_version",
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
    "extra",
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


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    text = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def normalize_string(value: Any) -> Any:
    if isinstance(value, str):
        return value.strip()
    return value


def trim_strings(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: trim_strings(val) for key, val in value.items()}
    if isinstance(value, list):
        return [trim_strings(item) for item in value]
    return normalize_string(value)


def optional_text(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def compact_extra(extra: Any) -> dict[str, str]:
    if not isinstance(extra, dict):
        return {}
    normalized: dict[str, str] = {}
    for key, value in extra.items():
        if key == "attachments":
            continue
        if isinstance(value, str):
            text = value.strip()
        else:
            text = json.dumps(value, ensure_ascii=False, sort_keys=True)
        if text:
            normalized[str(key)] = text
    return normalized


def parse_extra_attachments(extra: Any) -> list[dict[str, Any]]:
    if not isinstance(extra, dict) or "attachments" not in extra:
        return []
    raw = extra.get("attachments")
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except Exception:
            raw = [{"kind": "legacy", "target": raw}]
    if not isinstance(raw, list):
        return []
    attachments = []
    for item in raw:
        if not isinstance(item, dict):
            continue
        target = optional_text(item.get("target"))
        if not target:
            continue
        attachment = {"kind": optional_text(item.get("kind")) or "legacy", "target": target}
        for key in ["label", "description"]:
            text = optional_text(item.get(key))
            if text:
                attachment[key] = text
        attachments.append(attachment)
    return attachments


def ensure_story(ticket: dict[str, Any]) -> None:
    if not isinstance(ticket.get("stories"), list):
        ticket["stories"] = []
    if not ticket["stories"]:
        ticket["stories"].append(
            {
                "id": str(uuid.uuid4()),
                "given": f"Ticket {ticket.get('id', 'unknown')} exists.",
                "when": "A user or system inspects the ticket.",
                "then": "The ticket exposes a valid current behavior definition.",
            }
        )
    for story in ticket["stories"]:
        if not isinstance(story, dict):
            continue
        if not UUID_RE.fullmatch(str(story.get("id") or "")):
            story["id"] = str(uuid.uuid4())
        for key in ["given", "when", "then"]:
            if not optional_text(story.get(key)):
                story[key] = f"Ticket {ticket.get('id', 'unknown')} {key} is preserved for human refinement."
        if "sample" in story and optional_text(story.get("sample")) is None:
            story.pop("sample", None)


def ensure_risks(ticket: dict[str, Any]) -> None:
    if not isinstance(ticket.get("risks"), list):
        ticket["risks"] = []
    normalized = []
    for idx, risk in enumerate(ticket["risks"]):
        if not isinstance(risk, dict):
            continue
        risk_id = optional_text(risk.get("id")) or f"risk-{idx + 1}"
        if not RISK_ID_RE.fullmatch(risk_id):
            risk_id = f"risk-{idx + 1}"
        description = optional_text(risk.get("description"))
        if not description:
            continue
        item = {"id": risk_id, "description": description}
        for key in ["mitigation", "status"]:
            text = optional_text(risk.get(key))
            if text:
                item[key] = text
        normalized.append(item)
    ticket["risks"] = normalized


def ensure_progress(ticket: dict[str, Any]) -> None:
    if not isinstance(ticket.get("progress_record"), list):
        ticket["progress_record"] = []
    normalized = []
    for record in ticket["progress_record"]:
        if not isinstance(record, dict):
            continue
        summary = optional_text(record.get("summary"))
        if not summary:
            continue
        item: dict[str, Any] = {"summary": summary}
        at = optional_text(record.get("at"))
        if at:
            item["at"] = at
        evidence = record.get("evidence")
        if isinstance(evidence, list):
            item["evidence"] = [str(entry).strip() for entry in evidence if str(entry).strip()]
        normalized.append(item)
    ticket["progress_record"] = normalized


def ensure_attachments(ticket: dict[str, Any], extra_attachments: list[dict[str, Any]]) -> None:
    existing = ticket.get("attachments")
    if not isinstance(existing, list):
        existing = []
    normalized = []
    for item in [*existing, *extra_attachments]:
        if not isinstance(item, dict):
            continue
        target = optional_text(item.get("target"))
        if not target:
            continue
        attachment = {"kind": optional_text(item.get("kind")) or "legacy", "target": target}
        for key in ["label", "description"]:
            text = optional_text(item.get(key))
            if text:
                attachment[key] = text
        normalized.append(attachment)
    ticket["attachments"] = normalized


def repair_ticket(ticket: dict[str, Any], ticket_id: str) -> tuple[dict[str, Any], list[str]]:
    original = json.dumps(ticket, ensure_ascii=False, sort_keys=True)
    ticket = trim_strings(ticket)
    today = date.today().isoformat()

    extra_attachments = parse_extra_attachments(ticket.get("extra"))
    extra = compact_extra(ticket.get("extra"))

    title = optional_text(ticket.get("title")) or f"Ticket {ticket_id}"
    summary = optional_text(ticket.get("summary")) or title
    lane = optional_text(ticket.get("lane")) or "bbd"
    if not LANE_RE.fullmatch(lane):
        lane = "bbd"
    status = optional_text(ticket.get("status")) or "todo"
    if status not in STATUS_VALUES:
        status = "todo"
    created_at = optional_text(ticket.get("created_at")) or today
    updated_at = optional_text(ticket.get("updated_at")) or today
    if not DATE_RE.fullmatch(created_at):
        created_at = today
    if not DATE_RE.fullmatch(updated_at):
        updated_at = today

    repaired = {key: value for key, value in ticket.items() if key in CORE_KEYS}
    repaired.update(
        {
            "schema_version": 1,
            "id": ticket_id,
            "lane": lane,
            "title": title,
            "summary": summary,
            "status": status,
            "created_at": created_at,
            "updated_at": updated_at,
        }
    )
    if extra:
        repaired["extra"] = extra
    else:
        repaired.pop("extra", None)

    ensure_story(repaired)
    ensure_risks(repaired)
    ensure_progress(repaired)
    ensure_attachments(repaired, extra_attachments)

    changed = []
    updated = json.dumps(repaired, ensure_ascii=False, sort_keys=True)
    if updated != original:
        changed.append("safe_schema_normalization")
    return repaired, changed


def ticket_path_from_finding(data_root: Path, project: str, finding: dict[str, Any]) -> Path | None:
    file_value = finding.get("file")
    if not isinstance(file_value, str) or not file_value.endswith(".json"):
        return None
    path = data_root / "projects" / project / file_value
    if path.is_file():
        return path
    return None


def run_rebuild_index(repo_root: Path, project: str) -> None:
    script = repo_root / "scripts" / "rebuild_ticket_index.py"
    if script.is_file():
        subprocess.run(
            [sys.executable, str(script), "--project", project, "--repo-root", str(repo_root)],
            cwd=repo_root,
            text=True,
            capture_output=True,
            check=False,
        )


def repair(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    report_path = Path(args.report_path).resolve()
    report = load_json(report_path)
    findings = report.get("findings", []) if isinstance(report, dict) else []
    if not isinstance(findings, list):
        findings = []

    changed = []
    skipped = []
    paths = []
    for finding in findings:
        if not isinstance(finding, dict):
            continue
        path = ticket_path_from_finding(data_root, args.project, finding)
        if path and path not in paths:
            paths.append(path)

    for path in paths:
        try:
            ticket = load_json(path)
            if not isinstance(ticket, dict):
                raise ValueError("ticket root is not an object")
            ticket_id = str(ticket.get("id") or path.name[:6])
            repaired, fields = repair_ticket(ticket, ticket_id)
            if fields:
                write_json(path, repaired)
                changed.append({"id": ticket_id, "fields": fields, "reason": "deterministic safe schema repair"})
        except Exception as exc:
            skipped.append({"id": path.name[:6], "reason": str(exc)})

    if changed:
        run_rebuild_index(repo_root, args.project)

    status_before = "passed" if report.get("status") == "passed" else "failed"
    return {
        "summary": "deterministic repair applied" if changed else "no changes needed",
        "changed": changed,
        "skipped": skipped,
        "ticket_validation": {
            "audit_status_before": status_before,
            "finding_count_before": len(findings),
            "report_path": str(report_path),
        },
        "verification": {
            "audit_report_consumed": "passed",
            "repair_attempted": "passed" if changed else "not_needed",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--report-path", required=True)
    args = parser.parse_args()
    print(json.dumps(repair(args), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
