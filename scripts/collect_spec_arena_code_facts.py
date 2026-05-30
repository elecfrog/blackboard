#!/usr/bin/env python3
"""Collect deterministic code facts for Spec Arena reviewers.

The output is intentionally small JSON: enough to prevent reviewers from
inventing basic repository facts, without turning this into an implementation
analysis script.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


def read_text(root: Path, rel: str) -> str:
    path = root / rel
    if not path.exists():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def line_matches(text: str, pattern: str) -> list[dict[str, object]]:
    regex = re.compile(pattern)
    matches: list[dict[str, object]] = []
    for index, line in enumerate(text.splitlines(), start=1):
        if regex.search(line):
            matches.append({"line": index, "text": line.strip()})
    return matches


def file_exists(root: Path, rel: str) -> dict[str, object]:
    path = root / rel
    return {"path": rel, "exists": path.exists()}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True)
    parser.add_argument(
        "--draft-path",
        default=".bb_template/projects/blackboard/wiki/research/088-feishu-push-implementation-draft.md",
    )
    args = parser.parse_args()

    root = Path(args.repo_root).resolve()
    runner_config_rel = "bb_backend/crates/bb_daemon/src/http/task_graph/runner_config.rs"
    daemon_rel = "bb_backend/crates/bb_daemon/src/daemon.rs"
    bb_cli_main_rel = "bb_backend/crates/bb_cli/src/main.rs"
    bb_cli_commands_rel = "bb_backend/crates/bb_cli/src/commands"

    runner_config = read_text(root, runner_config_rel)
    daemon = read_text(root, daemon_rel)
    bb_cli_main = read_text(root, bb_cli_main_rel)
    draft = read_text(root, args.draft_path)

    facts = {
        "schema_version": 1,
        "kind": "spec_arena_code_facts",
        "files": {
            "runner_config": file_exists(root, runner_config_rel),
            "daemon_rs": file_exists(root, daemon_rel),
            "bb_cli_main": file_exists(root, bb_cli_main_rel),
            "bb_cli_commands_dir": file_exists(root, bb_cli_commands_rel),
        },
        "runner_config": {
            "path": runner_config_rel,
            "exists": bool(runner_config),
            "runner_config_struct_lines": line_matches(runner_config, r"pub struct RunnerConfig\b"),
            "deny_unknown_fields_lines": line_matches(runner_config, r"deny_unknown_fields"),
            "has_feishu_field": bool(re.search(r"\bfeishu\s*:", runner_config)),
            "has_read_env_or_toml": "read_env_or_toml" in runner_config,
            "has_read_env_bool_or_toml": "read_env_bool_or_toml" in runner_config,
        },
        "daemon": {
            "path": daemon_rel,
            "exists": bool(daemon),
            "run_graph_signature_lines": line_matches(daemon, r"^fn run_graph\("),
            "run_graph_return_lines": line_matches(daemon, r"\) -> Result<RunOutcome>"),
            "create_run_lines": line_matches(daemon, r"task_graph::create_run"),
            "execute_run_lines": line_matches(daemon, r"task_graph::execute_run"),
            "run_graph_call_lines": line_matches(daemon, r"run_graph\("),
            "run_outcome_match_lines": line_matches(
                daemon,
                r"RunOutcome::(Succeeded|Paused|Failed|Cancelled)",
            ),
        },
        "bb_cli": {
            "path": bb_cli_main_rel,
            "exists": bool(bb_cli_main),
            "commands_enum_lines": line_matches(bb_cli_main, r"enum Commands\b"),
            "has_feishu_subcommand": bool(re.search(r"\bFeishu\b|\bfeishu\b", bb_cli_main)),
            "has_test_push": "test-push" in bb_cli_main or "test_push" in bb_cli_main,
            "commands_dir_exists": (root / bb_cli_commands_rel).exists(),
        },
        "draft_checks": {
            "path": args.draft_path,
            "internal_run_outcome_occurrences": line_matches(draft, r"InternalRunOutcome"),
            "spawn_feishu_notification_occurrences": line_matches(
                draft,
                r"spawn_feishu_notification",
            ),
        },
    }
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
