#!/usr/bin/env python3
"""Collect deterministic facts for the nightly optimization boss report.

The final LLM node should write prose, not infer basic facts. This script reads
the current run state, generated artifacts, child graph health reports, and git
diff metadata, then emits one JSON object for the report prompt.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


def slash(path: Path | str) -> str:
    return str(path).replace("\\", "/")


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def read_text(path: Path, max_chars: int) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False, "path": slash(path), "chars": 0, "excerpt": ""}
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except Exception as exc:
        return {
            "exists": True,
            "path": slash(path),
            "chars": 0,
            "error": str(exc),
            "excerpt": "",
        }
    return {
        "exists": True,
        "path": slash(path),
        "chars": len(text),
        "excerpt": text[:max_chars],
        "truncated": len(text) > max_chars,
    }


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


def parse_porcelain(status: str) -> dict[str, Any]:
    counts: dict[str, int] = {
        "added": 0,
        "modified": 0,
        "deleted": 0,
        "renamed": 0,
        "copied": 0,
        "untracked": 0,
        "other": 0,
    }
    files: list[dict[str, str]] = []
    for raw in status.splitlines():
        if not raw:
            continue
        code = raw[:2]
        path = raw[3:] if len(raw) > 3 else ""
        if code == "??":
            key = "untracked"
        elif "R" in code:
            key = "renamed"
        elif "C" in code:
            key = "copied"
        elif "A" in code:
            key = "added"
        elif "D" in code:
            key = "deleted"
        elif "M" in code:
            key = "modified"
        else:
            key = "other"
        counts[key] += 1
        files.append({"status": code.strip() or code, "path": path})
    counts["total"] = len(files)
    return {"counts": counts, "files": files[:250], "truncated": len(files) > 250}


def parse_numstat(numstat: str) -> dict[str, Any]:
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
    files.sort(
        key=lambda item: (item["insertions"] or 0) + (item["deletions"] or 0),
        reverse=True,
    )
    return {
        "file_count": len(files),
        "insertions": insertions,
        "deletions": deletions,
        "binary_count": binary_count,
        "largest_files": files[:40],
    }


def node_output(run_dir: Path, node_id: str) -> Any:
    return read_json(run_dir / "node_outputs" / f"{node_id}.json")


def artifact_path_from_output(value: Any) -> Path | None:
    if isinstance(value, dict):
        path = value.get("path")
        if isinstance(path, str) and path:
            return Path(path)
    return None


def child_run_id_from_quality(value: Any) -> str | None:
    if not isinstance(value, dict):
        return None
    stdout_json = value.get("stdout_json")
    if isinstance(stdout_json, dict):
        run_id = stdout_json.get("run_id")
        if isinstance(run_id, str) and run_id:
            return run_id
    return None


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True)
    parser.add_argument("--workspace-root", default=os.environ.get("BB_WORKSPACE_ROOT", ""))
    parser.add_argument("--project", default=os.environ.get("BB_DAEMON_PROJECT", "blackboard"))
    parser.add_argument("--parent-run-id", default=os.environ.get("BB_TASK_GRAPH_RUN", ""))
    parser.add_argument("--max-artifact-chars", type=int, default=12000)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    workspace_root = Path(args.workspace_root).resolve()
    parent_run_id = args.parent_run_id
    run_root = workspace_root / "runtime" / "task_graph_runs" / args.project
    parent_run_dir = run_root / parent_run_id if parent_run_id else Path()

    status_cmd = run_git(repo_root, ["status", "--porcelain=v1"])
    diff_stat_cmd = run_git(repo_root, ["diff", "--stat"])
    numstat_cmd = run_git(repo_root, ["diff", "--numstat"])
    staged_numstat_cmd = run_git(repo_root, ["diff", "--cached", "--numstat"])

    parent_run = read_json(parent_run_dir / "run.json") if parent_run_dir else None
    context = parent_run.get("context", {}) if isinstance(parent_run, dict) else {}
    outputs = context.get("node_outputs", {}) if isinstance(context, dict) else {}

    review_output = outputs.get("code-review")
    fix_output = outputs.get("review-scout-fix")
    quality_output = outputs.get("code-quality-check-fix")
    maintenance_output = outputs.get("inbox-ticket-maintain")
    daily_context_output = outputs.get("collect-daily-context")
    daily_brief_output = outputs.get("daily-brief")

    review_path = artifact_path_from_output(review_output)
    fix_path = artifact_path_from_output(fix_output)
    daily_brief_path = artifact_path_from_output(daily_brief_output)
    child_run_id = child_run_id_from_quality(quality_output)
    child_run_dir = run_root / child_run_id if child_run_id else Path()

    health_reports: dict[str, Any] = {}
    if child_run_dir.exists():
        for node_id in [
            "rust-health-report",
            "frontend-health-report",
            "rust-compile-verify",
            "frontend-compile-verify",
            "frontend-write-scope-audit",
        ]:
            health_reports[node_id] = node_output(child_run_dir, node_id)

    result = {
        "ok": True,
        "project": args.project,
        "repo_root": slash(repo_root),
        "workspace_root": slash(workspace_root),
        "parent_run_id": parent_run_id,
        "child_run_ids": {"code_quality": child_run_id},
        "git": {
            "status": parse_porcelain(status_cmd["stdout"]) if status_cmd["ok"] else status_cmd,
            "diff_stat": diff_stat_cmd["stdout"][:20000],
            "diff_numstat": parse_numstat(numstat_cmd["stdout"]) if numstat_cmd["ok"] else numstat_cmd,
            "staged_numstat": parse_numstat(staged_numstat_cmd["stdout"])
            if staged_numstat_cmd["ok"]
            else staged_numstat_cmd,
        },
        "artifacts": {
            "code_review": read_text(review_path, args.max_artifact_chars)
            if review_path
            else {"exists": False, "path": None, "excerpt": ""},
            "scout_fix": read_text(fix_path, args.max_artifact_chars)
            if fix_path
            else {"exists": False, "path": None, "excerpt": ""},
            "daily_brief": read_text(daily_brief_path, args.max_artifact_chars)
            if daily_brief_path
            else {"exists": False, "path": None, "excerpt": ""},
        },
        "daily": {
            "context": daily_context_output,
            "brief": daily_brief_output,
        },
        "quality": {
            "raw_output": quality_output,
            "health_reports": health_reports,
        },
        "maintenance": maintenance_output,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
