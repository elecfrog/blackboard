#!/usr/bin/env python3
"""Collect deterministic context for a Rust/Vue code review TaskGraph.

The script always exits 0 and emits a ResourceBundle JSON object. Tool failures
are represented as check results so the reviewer node can still produce a useful
report instead of losing all context.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from resource_bundle_contract import apply_resource_bundle_contract


MAX_CAPTURE_CHARS = 20000
TEXT_SUFFIXES = {
    ".css",
    ".html",
    ".js",
    ".json",
    ".md",
    ".rs",
    ".scss",
    ".toml",
    ".ts",
    ".tsx",
    ".vue",
}


def slash(path: Path | str) -> str:
    return str(path).replace("\\", "/")


def truthy(value: str) -> bool:
    return value.strip().lower() in {"1", "true", "yes", "y", "on"}


def clamp_text(value: str, limit: int = MAX_CAPTURE_CHARS) -> str:
    if len(value) <= limit:
        return value
    half = max(1, limit // 2)
    return (
        value[:half]
        + f"\n...[truncated {len(value) - limit} chars]...\n"
        + value[-half:]
    )


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


def run_command(
    label: str,
    command: list[str],
    cwd: Path,
    timeout_seconds: int,
    capture_limit: int = MAX_CAPTURE_CHARS,
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
            "cwd": slash(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": proc.returncode,
            "ok": proc.returncode == 0,
            "duration_ms": int((time.time() - started) * 1000),
            "stdout": clamp_text(proc.stdout or "", capture_limit),
            "stderr": clamp_text(proc.stderr or "", capture_limit),
        }
    except FileNotFoundError as exc:
        return {
            "label": label,
            "command": command,
            "cwd": slash(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": None,
            "ok": False,
            "duration_ms": int((time.time() - started) * 1000),
            "error": f"command_not_found: {exc}",
            "stdout": "",
            "stderr": "",
        }
    except subprocess.TimeoutExpired as exc:
        stdout = exc.stdout if isinstance(exc.stdout, str) else ""
        stderr = exc.stderr if isinstance(exc.stderr, str) else ""
        return {
            "label": label,
            "command": command,
            "cwd": slash(cwd),
            "cargo_target_dir": env.get("CARGO_TARGET_DIR") if env else None,
            "exit_code": None,
            "ok": False,
            "timed_out": True,
            "duration_ms": int((time.time() - started) * 1000),
            "error": f"timeout after {timeout_seconds}s",
            "stdout": clamp_text(stdout, capture_limit),
            "stderr": clamp_text(stderr, capture_limit),
        }


def cargo_env(repo_root: Path, name: str) -> dict[str, str]:
    env = os.environ.copy()
    target_dir = repo_root / ".bb_template" / "runtime" / "cargo-target" / name
    target_dir.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    return env


def skipped_check(label: str, reason: str) -> dict[str, Any]:
    return {
        "label": label,
        "ok": True,
        "skipped": True,
        "reason": reason,
    }


def git_command(args: list[str], repo_root: Path, timeout_seconds: int = 60) -> dict[str, Any]:
    return run_command(
        f"git {' '.join(args)}",
        [resolve_program("git"), *args],
        repo_root,
        timeout_seconds,
    )


def safe_relative(path: Path, root: Path) -> str:
    try:
        return slash(path.resolve().relative_to(root.resolve()))
    except ValueError:
        return slash(path)


def read_untracked_samples(repo_root: Path, files: list[str]) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for rel in files[:20]:
        path = (repo_root / rel).resolve()
        if not path.is_file() or path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        try:
            size = path.stat().st_size
            if size > 50000:
                samples.append({"path": rel, "skipped": True, "reason": f"large file: {size} bytes"})
                continue
            samples.append(
                {
                    "path": rel,
                    "size": size,
                    "content": clamp_text(path.read_text(encoding="utf-8", errors="replace"), 20000),
                }
            )
        except OSError as exc:
            samples.append({"path": rel, "skipped": True, "reason": str(exc)})
    return samples


def package_has_script(package_json: Path, script: str) -> bool:
    try:
        data = json.loads(package_json.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return script in data.get("scripts", {})


def normalize_repo_root(path: Path) -> Path:
    resolved = path.resolve()
    if (resolved / ".git").exists():
        return resolved
    parent = resolved.parent
    if resolved.name == ".bb_template" and (parent / ".git").exists():
        return parent.resolve()
    return resolved


def collect_git_context(
    repo_root: Path,
    diff_base: str,
    max_diff_chars: int,
) -> dict[str, Any]:
    status = git_command(["status", "--short"], repo_root)
    untracked = git_command(["ls-files", "--others", "--exclude-standard"], repo_root)
    base = diff_base.strip()
    diff_args = [base, "--"] if base else ["--"]

    diff_stat = git_command(["diff", "--stat", *diff_args], repo_root)
    diff_names = git_command(["diff", "--name-only", *diff_args], repo_root)
    diff_patch = run_command(
        "git diff",
        [resolve_program("git"), "diff", "--unified=80", *diff_args],
        repo_root,
        120,
        capture_limit=max_diff_chars,
    )
    diff_check = git_command(["diff", "--check", *diff_args], repo_root)

    changed_files = [
        line.strip()
        for line in (diff_names.get("stdout") or "").splitlines()
        if line.strip()
    ]
    untracked_files = [
        line.strip()
        for line in (untracked.get("stdout") or "").splitlines()
        if line.strip()
    ]

    return {
        "status": status,
        "diff_base": base,
        "diff_stat": diff_stat,
        "diff_names": diff_names,
        "diff_patch": diff_patch,
        "diff_check": diff_check,
        "changed_files": changed_files,
        "untracked_files": untracked_files,
        "untracked_samples": read_untracked_samples(repo_root, untracked_files),
    }


def collect_static_checks(
    repo_root: Path,
    timeout_seconds: int,
    run_checks: bool,
) -> list[dict[str, Any]]:
    if not run_checks:
        return [skipped_check("static_checks", "--run-checks=false")]

    checks: list[dict[str, Any]] = []
    backend_manifest = repo_root / "bb_backend" / "Cargo.toml"
    web_package = repo_root / "bb_web" / "package.json"

    checks.append(git_command(["diff", "--check", "HEAD", "--"], repo_root))

    if backend_manifest.exists():
        rust_env = cargo_env(repo_root, "code-review-static")
        checks.append(
            run_command(
                "rust_fmt_check",
                [
                    resolve_program("cargo"),
                    "fmt",
                    "--manifest-path",
                    slash(backend_manifest),
                    "--all",
                    "--check",
                ],
                repo_root,
                timeout_seconds,
                env=rust_env,
            )
        )
        checks.append(
            run_command(
                "rust_clippy",
                [
                    resolve_program("cargo"),
                    "clippy",
                    "--manifest-path",
                    slash(backend_manifest),
                    "--all-targets",
                    "--message-format",
                    "short",
                ],
                repo_root,
                timeout_seconds,
                env=rust_env,
            )
        )
    else:
        checks.append(skipped_check("rust_fmt_check", "bb_backend/Cargo.toml not found"))
        checks.append(skipped_check("rust_clippy", "bb_backend/Cargo.toml not found"))

    if web_package.exists():
        if package_has_script(web_package, "lint"):
            checks.append(
                run_command(
                    "vue_lint",
                    [resolve_program("npm"), "run", "lint", "--prefix", slash(web_package.parent)],
                    repo_root,
                    timeout_seconds,
                )
            )
        else:
            checks.append(skipped_check("vue_lint", "bb_web/package.json has no lint script"))
        checks.append(
            run_command(
                "vue_build",
                [resolve_program("npm"), "run", "build", "--prefix", slash(web_package.parent)],
                repo_root,
                timeout_seconds,
            )
        )
    else:
        checks.append(skipped_check("vue_lint", "bb_web/package.json not found"))
        checks.append(skipped_check("vue_build", "bb_web/package.json not found"))

    return checks


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--source-root", default=".")
    parser.add_argument("--review-input", default="")
    parser.add_argument("--diff-base", default="HEAD")
    parser.add_argument("--run-checks", default="true")
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--max-diff-chars", type=int, default=60000)
    args = parser.parse_args()

    repo_root = normalize_repo_root(Path(args.repo_root))
    source_root = Path(args.source_root).resolve()
    run_checks = truthy(args.run_checks)

    git_context = collect_git_context(repo_root, args.diff_base, args.max_diff_chars)
    checks = collect_static_checks(repo_root, args.timeout_seconds, run_checks)
    failed_checks = [
        check
        for check in checks
        if not check.get("ok") and not check.get("skipped")
    ]

    bundle = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "task": {
            "type": "rust_vue_code_review",
            "input": args.review_input,
            "diff_base": args.diff_base.strip(),
        },
        "data_sources": {
            "repo_root": slash(repo_root),
            "source_root": slash(source_root),
            "git": git_context,
            "static_checks": checks,
        },
        "runtime": {
            "cwd_policy": "Agent cwd/workspace is separate from data_sources.source_root.",
            "write_policy": "Reviewer must not edit project files. Final report is written by system_write_output.",
        },
        "contracts": {
            "reviewer_output": "Markdown report with Findings first, then static checks, coverage, and residual risks.",
            "final_output_dir": "wiki/reviews",
            "source_policy": "Prefer exact file paths and line numbers from git diff, clippy, vue-tsc, or build output.",
        },
        "summary": {
            "changed_file_count": len(git_context["changed_files"]),
            "untracked_file_count": len(git_context["untracked_files"]),
            "static_check_count": len(checks),
            "failed_static_check_count": len(failed_checks),
            "failed_static_checks": [check["label"] for check in failed_checks],
        },
    }
    bundle = apply_resource_bundle_contract(bundle, "task_graph_code_review_resource_bundle")
    # Keep stdout ASCII-safe for Windows consoles that still default to GBK.
    print(json.dumps(bundle, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
