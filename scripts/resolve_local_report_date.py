#!/usr/bin/env python3
"""Resolve the nightly report date from the machine's local clock."""

from __future__ import annotations

import json
import sys
from datetime import datetime


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    now = datetime.now().astimezone()
    offset = now.strftime("%z")
    formatted_offset = f"{offset[:3]}:{offset[3:]}" if offset else ""
    result = {
        "schema_version": 1,
        "source": "machine-local-time",
        "report_date": now.date().isoformat(),
        "local_datetime": now.isoformat(timespec="seconds"),
        "timezone_name": now.tzname(),
        "utc_offset": formatted_offset,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
