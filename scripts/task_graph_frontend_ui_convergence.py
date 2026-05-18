#!/usr/bin/env python3
"""Deterministic scanner/validator for the frontend UI convergence graph."""

from __future__ import annotations

import argparse
import json
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_FORBIDDEN = [
    "bb-primary-command",
    "bb-secondary-command",
    "runtime-button",
]

SCAN_SUFFIXES = {".vue", ".ts", ".css"}
SKIP_DIRS = {"node_modules", "dist", ".git", ".turbo", "coverage"}


def slash(path: Path) -> str:
    return str(path).replace("\\", "/")


def repo_relative(repo_root: Path, path: Path) -> str:
    try:
        return slash(path.relative_to(repo_root))
    except ValueError:
        return slash(path)


def iter_source_files(frontend_root: Path):
    for path in frontend_root.rglob("*"):
        if not path.is_file() or path.suffix not in SCAN_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        yield path


def collect_legacy_matches(repo_root: Path, frontend_workspace: str, forbidden: list[str]) -> dict[str, Any]:
    frontend_root = (repo_root / frontend_workspace).resolve()
    src_root = frontend_root / "src"
    matches: list[dict[str, Any]] = []

    if not src_root.exists():
        return {
            "frontend_root": slash(frontend_root),
            "has_issues": True,
            "matches": [],
            "errors": [f"frontend src does not exist: {slash(src_root)}"],
        }

    for path in iter_source_files(src_root):
        text = path.read_text(encoding="utf-8", errors="replace")
        rel = repo_relative(repo_root, path)
        for line_no, line in enumerate(text.splitlines(), start=1):
            for token in forbidden:
                if token not in line:
                    continue
                if token == "bb-icon-command" and rel.endswith("components/common/BbIconCommand.vue"):
                    continue
                matches.append(
                    {
                        "token": token,
                        "path": rel,
                        "line": line_no,
                        "snippet": line.strip()[:240],
                    }
                )

    common_dir = src_root / "components" / "common"
    components = sorted(path.stem for path in common_dir.glob("Bb*.vue")) if common_dir.exists() else []
    return {
        "frontend_root": slash(frontend_root),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "has_issues": bool(matches),
        "matches": matches,
        "common_components": components,
        "recommendations": [
            "Use BbButton for repeated button skins.",
            "Use BbIconCommand for icon-only commands.",
            "Use BbActionGroup for repeated action clusters.",
            "Keep page-local CSS for placement only, not button skin/focus/hover/icon sizing.",
        ],
    }


def run_command(args: list[str], cwd: Path, timeout: int) -> dict[str, Any]:
    started = time.time()
    program = shutil.which(args[0]) or args[0]
    try:
        completed = subprocess.run(
            [program, *args[1:]],
            cwd=cwd,
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return {
            "command": args,
            "exit_code": completed.returncode,
            "duration_ms": int((time.time() - started) * 1000),
            "stdout_tail": completed.stdout[-12000:],
            "stderr_tail": completed.stderr[-12000:],
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "command": args,
            "exit_code": None,
            "timed_out": True,
            "duration_ms": int((time.time() - started) * 1000),
            "stdout_tail": (exc.stdout or "")[-12000:] if isinstance(exc.stdout, str) else "",
            "stderr_tail": (exc.stderr or "")[-12000:] if isinstance(exc.stderr, str) else "",
        }


def scan(args: argparse.Namespace) -> int:
    repo_root = Path(args.repo_root).resolve()
    payload = collect_legacy_matches(repo_root, args.frontend_workspace, args.forbidden)
    payload.update(
        {
            "mode": "scan",
            "repo_root": slash(repo_root),
            "forbidden": args.forbidden,
        }
    )
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def validate(args: argparse.Namespace) -> int:
    repo_root = Path(args.repo_root).resolve()
    scan_payload = collect_legacy_matches(repo_root, args.frontend_workspace, args.forbidden)
    commands = [
        run_command(["git", "diff", "--check", "--", f"{args.frontend_workspace}/src", f"{args.frontend_workspace}/DESIGN.md"], repo_root, 120),
        run_command(["npm", "run", "build", "--prefix", args.frontend_workspace], repo_root, args.timeout_seconds),
    ]
    has_command_issues = any(command.get("exit_code") != 0 for command in commands)
    payload = {
        "mode": "validate",
        "repo_root": slash(repo_root),
        "frontend_workspace": args.frontend_workspace,
        "scan": scan_payload,
        "commands": commands,
        "has_issues": bool(scan_payload.get("has_issues")) or has_command_issues,
    }
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="mode", required=True)

    def add_common(subparser: argparse.ArgumentParser) -> None:
        subparser.add_argument("--repo-root", required=True)
        subparser.add_argument("--frontend-workspace", default="bb_web")
        subparser.add_argument("--forbidden", action="append", default=list(DEFAULT_FORBIDDEN))

    scan_parser = subparsers.add_parser("scan")
    add_common(scan_parser)
    scan_parser.set_defaults(func=scan)

    validate_parser = subparsers.add_parser("validate")
    add_common(validate_parser)
    validate_parser.add_argument("--timeout-seconds", type=int, default=900)
    validate_parser.set_defaults(func=validate)
    return parser


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
