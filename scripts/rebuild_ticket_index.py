#!/usr/bin/env python3
import argparse
import json
import re
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib


CORE_KEYS = {"id", "lane", "title", "status", "created_at", "updated_at"}


def stringify(value):
    if isinstance(value, str):
        return value
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def parse_md(path: Path):
    text = path.read_text(encoding="utf-8")
    warnings = []
    if not text.startswith("+++\n"):
        return {}, warnings, "missing TOML frontmatter"
    end = text.find("\n+++\n", 4)
    if end < 0:
        return {}, warnings, "unterminated TOML frontmatter"
    try:
        fields = tomllib.loads(text[4:end])
    except tomllib.TOMLDecodeError as err:
        return {}, warnings, f"invalid TOML frontmatter: {err}"
    if "ticket_spec" not in fields:
        warnings.append("missing frontmatter field `ticket_spec`")
    return fields, warnings, None


def entry_from_json(path: Path):
    data = json.loads(path.read_text(encoding="utf-8"))
    extra = data.get("extra") if isinstance(data.get("extra"), dict) else {}
    attachments = data.get("attachments") if isinstance(data.get("attachments"), list) else []
    return {
        "name": path.name,
        "path": f"tickets/{path.name}",
        "id": str(data.get("id", "")),
        "lane": str(data.get("lane", "")),
        "title": str(data.get("title", "")),
        "status": str(data.get("status", "")),
        "created_at": str(data.get("created_at", "")),
        "updated_at": str(data.get("updated_at", "")),
        "attachments": attachments,
        "extra": {str(k): stringify(v) for k, v in sorted(extra.items())},
        "metadata_error": None,
        "metadata_warnings": [],
    }


def entry_from_md(path: Path):
    fields, warnings, error = parse_md(path)
    fallback_id = path.name.split("-", 1)[0]
    extra = {
        str(k): stringify(v)
        for k, v in sorted(fields.items())
        if k not in CORE_KEYS and k != "ticket_spec"
    }
    return {
        "name": path.name,
        "path": f"tickets/{path.name}",
        "id": str(fields.get("id", fallback_id)),
        "lane": str(fields.get("lane", "")),
        "title": str(fields.get("title", "")),
        "status": str(fields.get("status", "")),
        "created_at": str(fields.get("created_at", "")),
        "updated_at": str(fields.get("updated_at", "")),
        "attachments": [],
        "extra": extra,
        "metadata_error": error,
        "metadata_warnings": warnings,
    }


def ticket_file(path: Path) -> bool:
    return path.is_file() and re.match(r"^[0-9]{6}-.+\.(json|md)$", path.name) is not None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", default=".")
    args = parser.parse_args()

    project_root = Path(args.repo_root).resolve() / ".bb_template" / "projects" / args.project
    tickets_dir = project_root / "tickets"
    index_path = project_root / "__tickets__.json"
    if not tickets_dir.is_dir():
        print(f"missing tickets directory: {tickets_dir}", file=sys.stderr)
        return 2

    entries = []
    errors = []
    for path in sorted(tickets_dir.iterdir()):
        if not ticket_file(path):
            continue
        try:
            entry = entry_from_json(path) if path.suffix == ".json" else entry_from_md(path)
        except Exception as err:
            errors.append(f"{path.name}: {err}")
            entry = {
                "name": path.name,
                "path": f"tickets/{path.name}",
                "id": path.name.split("-", 1)[0],
                "lane": "",
                "title": "",
                "status": "",
                "created_at": "",
                "updated_at": "",
                "attachments": [],
                "extra": {},
                "metadata_error": str(err),
                "metadata_warnings": [],
            }
        entries.append(entry)

    entries.sort(key=lambda entry: (entry.get("id", ""), entry.get("path", "")))
    max_id = 0
    for entry in entries:
        ticket_id = entry.get("id", "")
        if re.match(r"^[0-9]{6}$", ticket_id):
            max_id = max(max_id, int(ticket_id))

    payload = {
        "current_counter": f"{max_id:06}" if max_id else "000000",
        "tickets": entries,
    }
    tmp = index_path.with_name(f".{index_path.name}.{Path.cwd().stat().st_ino}.tmp")
    tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    tmp.replace(index_path)
    print(json.dumps({
        "project": args.project,
        "tickets": len(entries),
        "current_counter": payload["current_counter"],
        "errors": errors,
    }, ensure_ascii=False))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
