#!/usr/bin/env python3
"""Prepare Blackboard dependencies after clone or pull."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
BB_BACKEND_DIR = REPO_ROOT / "bb_backend"
BB_WEB_DIR = REPO_ROOT / "bb_web"
BB_DESKTOP_DIR = REPO_ROOT / "bb_desktop"


def npm_cmd() -> str:
    return "npm.cmd" if sys.platform == "win32" else "npm"


def run(cmd: list[str], cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess:
    print(f"  -> {' '.join(cmd)}", flush=True)
    return subprocess.run(cmd, cwd=cwd, check=True)


def _is_newer_than_marker(paths: list[Path], marker: Path) -> bool:
    if not marker.is_file():
        return True

    marker_mtime = marker.stat().st_mtime
    return any(path.is_file() and path.stat().st_mtime > marker_mtime for path in paths)


def _npm_install_needed(package_dir: Path) -> bool:
    node_modules = package_dir / "node_modules"
    install_marker = node_modules / ".package-lock.json"

    if not node_modules.is_dir():
        return True

    dependency_inputs = [
        package_dir / "package.json",
        package_dir / "package-lock.json",
    ]
    return _is_newer_than_marker(dependency_inputs, install_marker)


def ensure_web_dependencies(force: bool = False) -> None:
    if force or _npm_install_needed(BB_WEB_DIR):
        run([npm_cmd(), "install", "--no-audit", "--no-fund"], cwd=BB_WEB_DIR)
    else:
        print("  Blackboard web npm dependencies are ready", flush=True)


def ensure_desktop_dependencies(force: bool = False) -> None:
    if not BB_DESKTOP_DIR.is_dir():
        return
    if force or _npm_install_needed(BB_DESKTOP_DIR):
        run([npm_cmd(), "install", "--no-audit", "--no-fund"], cwd=BB_DESKTOP_DIR)
    else:
        print("  Blackboard desktop npm dependencies are ready", flush=True)


def ensure_backend_dependencies() -> None:
    if not BB_BACKEND_DIR.is_dir():
        raise RuntimeError(f"backend directory is missing: {BB_BACKEND_DIR}")
    run(["cargo", "fetch"], cwd=BB_BACKEND_DIR)


def setup_project(
    force_install: bool = False,
    skip_backend: bool = False,
    skip_web: bool = False,
    skip_desktop: bool = False,
) -> None:
    if not skip_backend:
        ensure_backend_dependencies()

    if not skip_web:
        ensure_web_dependencies(force=force_install)

    if not skip_desktop:
        ensure_desktop_dependencies(force=force_install)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Prepare Blackboard dependencies after clone or pull.",
    )
    parser.add_argument("--skip-backend", action="store_true", help="Skip cargo fetch.")
    parser.add_argument("--skip-web", action="store_true", help="Skip frontend npm install.")
    parser.add_argument(
        "--skip-desktop",
        action="store_true",
        help="Skip desktop npm install.",
    )
    parser.add_argument(
        "--force-install",
        action="store_true",
        help="Force npm install even if node_modules already exists.",
    )
    args = parser.parse_args()

    print("== Blackboard setup ==", flush=True)
    setup_project(
        force_install=args.force_install,
        skip_backend=args.skip_backend,
        skip_web=args.skip_web,
        skip_desktop=args.skip_desktop,
    )
    print("\nSetup complete. Run: python3 scripts/dev.py", flush=True)


if __name__ == "__main__":
    main()
