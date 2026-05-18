#!/usr/bin/env python3
"""Normalize low-level LLM decision shape errors without adding semantics."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any


REQUIRED_KEYS = {"processed", "decisions", "retained", "continue"}


def configure_stdio() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def normalize(value: Any) -> dict[str, Any]:
    if isinstance(value, dict) and REQUIRED_KEYS.issubset(value.keys()):
        return {
            "processed": bool(value.get("processed")),
            "decisions": value.get("decisions") if isinstance(value.get("decisions"), list) else [],
            "retained": value.get("retained") if isinstance(value.get("retained"), list) else [],
            "continue": bool(value.get("continue")),
        }

    if isinstance(value, dict):
        note_id = str(value.get("note_id", "")).strip()
        reason = str(value.get("reason", "")).strip()
        if note_id and reason:
            # Safe repair only: a single partial object can only become retained,
            # never a delete/apply decision.
            return {
                "processed": False,
                "decisions": [],
                "retained": [{"note_id": note_id, "reason": reason}],
                "continue": False,
            }

    return {
        "processed": False,
        "decisions": [],
        "retained": [{"note_id": "note_0", "reason": "LLM output did not match decision object shape"}],
        "continue": False,
    }


def main() -> int:
    configure_stdio()

    parser = argparse.ArgumentParser()
    parser.add_argument("--decision-json", required=True)
    args = parser.parse_args()

    value = json.loads(args.decision_json)
    print(json.dumps(normalize(value), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
