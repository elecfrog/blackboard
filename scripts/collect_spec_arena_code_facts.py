#!/usr/bin/env python3
"""Collect deterministic code facts for Spec Arena reviewers.

Issue-agnostic: callers pass `--target-files-json` (a JSON-encoded array of
repo-relative paths) and `--draft-path`; this script reports a small,
schema-stable JSON describing each path and a short head-of-file slice of the
draft. The point is to anchor reviewers in basic facts (file exists, size,
draft head) without making semantic assumptions about which issue is being
audited.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


DRAFT_HEAD_LINES_LIMIT = 80
DRAFT_HEAD_LINE_BYTES_LIMIT = 400


def file_descriptor(root: Path, rel: str) -> dict[str, object]:
    path = (root / rel)
    exists = path.exists()
    size_bytes: int | None
    if exists:
        try:
            size_bytes = path.stat().st_size if path.is_file() else None
        except OSError:
            size_bytes = None
    else:
        size_bytes = None
    return {
        "path": rel,
        "exists": exists,
        "size_bytes": size_bytes,
        "is_dir": exists and path.is_dir(),
    }


def draft_head(root: Path, rel: str) -> dict[str, object]:
    path = root / rel
    exists = path.exists() and path.is_file()
    size_bytes: int | None = None
    head_lines: list[str] = []
    if exists:
        try:
            size_bytes = path.stat().st_size
        except OSError:
            size_bytes = None
        try:
            with path.open("r", encoding="utf-8", errors="replace") as fh:
                for index, line in enumerate(fh):
                    if index >= DRAFT_HEAD_LINES_LIMIT:
                        break
                    stripped = line.rstrip("\n")
                    if len(stripped) > DRAFT_HEAD_LINE_BYTES_LIMIT:
                        stripped = stripped[:DRAFT_HEAD_LINE_BYTES_LIMIT] + "..."
                    head_lines.append(stripped)
        except OSError:
            pass
    return {
        "path": rel,
        "exists": exists,
        "size_bytes": size_bytes,
        "head_lines": head_lines,
    }


def parse_target_files(raw: str) -> list[str]:
    if not raw or not raw.strip():
        return []
    try:
        decoded = json.loads(raw)
    except json.JSONDecodeError as err:
        print(
            f"--target-files-json is not valid JSON: {err}",
            file=sys.stderr,
        )
        sys.exit(2)
    if not isinstance(decoded, list):
        print(
            "--target-files-json must decode to a JSON array of strings",
            file=sys.stderr,
        )
        sys.exit(2)
    targets: list[str] = []
    for item in decoded:
        if not isinstance(item, str) or not item.strip():
            print(
                "--target-files-json entries must be non-empty strings",
                file=sys.stderr,
            )
            sys.exit(2)
        targets.append(item.strip())
    return targets


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        required=True,
        help="Repository root used to resolve --target-files-json and --draft-path entries.",
    )
    parser.add_argument(
        "--draft-path",
        required=True,
        help="Repo-relative path to the implementation draft under audit.",
    )
    parser.add_argument(
        "--target-files-json",
        required=True,
        help="JSON array string of repo-relative paths whose existence/size should be reported.",
    )
    args = parser.parse_args()

    root = Path(args.repo_root).resolve()
    targets = parse_target_files(args.target_files_json)

    facts = {
        "schema_version": 1,
        "kind": "spec_arena_code_facts",
        "repo_root": str(root),
        "files": [file_descriptor(root, rel) for rel in targets],
        "draft_checks": draft_head(root, args.draft_path),
    }
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
