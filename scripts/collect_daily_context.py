#!/usr/bin/env python3
"""Collect deterministic repo/inbox/ticket facts for a nightly daily brief."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


OPEN_STATUSES = {"todo", "in_progress", "blocked", "review"}


def slash(path: Path | str) -> str:
    return str(path).replace("\\", "/")


def machine_local_now() -> datetime:
    return datetime.now().astimezone()


def utc_offset(value: datetime) -> str:
    offset = value.strftime("%z")
    return f"{offset[:3]}:{offset[3:]}" if offset else ""


def configure_stdio() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def resolve_repo_root(value: str) -> Path:
    path = Path(value).resolve()
    if (path / ".bb_template").is_dir() or (path / ".bb").is_dir():
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


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def run_git(repo_root: Path, args: list[str], timeout: int = 60) -> dict[str, Any]:
    try:
        proc = subprocess.run(
            ["git", *args],
            cwd=repo_root,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except Exception as exc:
        return {"ok": False, "args": args, "error": str(exc), "stdout": "", "stderr": ""}
    return {
        "ok": proc.returncode == 0,
        "args": args,
        "exit_code": proc.returncode,
        "stdout": proc.stdout,
        "stderr": proc.stderr,
    }


def parse_porcelain(status: str, limit: int) -> dict[str, Any]:
    counts: Counter[str] = Counter()
    files: list[dict[str, str]] = []
    for raw in status.splitlines():
        if not raw:
            continue
        code = raw[:2]
        path = raw[3:] if len(raw) > 3 else ""
        if code == "??":
            kind = "untracked"
        elif "R" in code:
            kind = "renamed"
        elif "C" in code:
            kind = "copied"
        elif "A" in code:
            kind = "added"
        elif "D" in code:
            kind = "deleted"
        elif "M" in code:
            kind = "modified"
        else:
            kind = "other"
        counts[kind] += 1
        files.append({"status": code.strip() or code, "path": path})
    counts["total"] = len(files)
    return {
        "counts": dict(counts),
        "files": files[:limit],
        "truncated": len(files) > limit,
    }


def parse_numstat(numstat: str, limit: int) -> dict[str, Any]:
    files: list[dict[str, Any]] = []
    insertions = 0
    deletions = 0
    binary_count = 0
    for raw in numstat.splitlines():
        parts = raw.split("\t")
        if len(parts) < 3:
            continue
        added, removed, path = parts[0], parts[1], "\t".join(parts[2:])
        binary = added == "-" or removed == "-"
        if binary:
            binary_count += 1
            add_n = del_n = None
        else:
            add_n = int(added)
            del_n = int(removed)
            insertions += add_n
            deletions += del_n
        files.append(
            {
                "path": path,
                "insertions": add_n,
                "deletions": del_n,
                "binary": binary,
            }
        )
    files.sort(key=lambda item: (item["insertions"] or 0) + (item["deletions"] or 0), reverse=True)
    return {
        "file_count": len(files),
        "insertions": insertions,
        "deletions": deletions,
        "binary_count": binary_count,
        "largest_files": files[:limit],
    }


def collect_repo(repo_root: Path, limit: int) -> dict[str, Any]:
    status = run_git(repo_root, ["status", "--porcelain=v1"])
    diff_stat = run_git(repo_root, ["diff", "--stat"])
    numstat = run_git(repo_root, ["diff", "--numstat"])
    staged_numstat = run_git(repo_root, ["diff", "--cached", "--numstat"])
    return {
        "status": parse_porcelain(status["stdout"], limit) if status["ok"] else status,
        "diff_stat": diff_stat["stdout"][:20000] if diff_stat["ok"] else diff_stat,
        "diff_numstat": parse_numstat(numstat["stdout"], limit) if numstat["ok"] else numstat,
        "staged_numstat": parse_numstat(staged_numstat["stdout"], limit)
        if staged_numstat["ok"]
        else staged_numstat,
    }


def load_inbox_document(project_root: Path, name: str) -> dict[str, Any] | None:
    path = project_root / "inbox" / name
    if not path.is_file() or path.parent != project_root / "inbox":
        return None
    value = read_json(path)
    return value if isinstance(value, dict) else None


def collect_inbox(project_root: Path, limit: int) -> dict[str, Any]:
    index = read_json(project_root / "__inbox__.json")
    notes = index.get("notes", []) if isinstance(index, dict) else []
    source_counts: Counter[str] = Counter()
    topic_counts: Counter[str] = Counter()
    ticket_refs: Counter[str] = Counter()
    previews: list[dict[str, Any]] = []

    for item in notes:
        if not isinstance(item, dict):
            continue
        source_counts[str(item.get("source") or "")] += 1
        topic_counts[str(item.get("topic") or "")] += 1
        if len(previews) >= limit:
            continue
        name = str(item.get("name") or "")
        document = load_inbox_document(project_root, name) if name else None
        related_tickets = document.get("related_tickets", []) if isinstance(document, dict) else []
        for ticket_id in related_tickets if isinstance(related_tickets, list) else []:
            if isinstance(ticket_id, str):
                ticket_refs[ticket_id] += 1
        previews.append(
            {
                "name": name,
                "title": str(item.get("title") or ""),
                "source": str(item.get("source") or ""),
                "topic": str(item.get("topic") or ""),
                "time": str(item.get("time") or ""),
                "modified_at": str(item.get("modified_at") or ""),
                "excerpt": str(item.get("excerpt") or "")[:500],
                "related_tickets": related_tickets if isinstance(related_tickets, list) else [],
                "next_step": document.get("next_step", []) if isinstance(document, dict) else [],
            }
        )

    return {
        "note_count": len(notes),
        "by_source": source_counts.most_common(12),
        "by_topic": topic_counts.most_common(20),
        "related_ticket_refs": ticket_refs.most_common(20),
        "latest_notes": previews,
        "index_loaded": isinstance(index, dict),
    }


def parse_date(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value.strip():
        return None
    text = value.strip()
    try:
        if len(text) == 10:
            return datetime.fromisoformat(text).replace(tzinfo=timezone.utc)
        return datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None


def ticket_summary(item: dict[str, Any]) -> dict[str, Any]:
    extra = item.get("extra") if isinstance(item.get("extra"), dict) else {}
    return {
        "id": str(item.get("id") or ""),
        "title": str(item.get("title") or ""),
        "status": str(item.get("status") or ""),
        "lane": str(item.get("lane") or ""),
        "updated_at": str(item.get("updated_at") or ""),
        "assignee": str(extra.get("assignee") or ""),
        "path": str(item.get("path") or ""),
    }


def collect_tickets(project_root: Path, limit: int, stale_days: int, now: datetime) -> dict[str, Any]:
    index = read_json(project_root / "__tickets__.json")
    tickets = index.get("tickets", []) if isinstance(index, dict) else []
    by_status: Counter[str] = Counter()
    by_lane: Counter[str] = Counter()
    by_assignee: Counter[str] = Counter()
    metadata_warning_count = 0
    metadata_error_count = 0
    open_tickets: list[dict[str, Any]] = []

    for item in tickets:
        if not isinstance(item, dict):
            continue
        status = str(item.get("status") or "")
        lane = str(item.get("lane") or "")
        by_status[status] += 1
        by_lane[lane] += 1
        extra = item.get("extra") if isinstance(item.get("extra"), dict) else {}
        assignee = str(extra.get("assignee") or "")
        if assignee:
            by_assignee[assignee] += 1
        if item.get("metadata_error"):
            metadata_error_count += 1
        warnings = item.get("metadata_warnings")
        if isinstance(warnings, list):
            metadata_warning_count += len(warnings)
        if status in OPEN_STATUSES:
            summary = ticket_summary(item)
            updated = parse_date(item.get("updated_at"))
            if updated:
                summary["age_days"] = max(0, (now - updated).days)
            open_tickets.append(summary)

    open_tickets.sort(key=lambda item: (item.get("age_days", -1), item.get("updated_at", "")), reverse=True)
    stale = [item for item in open_tickets if item.get("age_days", 0) >= stale_days]
    recent = sorted(open_tickets, key=lambda item: item.get("updated_at", ""), reverse=True)

    return {
        "ticket_count": len(tickets),
        "open_ticket_count": len(open_tickets),
        "by_status": dict(by_status),
        "by_lane": dict(by_lane),
        "by_assignee": by_assignee.most_common(20),
        "metadata_error_count": metadata_error_count,
        "metadata_warning_count": metadata_warning_count,
        "open_stale_tickets": stale[:limit],
        "recent_open_tickets": recent[:limit],
        "index_loaded": isinstance(index, dict),
        "current_counter": index.get("current_counter") if isinstance(index, dict) else None,
    }


def collect_run_context(workspace_root: Path, project: str) -> dict[str, Any]:
    run_id = os.environ.get("BB_TASK_GRAPH_RUN", "")
    if not run_id:
        return {"parent_run_id": "", "node_outputs": {}}
    run_path = workspace_root / "runtime" / "task_graph_runs" / project / run_id / "run.json"
    run = read_json(run_path)
    context = run.get("context", {}) if isinstance(run, dict) else {}
    outputs = context.get("node_outputs", {}) if isinstance(context, dict) else {}
    selected = {
        key: outputs.get(key)
        for key in [
            "code-review",
            "review-scout-fix",
            "code-quality-check-fix",
            "inbox-ticket-maintain",
        ]
        if key in outputs
    }
    return {
        "parent_run_id": run_id,
        "run_path": slash(run_path),
        "node_outputs": selected,
        "run_loaded": isinstance(run, dict),
    }


def main() -> int:
    configure_stdio()

    parser = argparse.ArgumentParser(description="Collect nightly repo/inbox/ticket daily context.")
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--workspace-root", default=os.environ.get("BB_WORKSPACE_ROOT", ""))
    parser.add_argument("--project", default=os.environ.get("BB_DAEMON_PROJECT", "blackboard"))
    parser.add_argument("--max-items", type=int, default=20)
    parser.add_argument("--stale-days", type=int, default=7)
    args = parser.parse_args()

    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    workspace_root = Path(args.workspace_root).resolve() if args.workspace_root else data_root
    project_root = data_root / "projects" / args.project
    limit = max(1, min(args.max_items, 50))
    now = machine_local_now()

    result = {
        "schema_version": 1,
        "generated_at": now.isoformat(timespec="seconds"),
        "time_source": "machine-local-time",
        "report_date": now.date().isoformat(),
        "timezone_name": now.tzname(),
        "utc_offset": utc_offset(now),
        "project": args.project,
        "repo_root": slash(repo_root),
        "data_root": slash(data_root),
        "workspace_root": slash(workspace_root),
        "repo": collect_repo(repo_root, limit),
        "inbox": collect_inbox(project_root, limit),
        "tickets": collect_tickets(project_root, limit, max(1, args.stale_days), now),
        "run_context": collect_run_context(workspace_root, args.project),
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
