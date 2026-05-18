#!/usr/bin/env python3
"""Audit TaskGraph agent write scopes from recorded agent-session events.

This is intentionally deterministic: LLM nodes may decide what to inspect, but
this script verifies that edit/write tool calls stayed inside declared file
ownership before the graph proceeds to compile verification.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class ScopeRule:
    node_id: str
    allow: tuple[str, ...]
    deny: tuple[str, ...]


def slash(value: str | Path) -> str:
    return str(value).replace("\\", "/").rstrip("/")


def repo_root_from_workspace(workspace_root: Path) -> Path:
    if workspace_root.name == ".bb_template":
        return workspace_root.parent
    return workspace_root


def normalize_scope_path(raw: str, repo_root: Path) -> str:
    raw = slash(raw.strip())
    repo = slash(repo_root.resolve())
    if not raw:
        return raw
    path = Path(raw)
    if path.is_absolute():
        try:
            return slash(path.resolve().relative_to(repo_root.resolve()))
        except ValueError:
            return slash(path.resolve())
    return raw.lstrip("/")


def path_matches(path: str, scope: str) -> bool:
    path = slash(path).lower()
    scope = slash(scope).lower()
    if not scope:
        return False
    if any(ch in scope for ch in "*?["):
        import fnmatch

        return fnmatch.fnmatch(path, scope)
    return path == scope or path.startswith(scope.rstrip("/") + "/")


def parse_rule(raw: str) -> ScopeRule:
    parts = raw.split("::", 2) if "::" in raw else raw.split("|", 2)
    if len(parts) != 3:
        raise ValueError(
            "rule must be 'node-id::allow1,allow2::deny1,deny2', got: " + raw
        )
    node_id, allow_raw, deny_raw = parts
    allow = tuple(item.strip() for item in allow_raw.split(",") if item.strip())
    deny = tuple(item.strip() for item in deny_raw.split(",") if item.strip())
    if not node_id.strip():
        raise ValueError("rule node id is empty")
    return ScopeRule(node_id=node_id.strip(), allow=allow, deny=deny)


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def infer_latest_run_id(workspace_root: Path, project: str, graph_id: str | None) -> str | None:
    runs_root = workspace_root / "runtime" / "task_graph_runs" / project
    if not runs_root.is_dir():
        return None
    candidates: list[tuple[float, str]] = []
    for child in runs_root.iterdir():
        if not child.is_dir():
            continue
        run_path = child / "run.json"
        if not run_path.is_file():
            continue
        try:
            run = read_json(run_path)
        except (OSError, json.JSONDecodeError):
            continue
        if graph_id and (run.get("graph_ref") or {}).get("id") != graph_id:
            continue
        if run.get("status") not in {"pending", "queued", "running", "paused"}:
            continue
        candidates.append((run_path.stat().st_mtime, child.name))
    if not candidates:
        return None
    candidates.sort(reverse=True)
    return candidates[0][1]


def load_session_id(run: dict[str, Any], run_dir: Path, node_id: str) -> str | None:
    for node in run.get("nodes") or []:
        if node.get("node_id") == node_id:
            return node.get("agent_session_id")
    node_path = run_dir / "nodes" / f"{node_id}.json"
    if node_path.is_file():
        node = read_json(node_path)
        return node.get("agent_session_id")
    return None


def iter_jsonl(path: Path):
    if not path.is_file():
        return
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                yield line_no, json.loads(line)
            except json.JSONDecodeError:
                yield line_no, {"type": "__invalid_json__", "raw": line}


def edit_paths_from_tool_use(event: dict[str, Any]) -> list[str]:
    tool = str(event.get("tool") or "").lower()
    payload = event.get("input") if isinstance(event.get("input"), dict) else {}
    paths: list[str] = []

    if tool in {"edit", "write", "multi_edit"}:
        raw_path = payload.get("path") or payload.get("file_path") or payload.get("filepath")
        if isinstance(raw_path, str):
            paths.append(raw_path)
        for key in ("edits", "files"):
            items = payload.get(key)
            if isinstance(items, list):
                for item in items:
                    if isinstance(item, dict):
                        raw_item_path = (
                            item.get("path")
                            or item.get("file_path")
                            or item.get("filepath")
                        )
                        if isinstance(raw_item_path, str):
                            paths.append(raw_item_path)

    return paths


def tool_result_error_text(event: dict[str, Any]) -> str:
    raw = event.get("error") or event.get("output") or event.get("message") or ""
    if isinstance(raw, dict):
        message = raw.get("message")
        if isinstance(message, str):
            return message
        return json.dumps(raw, ensure_ascii=False)
    return str(raw)


_WRITE_SHELL_RE = re.compile(
    r"\b(sed\s+-i|perl\s+-pi|python\s+-c|node\s+-e|Set-Content|Add-Content|Out-File|echo\s+.*>>|cat\s+>)",
    re.IGNORECASE,
)


def shell_write_violation(event: dict[str, Any], deny: tuple[str, ...]) -> str | None:
    tool = str(event.get("tool") or "").lower()
    if tool not in {"bash", "shell", "powershell", "pwsh"}:
        return None
    payload = event.get("input") if isinstance(event.get("input"), dict) else {}
    command = payload.get("command")
    if not isinstance(command, str) or not _WRITE_SHELL_RE.search(command):
        return None
    lowered = slash(command).lower()
    for denied in deny:
        if slash(denied).lower() in lowered:
            return command
    return None


def audit_rule(
    workspace_root: Path,
    repo_root: Path,
    project: str,
    run: dict[str, Any],
    run_dir: Path,
    rule: ScopeRule,
) -> dict[str, Any]:
    session_id = load_session_id(run, run_dir, rule.node_id)
    result: dict[str, Any] = {
        "node_id": rule.node_id,
        "session_id": session_id,
        "checked_edit_count": 0,
        "violations": [],
    }
    if not session_id:
        result["violations"].append(
            {"reason": "missing_agent_session_id", "node_id": rule.node_id}
        )
        return result

    events_path = (
        workspace_root
        / "runtime"
        / "agent_sessions"
        / project
        / session_id
        / "events.jsonl"
    )
    events = list(iter_jsonl(events_path) or [])
    # Build set of failed call_ids and map to their error messages
    failed_call_ids: set[str] = set()
    call_id_to_error: dict[str, str] = {}
    for _, event in events:
        if event.get("type") == "tool_result" and event.get("status") == "error":
            cid = event.get("call_id")
            if cid:
                cid_str = str(cid)
                failed_call_ids.add(cid_str)
                call_id_to_error[cid_str] = tool_result_error_text(event)

    # Keywords indicating scope/permission rejection (tool was correctly blocked)
    SCOPE_ERROR_KEYWORDS = (
        "scope",
        "permission",
        "denied",
        "forbidden",
        "not allowed",
        "access denied",
        "insufficient",
        "blocked",
        "write_roots",
        "write_deny_roots",
    )

    for line_no, event in events:
        if event.get("type") != "tool_use":
            continue
        call_id = event.get("call_id")
        if call_id and str(call_id) in failed_call_ids:
            cid_str = str(call_id)
            error_msg = call_id_to_error.get(cid_str, "").lower()
            # For edit/write tools, skip if failure was due to scope/permission rejection
            if event.get("tool") in ("edit", "write"):
                if any(kw in error_msg for kw in SCOPE_ERROR_KEYWORDS):
                    continue  # Tool was correctly blocked, no violation
            else:
                # For non-edit tools, skip failed calls (likely network/permission errors)
                continue
        for raw_path in edit_paths_from_tool_use(event):
            rel_path = normalize_scope_path(raw_path, repo_root)
            result["checked_edit_count"] += 1
            denied = [scope for scope in rule.deny if path_matches(rel_path, scope)]
            allowed = any(path_matches(rel_path, scope) for scope in rule.allow)
            if denied or not allowed:
                result["violations"].append(
                    {
                        "line": line_no,
                        "tool": event.get("tool"),
                        "path": rel_path,
                        "reason": "denied_path" if denied else "outside_allowed_scope",
                        "matched_deny": denied,
                        "allowed": list(rule.allow),
                    }
                )
        shell_command = shell_write_violation(event, rule.deny)
        if shell_command:
            result["violations"].append(
                {
                    "line": line_no,
                    "tool": event.get("tool"),
                    "reason": "shell_write_mentions_denied_path",
                    "command": shell_command,
                    "denied": list(rule.deny),
                }
            )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace-root", default=os.environ.get("BB_WORKSPACE_ROOT", "."))
    parser.add_argument("--project", default=os.environ.get("BB_DAEMON_PROJECT", "blackboard"))
    parser.add_argument("--run-id", default=os.environ.get("BB_TASK_GRAPH_RUN"))
    parser.add_argument("--graph-id", default=None)
    parser.add_argument(
        "--rule",
        action="append",
        required=True,
        help="node-id::allow1,allow2::deny1,deny2",
    )
    args = parser.parse_args()

    if not args.run_id:
        args.run_id = infer_latest_run_id(Path(args.workspace_root).resolve(), args.project, args.graph_id)
    if not args.run_id:
        print(json.dumps({"ok": False, "error": "missing run id"}, ensure_ascii=False))
        return 2

    workspace_root = Path(args.workspace_root).resolve()
    repo_root = repo_root_from_workspace(workspace_root).resolve()
    run_dir = (
        workspace_root
        / "runtime"
        / "task_graph_runs"
        / args.project
        / args.run_id
    )
    run_path = run_dir / "run.json"
    if not run_path.is_file():
        print(
            json.dumps(
                {"ok": False, "error": "run.json not found", "path": slash(run_path)},
                ensure_ascii=False,
            )
        )
        return 2

    rules = [parse_rule(raw) for raw in args.rule]
    run = read_json(run_path)
    node_results = [
        audit_rule(workspace_root, repo_root, args.project, run, run_dir, rule)
        for rule in rules
    ]
    violations = [v for result in node_results for v in result["violations"]]
    output = {
        "ok": not violations,
        "run_id": args.run_id,
        "project": args.project,
        "repo_root": slash(repo_root),
        "node_results": node_results,
        "violation_count": len(violations),
    }
    print(json.dumps(output, ensure_ascii=False, indent=2))
    return 0 if not violations else 1


if __name__ == "__main__":
    raise SystemExit(main())
