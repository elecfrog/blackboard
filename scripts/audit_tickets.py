#!/usr/bin/env python3
"""Audit Blackboard JSON tickets with the generated ticket schema.

The script exits with:
  0 when every ticket and maintenance check passes,
  2 when ticket data has fixable findings,
  1 when the audit infrastructure itself failed.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


TICKET_FILE_RE = re.compile(r"^[0-9]{6}-.+\.(json|md)$")
UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
)
MAX_TAIL = 1600


def tail(text: str, limit: int = MAX_TAIL) -> str:
    if len(text) <= limit:
        return text
    return text[-limit:]


def resolve_repo_root(value: str) -> Path:
    path = Path(value).resolve()
    if (path / ".bb_template").is_dir() or (path / ".bb").is_dir():
        return path
    if path.name in {".bb_template", ".bb"}:
        return path.parent
    return path


def resolve_data_root(repo_root: Path) -> Path:
    if (repo_root / ".bb_template" / "projects").is_dir():
        return repo_root / ".bb_template"
    if (repo_root / ".bb" / "projects").is_dir():
        return repo_root / ".bb"
    return repo_root


class FindingSink:
    def __init__(self) -> None:
        self.findings: list[dict[str, Any]] = []

    def add(
        self,
        code: str,
        message: str,
        *,
        file: str | None = None,
        ticket_id: str | None = None,
        path: str | None = None,
        severity: str = "error",
    ) -> None:
        finding: dict[str, Any] = {
            "severity": severity,
            "code": code,
            "message": message,
        }
        if file:
            finding["file"] = file
        if ticket_id:
            finding["id"] = ticket_id
        if path:
            finding["path"] = path
        self.findings.append(finding)


class SchemaValidator:
    def __init__(self, schema: dict[str, Any], sink: FindingSink, file: str, ticket_id: str) -> None:
        self.schema = schema
        self.sink = sink
        self.file = file
        self.ticket_id = ticket_id

    def validate(self, value: Any, schema: dict[str, Any] | bool, path: str) -> None:
        if isinstance(schema, bool):
            if not schema:
                self.error(path, "schema forbids this value")
            return

        ref = schema.get("$ref")
        if isinstance(ref, str):
            self.validate(value, self.resolve_ref(ref), path)
            return

        if "anyOf" in schema:
            errors_before = len(self.sink.findings)
            snapshots: list[list[dict[str, Any]]] = []
            for candidate in schema["anyOf"]:
                local = FindingSink()
                validator = SchemaValidator(self.schema, local, self.file, self.ticket_id)
                validator.validate(value, candidate, path)
                if not local.findings:
                    return
                snapshots.append(local.findings)
            # Keep only a compact signal for anyOf; detailed branches are too noisy.
            self.sink.findings = self.sink.findings[:errors_before]
            self.error(path, f"value did not match anyOf ({len(snapshots)} branches)")
            return

        type_spec = schema.get("type")
        if type_spec is not None and not self.matches_type(value, type_spec):
            self.error(path, f"expected type {type_spec}, got {type(value).__name__}")
            return

        if "const" in schema and value != schema["const"]:
            self.error(path, f"value must equal const {schema['const']!r}")

        enum = schema.get("enum")
        if isinstance(enum, list) and value not in enum:
            self.error(path, f"value must be one of {enum!r}")

        if isinstance(value, dict):
            self.validate_object(value, schema, path)
        elif isinstance(value, list):
            self.validate_array(value, schema, path)
        elif isinstance(value, str):
            self.validate_string(value, schema, path)
        elif isinstance(value, (int, float)) and not isinstance(value, bool):
            self.validate_number(value, schema, path)

    def validate_object(self, value: dict[str, Any], schema: dict[str, Any], path: str) -> None:
        for field in schema.get("required", []):
            if isinstance(field, str) and field not in value:
                self.error(f"{path}.{field}", "required property is missing")

        properties = schema.get("properties")
        if not isinstance(properties, dict):
            properties = {}

        property_names = schema.get("propertyNames")
        if isinstance(property_names, dict):
            for key in value:
                self.validate_property_name(key, property_names, f"{path}.{key}")

        for field, field_schema in properties.items():
            if field in value:
                self.validate(value[field], field_schema, f"{path}.{field}")

        additional = schema.get("additionalProperties", True)
        if additional is False:
            for field in value:
                if field not in properties:
                    self.error(f"{path}.{field}", "additional property is not allowed")
        elif isinstance(additional, dict):
            for field, field_value in value.items():
                if field not in properties:
                    self.validate(field_value, additional, f"{path}.{field}")

    def validate_array(self, value: list[Any], schema: dict[str, Any], path: str) -> None:
        min_items = schema.get("minItems")
        if isinstance(min_items, int) and len(value) < min_items:
            self.error(path, f"expected at least {min_items} items")
        item_schema = schema.get("items")
        if item_schema is not None:
            for index, item in enumerate(value):
                self.validate(item, item_schema, f"{path}[{index}]")

    def validate_string(self, value: str, schema: dict[str, Any], path: str) -> None:
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            self.error(path, f"expected minLength {min_length}")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.search(pattern, value) is None:
            self.error(path, f"value does not match pattern {pattern}")
        if schema.get("format") == "uuid" and not UUID_RE.fullmatch(value):
            self.error(path, "value must be a UUID")

    def validate_number(self, value: int | float, schema: dict[str, Any], path: str) -> None:
        minimum = schema.get("minimum")
        if isinstance(minimum, (int, float)) and value < minimum:
            self.error(path, f"expected minimum {minimum}")

    def validate_property_name(self, value: str, schema: dict[str, Any], path: str) -> None:
        not_schema = schema.get("not")
        if isinstance(not_schema, dict) and "const" in not_schema and value == not_schema["const"]:
            self.error(path, f"property name {value!r} is forbidden")

    def matches_type(self, value: Any, type_spec: Any) -> bool:
        if isinstance(type_spec, list):
            return any(self.matches_type(value, item) for item in type_spec)
        if type_spec == "object":
            return isinstance(value, dict)
        if type_spec == "array":
            return isinstance(value, list)
        if type_spec == "string":
            return isinstance(value, str)
        if type_spec == "integer":
            return isinstance(value, int) and not isinstance(value, bool)
        if type_spec == "number":
            return isinstance(value, (int, float)) and not isinstance(value, bool)
        if type_spec == "boolean":
            return isinstance(value, bool)
        if type_spec == "null":
            return value is None
        return True

    def resolve_ref(self, ref: str) -> dict[str, Any] | bool:
        if not ref.startswith("#/"):
            self.error("$ref", f"unsupported ref {ref}")
            return True
        value: Any = self.schema
        for part in ref[2:].split("/"):
            if not isinstance(value, dict) or part not in value:
                self.error("$ref", f"missing ref {ref}")
                return True
            value = value[part]
        return value

    def error(self, path: str, message: str) -> None:
        self.sink.add(
            "schema",
            message,
            file=self.file,
            ticket_id=self.ticket_id,
            path=path,
        )


def check_text(value: Any, sink: FindingSink, file: str, ticket_id: str, path: str) -> None:
    if not isinstance(value, str):
        return
    if value.strip() != value:
        sink.add("text", "must not have surrounding whitespace", file=file, ticket_id=ticket_id, path=path)
    if "\0" in value or len(value) > 4000:
        sink.add("text", "contains invalid text", file=file, ticket_id=ticket_id, path=path)


def validate_business_text(ticket: dict[str, Any], sink: FindingSink, file: str, ticket_id: str) -> None:
    for key in ["id", "lane", "title", "summary", "status", "created_at", "updated_at"]:
        check_text(ticket.get(key), sink, file, ticket_id, f"$.{key}")
    for index, story in enumerate(ticket.get("stories", [])):
        if isinstance(story, dict):
            for key in ["id", "given", "when", "then", "sample"]:
                if key in story:
                    check_text(story.get(key), sink, file, ticket_id, f"$.stories[{index}].{key}")
    for index, risk in enumerate(ticket.get("risks", [])):
        if isinstance(risk, dict):
            for key in ["id", "description", "mitigation", "status"]:
                if key in risk:
                    check_text(risk.get(key), sink, file, ticket_id, f"$.risks[{index}].{key}")
    for index, record in enumerate(ticket.get("progress_record", [])):
        if isinstance(record, dict):
            for key in ["at", "summary"]:
                if key in record:
                    check_text(record.get(key), sink, file, ticket_id, f"$.progress_record[{index}].{key}")
            for evidence_index, evidence in enumerate(record.get("evidence", [])):
                check_text(
                    evidence,
                    sink,
                    file,
                    ticket_id,
                    f"$.progress_record[{index}].evidence[{evidence_index}]",
                )
    for index, attachment in enumerate(ticket.get("attachments", [])):
        if isinstance(attachment, dict):
            for key in ["kind", "target", "label", "description"]:
                if key in attachment:
                    check_text(attachment.get(key), sink, file, ticket_id, f"$.attachments[{index}].{key}")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def run_command(name: str, args: list[str], cwd: Path, env: dict[str, str] | None = None) -> dict[str, Any]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    try:
        result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False, env=process_env)
    except OSError as exc:
        return {
            "name": name,
            "status": "failed",
            "exit_code": None,
            "stdout_tail": "",
            "stderr_tail": str(exc),
            "command": args,
        }
    return {
        "name": name,
        "status": "passed" if result.returncode == 0 else "failed",
        "exit_code": result.returncode,
        "stdout_tail": tail(result.stdout),
        "stderr_tail": tail(result.stderr),
        "command": args,
    }


def skipped_command(name: str, args: list[str], reason: str) -> dict[str, Any]:
    return {
        "name": name,
        "status": "skipped_missing_tool",
        "exit_code": None,
        "stdout_tail": "",
        "stderr_tail": reason,
        "command": args,
    }


def cargo_env(data_root: Path, name: str) -> dict[str, str]:
    target = data_root / "runtime" / "cargo-target" / name
    target.mkdir(parents=True, exist_ok=True)
    return {"CARGO_TARGET_DIR": str(target)}


def is_stale_schema_result(command: dict[str, Any]) -> bool:
    text = f"{command.get('stdout_tail') or ''}\n{command.get('stderr_tail') or ''}".lower()
    return "stale schema" in text or "schema files are stale" in text


def ticket_id_from_name(path: Path) -> str | None:
    first = path.name.split("-", 1)[0]
    return first if re.fullmatch(r"^[0-9]{6}$", first) else None


def audit(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    repo_root = resolve_repo_root(args.repo_root)
    data_root = resolve_data_root(repo_root)
    project_root = data_root / "projects" / args.project
    tickets_dir = project_root / "tickets"
    schema_path = Path(args.schema).resolve() if args.schema else data_root / "schemas" / "ticket.schema.json"
    report_path = (
        Path(args.report_path).resolve()
        if args.report_path
        else data_root / "runtime" / "ticket-audit" / f"{args.project}-latest.json"
    )
    sink = FindingSink()
    commands: list[dict[str, Any]] = []

    schema: dict[str, Any] | None = None
    schema_status = "passed"
    try:
        schema = load_json(schema_path)
        if not isinstance(schema, dict):
            raise ValueError("schema root must be an object")
    except Exception as exc:
        schema_status = "failed"
        sink.add("schema_file", f"failed to read schema: {exc}", file=str(schema_path))

    if args.run_schema_check:
        if (repo_root / "bb_backend" / "Cargo.toml").is_file():
            schema_cargo_env = cargo_env(data_root, "ticket-audit-schema")
            cmd = run_command(
                "bb_schema_check",
                [
                    "cargo",
                    "run",
                    "--manifest-path",
                    "bb_backend/Cargo.toml",
                    "-p",
                    "bb_schema",
                    "--",
                    "check",
                    "--root",
                    str(data_root),
                ],
                repo_root,
                env=schema_cargo_env,
            )
            commands.append(cmd)
            if cmd["status"] != "passed" and args.run_maintenance and is_stale_schema_result(cmd):
                generate_cmd = run_command(
                    "bb_schema_generate",
                    [
                        "cargo",
                        "run",
                        "--manifest-path",
                        "bb_backend/Cargo.toml",
                        "-p",
                        "bb_schema",
                        "--",
                        "generate",
                        "--root",
                        str(data_root),
                    ],
                    repo_root,
                    env=schema_cargo_env,
                )
                commands.append(generate_cmd)
                if generate_cmd["status"] == "passed":
                    cmd = run_command(
                        "bb_schema_check_after_generate",
                        [
                            "cargo",
                            "run",
                            "--manifest-path",
                            "bb_backend/Cargo.toml",
                            "-p",
                            "bb_schema",
                            "--",
                            "check",
                            "--root",
                            str(data_root),
                        ],
                        repo_root,
                        env=schema_cargo_env,
                    )
                    commands.append(cmd)
            if cmd["status"] != "passed":
                sink.add("bb_schema_check", "generated schema is stale or command failed")
        else:
            commands.append(
                {
                    "name": "bb_schema_check",
                    "status": "not_run_no_source",
                    "exit_code": None,
                    "stdout_tail": "",
                    "stderr_tail": "bb_backend/Cargo.toml not found",
                    "command": [],
                }
            )

    json_ticket_count = 0
    legacy_markdown_count = 0
    invalid_json: list[str] = []

    if not tickets_dir.is_dir():
        sink.add("project", f"tickets directory does not exist: {tickets_dir}")
    elif schema is not None:
        for path in sorted(tickets_dir.iterdir()):
            if not path.is_file() or not TICKET_FILE_RE.fullmatch(path.name):
                continue
            rel = f"tickets/{path.name}"
            if path.suffix == ".md":
                legacy_markdown_count += 1
                sink.add("legacy_markdown", "legacy Markdown ticket remains", file=rel, ticket_id=ticket_id_from_name(path))
                continue
            json_ticket_count += 1
            ticket_id = ticket_id_from_name(path)
            try:
                ticket = load_json(path)
                if not isinstance(ticket, dict):
                    raise ValueError("ticket root must be an object")
            except Exception as exc:
                invalid_json.append(path.name)
                sink.add("json_parse", f"invalid JSON: {exc}", file=rel, ticket_id=ticket_id)
                continue
            ticket_id = str(ticket.get("id") or ticket_id or "")
            before = len(sink.findings)
            SchemaValidator(schema, sink, rel, ticket_id).validate(ticket, schema, "$")
            validate_business_text(ticket, sink, rel, ticket_id)
            if len(sink.findings) > before:
                invalid_json.append(path.name)

    index_spec_entries = 0
    index_path = project_root / "__tickets__.json"
    try:
        index = load_json(index_path)
        if isinstance(index, dict):
            index_spec_entries = sum(1 for item in index.get("tickets", []) if isinstance(item, dict) and "spec" in item)
            if index_spec_entries:
                sink.add("index", "__tickets__.json must not contain spec fields", file=str(index_path))
        else:
            sink.add("index", "__tickets__.json root must be an object", file=str(index_path))
    except Exception as exc:
        sink.add("index", f"failed to read __tickets__.json: {exc}", file=str(index_path))

    verification = {
        "schema_file": schema_status,
        "bb_schema_check": "not_run",
        "bb_schema_generate": "not_run",
        "rebuild_ticket_index": "not_run",
        "check_ticket_ids": "not_run",
        "qmd_embed": "not_run",
    }
    if args.run_schema_check and commands:
        for command in commands:
            if command["name"] == "bb_schema_generate":
                verification["bb_schema_generate"] = command["status"]
            if command["name"] in {"bb_schema_check", "bb_schema_check_after_generate"}:
                verification["bb_schema_check"] = command["status"]

    if args.run_maintenance:
        maintenance = [
            (
                "rebuild_ticket_index",
                [
                    "python3",
                    "scripts/rebuild_ticket_index.py",
                    "--project",
                    args.project,
                    "--repo-root",
                    str(repo_root),
                ],
            ),
            ("check_ticket_ids", ["python3", "scripts/check_ticket_ids.py", "--project", args.project]),
            ("qmd_embed", ["qmd", "embed"]),
        ]
        for name, command in maintenance:
            if name == "qmd_embed" and shutil.which(command[0]) is None:
                cmd = skipped_command(name, command, f"{command[0]} not found; optional embed maintenance skipped")
            else:
                cmd = run_command(name, command, repo_root)
                if name == "qmd_embed" and cmd["exit_code"] is None:
                    cmd["status"] = "skipped_missing_tool"
                    cmd["stderr_tail"] = cmd.get("stderr_tail") or "optional embed maintenance tool failed to spawn"
            commands.append(cmd)
            verification[name] = cmd["status"]
            if cmd["status"] not in {"passed", "skipped_missing_tool"}:
                sink.add(name, f"{name} failed", file=cmd.get("stderr_tail") or cmd.get("stdout_tail") or None)

    status = "passed" if not sink.findings else "failed"
    report = {
        "schema_version": 1,
        "project": args.project,
        "status": status,
        "ok": status == "passed",
        "schema_path": str(schema_path),
        "report_path": str(report_path),
        "counts": {
            "json_ticket_count": json_ticket_count,
            "legacy_markdown_count": legacy_markdown_count,
            "invalid_json_count": len(set(invalid_json)),
            "index_spec_entries": index_spec_entries,
        },
        "invalid_json": sorted(set(invalid_json)),
        "findings": sink.findings,
        "verification": verification,
        "commands": commands,
    }

    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    infrastructure_failed = verification["schema_file"] == "failed" or verification["bb_schema_check"] == "failed"
    if infrastructure_failed:
        exit_code = 1
    elif sink.findings:
        exit_code = 2
    else:
        exit_code = 0
    return report, exit_code


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Blackboard tickets against generated schema.")
    parser.add_argument("--project", required=True)
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--schema")
    parser.add_argument("--report-path")
    parser.add_argument("--run-maintenance", action="store_true")
    parser.add_argument("--run-schema-check", action="store_true")
    args = parser.parse_args()

    report, exit_code = audit(args)
    summary = {
        "ok": report["ok"],
        "status": report["status"],
        "project": report["project"],
        "report_path": report["report_path"],
        "schema_path": report["schema_path"],
        "counts": report["counts"],
        "invalid_json": report["invalid_json"],
        "verification": report["verification"],
        "findings_preview": report["findings"][:8],
        "finding_count": len(report["findings"]),
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
