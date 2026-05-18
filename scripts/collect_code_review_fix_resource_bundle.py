#!/usr/bin/env python3
"""Collect a ResourceBundle for code-review guided scout fixes."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from resource_bundle_contract import apply_resource_bundle_contract


def slash(path: Path) -> str:
    return str(path.resolve()).replace("\\", "/")


def normalize_repo_root(path: Path) -> Path:
    root = path.resolve()
    if (root / ".git").exists():
        return root
    if root.name == ".bb_template" and (root.parent / ".git").exists():
        return root.parent
    return root


def run_command(args: list[str], cwd: Path, timeout: int = 30) -> dict[str, Any]:
    try:
        proc = subprocess.run(
            args,
            cwd=str(cwd),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
        return {
            "command": args,
            "exit_code": proc.returncode,
            "stdout": proc.stdout[-20000:],
            "stderr": proc.stderr[-20000:],
        }
    except Exception as exc:  # noqa: BLE001 - bundle must stay best-effort
        return {
            "command": args,
            "exit_code": None,
            "stdout": "",
            "stderr": f"{type(exc).__name__}: {exc}",
        }


def resolve_report_path(raw: str, repo_root: Path, workspace_root: Path) -> Path | None:
    candidates: list[Path] = []
    path = Path(raw)
    if path.is_absolute():
        candidates.append(path)
    else:
        candidates.extend(
            [
                repo_root / path,
                workspace_root / path,
                workspace_root / "wiki" / "reviews" / path,
                repo_root / ".bb_template" / "wiki" / "reviews" / path,
            ]
        )

    for candidate in candidates:
        if candidate.exists() and candidate.is_file():
            return candidate.resolve()
    return candidates[0].resolve() if candidates else None


FINDING_HEADING = re.compile(
    r"(?im)^(?:#{2,5}\s*)?(?:finding\s*)?(?:\d+[\).:-]\s*)?"
    r"(?:\[(?P<bracket>critical|high|medium|low|info)\]|(?P<word>critical|high|medium|low|info|严重|高|中|低))"
    r"[:：\]\s-]+(?P<title>.+)$"
)


def extract_findings(report: str, limit: int = 20) -> list[dict[str, str]]:
    findings: list[dict[str, str]] = []
    matches = list(FINDING_HEADING.finditer(report))
    severity_map = {
        "严重": "critical",
        "高": "high",
        "中": "medium",
        "低": "low",
    }
    for index, match in enumerate(matches[:limit]):
        raw_severity = (match.group("bracket") or match.group("word") or "info").lower()
        severity = severity_map.get(raw_severity, raw_severity)
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(report)
        body = report[start:end].strip()
        findings.append(
            {
                "severity": severity,
                "title": match.group("title").strip(),
                "body": body[:4000],
            }
        )
    return findings


def read_text(path: Path | None, max_chars: int) -> tuple[str, str | None]:
    if not path or not path.exists():
        return "", "review report not found"
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except Exception as exc:  # noqa: BLE001
        return "", f"{type(exc).__name__}: {exc}"
    if len(text) > max_chars:
        return text[:max_chars] + "\n\n[truncated]", None
    return text, None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--source-root", default=".")
    parser.add_argument("--review-report-path", required=True)
    parser.add_argument("--fix-input", default="")
    parser.add_argument("--max-report-chars", type=int, default=50000)
    parser.add_argument("--max-findings", type=int, default=20)
    args = parser.parse_args()

    repo_root = normalize_repo_root(Path(args.repo_root))
    workspace_root = repo_root / ".bb_template"
    source_root = Path(args.source_root)
    if not source_root.is_absolute():
        source_root = repo_root / source_root
    source_root = source_root.resolve()

    report_path = resolve_report_path(args.review_report_path, repo_root, workspace_root)
    report_text, report_error = read_text(report_path, args.max_report_chars)
    findings = extract_findings(report_text, args.max_findings)

    bundle = {
        "created_at": datetime.now(timezone.utc).isoformat(),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "task": {
            "type": "code_review_scout_fix",
            "input": args.fix_input
            or "Verify the code review findings and apply only safe critical/high/medium fixes. Treat low/info/nit findings as report-only unless explicitly requested.",
        },
        "paths": {
            "repo_root": slash(repo_root),
            "workspace_root": slash(workspace_root),
            "source_root": slash(source_root),
            "review_report_path": slash(report_path) if report_path else args.review_report_path,
        },
        "review_report": {
            "error": report_error,
            "text": report_text,
            "extracted_findings": findings,
        },
        "git": {
            "status_short": run_command(["git", "status", "--short"], repo_root),
            "diff_name_only": run_command(["git", "diff", "--name-only", "HEAD", "--"], repo_root),
            "diff_stat": run_command(["git", "diff", "--stat", "HEAD", "--"], repo_root),
        },
        "contracts": {
            "review_report_is_advisory": True,
            "must_verify_finding_against_source_before_edit": True,
            "default_actionable_severities": ["critical", "high", "medium"],
            "report_only_severities": ["low", "info", "nit"],
            "low_info_nit_requires_explicit_user_request": True,
            "max_small_fixes_per_run": 3,
            "allowed_edit_root": slash(source_root),
            "forbidden_edit_globs": [
                ".gitattributes",
                ".gitignore",
                ".editorconfig",
                ".prettierrc*",
                ".eslintrc*",
                ".github/**",
                ".bb_template/**",
                ".bb_template/runtime/**",
                ".bb_template/projects/**",
                ".bb_template/wiki/reviews/**",
                ".bb_template/task_graphs/**",
                ".bb_template/skills/**",
                ".bb_template/agents/**",
                ".bb_template/.pi/**",
                ".git/**",
                "target/**",
                "node_modules/**",
                "bb_web/dist/**",
            ],
            "do_not_create_or_edit_repo_governance_files": True,
            "do_not_revert_unrelated_changes": True,
            "final_output": "Markdown summary; system_write_output persists it.",
        },
    }
    bundle = apply_resource_bundle_contract(
        bundle, "task_graph_code_review_fix_resource_bundle"
    )
    json.dump(bundle, sys.stdout, ensure_ascii=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
