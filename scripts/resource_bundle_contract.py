#!/usr/bin/env python3
"""Shared ResourceBundle contract metadata for TaskGraph builder scripts."""

from __future__ import annotations

import json
from copy import deepcopy
from datetime import datetime, timezone
from typing import Any


RESOURCE_BUNDLE_SCHEMA_VERSION = 1
RESOURCE_BUNDLE_VERSION = "1.0"
BUILDER_REGISTRY_VERSION = "2026-05-23"


BUILDERS: dict[str, dict[str, Any]] = {
    "inbox_cleanup_resource_bundle": {
        "id": "inbox.cleanup.v1",
        "kind": "inbox_cleanup_resource_bundle",
        "version": "1.0",
        "script": "scripts/collect_inbox_resource_bundle.py",
        "description": "Loads selected inbox notes and active ticket catalog for one inbox cleanup loop iteration.",
        "output_contract": "JSON decisions over stable note_id values; no direct file mutation by the LLM.",
    },
    "task_graph_code_review_resource_bundle": {
        "id": "code_review.rust_vue.v1",
        "kind": "task_graph_code_review_resource_bundle",
        "version": "1.0",
        "script": "scripts/collect_code_review_resource_bundle.py",
        "description": "Collects git diff, untracked samples, and optional Rust/Vue static check outputs for review.",
        "output_contract": "Findings-first Markdown review written by a system_write_output node.",
    },
    "task_graph_code_review_fix_resource_bundle": {
        "id": "code_review.scout_fix.v1",
        "kind": "task_graph_code_review_fix_resource_bundle",
        "version": "1.0",
        "script": "scripts/collect_code_review_fix_resource_bundle.py",
        "description": "Collects a review report, extracted findings, and git context for conservative scout fix.",
        "output_contract": "Markdown fix report; source edits constrained by LLM tool_policy.",
    },
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def builder_metadata(kind: str) -> dict[str, Any]:
    try:
        builder = deepcopy(BUILDERS[kind])
    except KeyError as exc:
        raise ValueError(f"unknown ResourceBundle builder kind: {kind}") from exc
    builder["registry_version"] = BUILDER_REGISTRY_VERSION
    return builder


def apply_resource_bundle_contract(bundle: dict[str, Any], kind: str) -> dict[str, Any]:
    """Normalize a domain bundle into the stable ResourceBundle envelope."""

    result = dict(bundle)
    result["schema_version"] = RESOURCE_BUNDLE_SCHEMA_VERSION
    result["resource_bundle_version"] = RESOURCE_BUNDLE_VERSION
    result["kind"] = kind
    result.setdefault("generated_at", utc_now())
    result["builder"] = builder_metadata(kind)
    result.setdefault("contracts", {})
    result.setdefault("data_sources", {})
    result.setdefault("task", {})
    return result


def registry_document() -> dict[str, Any]:
    return {
        "schema_version": RESOURCE_BUNDLE_SCHEMA_VERSION,
        "resource_bundle_version": RESOURCE_BUNDLE_VERSION,
        "registry_version": BUILDER_REGISTRY_VERSION,
        "builders": list(BUILDERS.values()),
    }


def main() -> int:
    print(json.dumps(registry_document(), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
