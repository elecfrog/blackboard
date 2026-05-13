#!/usr/bin/env python3
"""Validate Blackboard ticket IDs, lanes, counters, and dependencies."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
if (REPO_ROOT / ".bb_template" / "projects").is_dir():
    DATA_ROOT = REPO_ROOT / ".bb_template"
elif (REPO_ROOT / ".bb" / "projects").is_dir():
    DATA_ROOT = REPO_ROOT / ".bb"
else:
    DATA_ROOT = REPO_ROOT
PROJECTS_DIR = DATA_ROOT / "projects"
PROJECT_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
LANE_RE = re.compile(r"^[a-z][a-z0-9-]{1,31}$")
TICKET_ID_RE = re.compile(r"^\d{6}$")
CROSS_PROJECT_DEP_RE = re.compile(r"^([a-z0-9][a-z0-9-]{0,63})/(\d{6})$")
TICKET_STATUSES = {"todo", "in_progress", "blocked", "review", "done", "archived"}


@dataclass(frozen=True)
class Ticket:
    project: str
    path: Path
    id: str
    lane: str
    dependencies: tuple[str, ...]


class Validator:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.project_tickets: dict[str, dict[str, Ticket]] = {}

    def error(self, message: str) -> None:
        self.errors.append(message)
        print(message, file=sys.stderr)

    def validate_project_name(self, project: str) -> bool:
        if PROJECT_RE.fullmatch(project):
            return True
        self.error(f"Invalid project name: {project}")
        return False

    def parse_frontmatter(self, path: Path, project: str) -> dict[str, object]:
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as exc:
            self.error(f"[{project}] Failed to read {path}: {exc}")
            return {}

        if not content.startswith("+++\n"):
            self.error(f"[{project}] Missing frontmatter in {path}")
            return {}

        end = content.find("\n+++", 4)
        if end == -1:
            self.error(f"[{project}] Unterminated frontmatter in {path}")
            return {}

        raw = content[4:end].strip()
        try:
            parsed = tomllib.loads(raw)
        except tomllib.TOMLDecodeError as exc:
            self.error(f"[{project}] Invalid TOML frontmatter in {path}: {exc}")
            return {}

        return parsed

    def active_lanes(self, meta_path: Path, project: str) -> set[str]:
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            self.error(f"[{project}] Failed to read {meta_path}: {exc}")
            return set()

        lanes: set[str] = set()
        for lane in meta.get("lanes", []):
            lane_id = str(lane.get("id", ""))
            if lane.get("status") == "active":
                lanes.add(lane_id)
        return lanes

    def dependency_ids(self, fields: dict[str, object]) -> tuple[str, ...]:
        value = fields.get("depends_on") or fields.get("dependencies") or ""
        if isinstance(value, list):
            raw_items = [str(item) for item in value]
        else:
            raw_items = re.split(r"[,\s]+", str(value))
        seen: set[str] = set()
        deps: list[str] = []
        for item in raw_items:
            dep = item.strip()
            if not dep or dep in seen:
                continue
            seen.add(dep)
            deps.append(dep)
        return tuple(deps)

    def validate_ticket_file(
        self,
        project: str,
        path: Path,
        active_lanes: set[str],
        seen_ids: set[str],
    ) -> Ticket | None:
        fields = self.parse_frontmatter(path, project)
        ticket_id = str(fields.get("id", ""))
        lane = str(fields.get("lane", ""))

        if not TICKET_ID_RE.fullmatch(ticket_id):
            self.error(f"[{project}] Invalid id in {path}: {ticket_id}")
            return None

        if not LANE_RE.fullmatch(lane):
            self.error(f"[{project}] Invalid lane in {path}: {lane}")
        elif lane not in active_lanes:
            self.error(f"[{project}] Lane is not active in __project__.json for {path}: {lane}")

        status = str(fields.get("status", ""))
        if status not in TICKET_STATUSES:
            allowed = ", ".join(sorted(TICKET_STATUSES))
            self.error(f"[{project}] Invalid ticket status in {path}: {status} (allowed: {allowed})")

        if "current" in fields:
            self.error(
                f"[{project}] Legacy top-level current field in {path}; "
                "use the body section instead"
            )
        if "family" in fields:
            self.error(f"[{project}] Legacy top-level family field in {path}; use lane instead")

        if not path.name.startswith(f"{ticket_id}-"):
            self.error(f"[{project}] Filename does not start with id in {path}")

        if ticket_id in seen_ids:
            self.error(f"[{project}] Duplicate ticket id: {ticket_id}")
        seen_ids.add(ticket_id)

        return Ticket(
            project=project,
            path=path,
            id=ticket_id,
            lane=lane,
            dependencies=self.dependency_ids(fields),
        )

    def validate_counter(self, project: str, index_path: Path, max_id: str) -> None:
        if not index_path.is_file():
            return

        try:
            index = json.loads(index_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            self.error(f"[{project}] Failed to read {index_path}: {exc}")
            return

        actual = str(index.get("current_counter", ""))
        if not actual:
            self.error(f"[{project}] Missing current_counter in {index_path}")
            return
        if not actual.isdigit():
            self.error(f"[{project}] Invalid current_counter in {index_path}: {actual}")
            return
        if int(actual) < int(max_id or "0"):
            self.error(
                f"[{project}] __tickets__.json current_counter {actual} "
                f"is lower than max ticket id {max_id}"
            )

    def validate_project(self, project: str) -> None:
        if not self.validate_project_name(project):
            return

        project_dir = PROJECTS_DIR / project
        tickets_dir = project_dir / "tickets"
        meta_path = project_dir / "__project__.json"
        index_path = project_dir / "__tickets__.json"

        if not meta_path.is_file():
            self.error(f"Project metadata missing for project {project} ({meta_path})")
            return
        if not tickets_dir.is_dir():
            self.error(f"Tickets directory missing for project {project} ({tickets_dir})")
            return

        active_lanes = self.active_lanes(meta_path, project)
        seen_ids: set[str] = set()
        tickets: dict[str, Ticket] = {}

        for path in sorted(tickets_dir.glob("*.md")):
            if not path.is_file():
                continue
            ticket = self.validate_ticket_file(project, path, active_lanes, seen_ids)
            if ticket is not None:
                tickets[ticket.id] = ticket

        max_id = max(seen_ids, default="000000")
        self.validate_counter(project, index_path, max_id)
        self.project_tickets[project] = tickets

    def validate_dependencies(self) -> None:
        for project, tickets in self.project_tickets.items():
            for ticket in tickets.values():
                for dep in ticket.dependencies:
                    dep_project = project
                    dep_id = dep
                    cross = CROSS_PROJECT_DEP_RE.fullmatch(dep)
                    if cross:
                        dep_project, dep_id = cross.group(1), cross.group(2)

                    if not TICKET_ID_RE.fullmatch(dep_id):
                        self.error(f"[{project}] Invalid dependency in {ticket.path}: {dep}")
                        continue
                    if dep_project == project and dep_id == ticket.id:
                        self.error(
                            f"[{project}] Ticket {ticket.id} cannot depend on itself "
                            f"({ticket.path})"
                        )
                    if dep_project not in self.project_tickets:
                        self.error(
                            f"[{project}] Unknown dependency project {dep_project} "
                            f"referenced by {ticket.path}"
                        )
                        continue
                    if dep_id not in self.project_tickets[dep_project]:
                        self.error(f"[{project}] Missing dependency {dep} referenced by {ticket.path}")

            self.validate_local_dependency_cycles(project, tickets)

    def validate_local_dependency_cycles(self, project: str, tickets: dict[str, Ticket]) -> None:
        state: dict[str, int] = {}
        stack: list[str] = []

        def local_deps(ticket: Ticket) -> list[str]:
            deps: list[str] = []
            for dep in ticket.dependencies:
                if CROSS_PROJECT_DEP_RE.fullmatch(dep):
                    continue
                if dep in tickets:
                    deps.append(dep)
            return deps

        def visit(ticket_id: str) -> None:
            mark = state.get(ticket_id, 0)
            if mark == 2:
                return
            if mark == 1:
                start = stack.index(ticket_id)
                cycle = " -> ".join([*stack[start:], ticket_id])
                self.error(f"[{project}] Ticket dependency cycle: {cycle}")
                return

            state[ticket_id] = 1
            stack.append(ticket_id)
            for dep in local_deps(tickets[ticket_id]):
                visit(dep)
            stack.pop()
            state[ticket_id] = 2

        for ticket_id in tickets:
            visit(ticket_id)


def project_names(scope_project: str | None) -> list[str]:
    if scope_project:
        return [scope_project]

    names: list[str] = []
    if not PROJECTS_DIR.is_dir():
        return names
    for entry in sorted(PROJECTS_DIR.iterdir()):
        if entry.is_dir() and (entry / "__project__.json").is_file():
            names.append(entry.name)
    return names


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate ticket id/lane/filename consistency across Blackboard projects.",
    )
    parser.add_argument("--project", help="Validate one project instead of every project.")
    args = parser.parse_args()

    validator = Validator()
    for project in project_names(args.project):
        validator.validate_project(project)
    validator.validate_dependencies()

    return 1 if validator.errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
