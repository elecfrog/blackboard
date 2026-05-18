#!/usr/bin/env python3
"""Deterministic compile/check verifier for the code monitor task graph.

The graph uses this as a shell/tool node. It always exits 0 and reports
`has_issues` in stdout JSON so the TaskGraph loop can decide whether to retry
the LLM repair nodes.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any


MAX_CAPTURE_CHARS = 12000


def tail(value: str, limit: int = MAX_CAPTURE_CHARS) -> str:
    if len(value) <= limit:
        return value
    return value[-limit:]


def resolve_program(program: str) -> str:
    resolved = shutil.which(program)
    if resolved:
        return resolved

    if os.name == "nt":
        suffixes = ("", ".cmd", ".exe", ".bat")
        candidate_dirs = [
            Path.home() / "scoop/apps/nodejs/current",
            Path.home() / "scoop/apps/nodejs/current/bin",
            Path.home() / "scoop/apps/rustup/current/.cargo/bin",
            Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "nodejs",
            Path(os.environ.get("APPDATA", "")) / "npm",
        ]
        for directory in candidate_dirs:
            if not directory:
                continue
            for suffix in suffixes:
                candidate = directory / f"{program}{suffix}"
                if candidate.exists():
                    return str(candidate)

    return program


def run_check(
    label: str,
    command: list[str],
    cwd: Path,
    timeout_seconds: int,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    started = time.time()
    try:
        proc = subprocess.run(
            command,
            cwd=str(cwd),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=timeout_seconds,
            check=False,
        )
        return {
            "label": label,
            "command": command,
            "cwd": str(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": proc.returncode,
            "ok": proc.returncode == 0,
            "duration_ms": int((time.time() - started) * 1000),
            "stdout_tail": tail(proc.stdout or ""),
            "stderr_tail": tail(proc.stderr or ""),
        }
    except FileNotFoundError as exc:
        return {
            "label": label,
            "command": command,
            "cwd": str(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": None,
            "ok": False,
            "duration_ms": int((time.time() - started) * 1000),
            "error": f"command_not_found: {exc}",
            "stdout_tail": "",
            "stderr_tail": "",
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "label": label,
            "command": command,
            "cwd": str(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": None,
            "ok": False,
            "timed_out": True,
            "duration_ms": int((time.time() - started) * 1000),
            "error": f"timeout after {timeout_seconds}s",
            "stdout_tail": tail(exc.stdout or "" if isinstance(exc.stdout, str) else ""),
            "stderr_tail": tail(exc.stderr or "" if isinstance(exc.stderr, str) else ""),
        }


def cargo_env(repo_root: Path, name: str) -> dict[str, str]:
    env = os.environ.copy()
    target_dir = repo_root / ".bb_template" / "runtime" / "cargo-target" / name
    target_dir.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    return env


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--frontend-workspace", default="bb_web")
    parser.add_argument("--rust-workspace", default="bb_backend")
    parser.add_argument("--package", default="")
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--skip-frontend", action="store_true")
    parser.add_argument("--skip-rust", action="store_true")
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    checks: list[dict[str, Any]] = []

    if not args.skip_frontend:
        frontend_dir = (repo_root / args.frontend_workspace).resolve()
        if frontend_dir.exists():
            checks.append(
                run_check(
                    "frontend_build",
                    [resolve_program("npm"), "run", "build", "--prefix", str(frontend_dir)],
                    repo_root,
                    args.timeout_seconds,
                )
            )
        else:
            checks.append(
                {
                    "label": "frontend_build",
                    "ok": False,
                    "exit_code": None,
                    "error": f"frontend workspace not found: {frontend_dir}",
                    "cwd": str(repo_root),
                }
            )

    if not args.skip_rust:
        rust_dir = (repo_root / args.rust_workspace).resolve()
        if rust_dir.exists():
            cargo_args = [resolve_program("cargo"), "clippy"]
            package = args.package.strip()
            if package:
                cargo_args.extend(["-p", package])
            cargo_args.extend(["--all-targets", "--message-format", "short"])
            checks.append(
                run_check(
                    "rust_clippy",
                    cargo_args,
                    rust_dir,
                    args.timeout_seconds,
                    env=cargo_env(repo_root, "code-monitor-verify"),
                )
            )
        else:
            checks.append(
                {
                    "label": "rust_clippy",
                    "ok": False,
                    "exit_code": None,
                    "error": f"rust workspace not found: {rust_dir}",
                    "cwd": str(repo_root),
                }
            )

    issues = [check for check in checks if not check.get("ok")]
    result = {
        "ok": not issues,
        "has_issues": bool(issues),
        "issue_count": len(issues),
        "checks": checks,
        "issues": issues,
    }
    print(json.dumps(result, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
