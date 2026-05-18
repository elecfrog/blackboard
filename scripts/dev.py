#!/usr/bin/env python3
"""
Blackboard local development entry point.

Runs the local Blackboard development stack:
  - bb_daemon -> http://127.0.0.1:3001
  - bb_server -> http://127.0.0.1:3002
  - bb_web    -> http://127.0.0.1:8060

Use scripts/setup.py after clone or pull. This script also checks frontend
dependencies before starting the Vite server.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

from setup import ensure_web_dependencies, npm_cmd


def env_port(name: str, default: int) -> int:
    value = os.environ.get(name)
    if value is None or not value.strip():
        return default
    try:
        port = int(value)
    except ValueError:
        print(f"  [warn] invalid {name}={value!r}; using {default}")
        return default
    if not 0 < port < 65536:
        print(f"  [warn] invalid {name}={value!r}; using {default}")
        return default
    return port


DAEMON_PORT = env_port("BB_DAEMON_PORT", 3001)
SERVER_PORT = env_port("BB_SERVER_PORT", 3002)
FRONTEND_PORT = env_port("BB_FRONTEND_PORT", 8060)
DAEMON_URL = f"http://127.0.0.1:{DAEMON_PORT}"
SERVER_URL = f"http://127.0.0.1:{SERVER_PORT}"

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
BB_BACKEND_DIR = REPO_ROOT / "bb_backend"
BB_WEB_DIR = REPO_ROOT / "bb_web"
REPO_TEMPLATE_ROOT = REPO_ROOT / ".bb_template"
DEV_ROOT_ENV = "BB_DEV_ROOT"
DEV_USER_ROOT_ENV = "BB_DEV_USER_ROOT"

processes: list[tuple[str, subprocess.Popen]] = []


def log(title: str) -> None:
    print("\n" + "=" * 60)
    print(f"  {title}")
    print("=" * 60)


def run_probe(cmd: list[str], timeout: int = 10) -> tuple[bool, str]:
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False, ""

    if result.returncode != 0:
        return False, result.stderr.strip()
    return True, result.stdout.strip()


def _uses_port(address: str, port: int) -> bool:
    return address.rsplit(":", 1)[-1] == str(port)


def _listening_pids_windows(port: int) -> set[int]:
    result = subprocess.run(
        ["netstat", "-ano"],
        capture_output=True,
        text=True,
        check=True,
    )

    pids: set[int] = set()
    for line in result.stdout.splitlines():
        parts = line.split()
        if len(parts) < 5:
            continue
        if parts[0].upper() != "TCP" or parts[-2].upper() != "LISTENING":
            continue
        if not _uses_port(parts[1], port):
            continue
        pid = parts[-1].strip()
        if pid.isdigit() and int(pid) > 0:
            pids.add(int(pid))
    return pids


def _listening_pids_posix(port: int) -> set[int]:
    try:
        result = subprocess.run(
            ["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN", "-t"],
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError:
        print(f"  [warn] lsof not found; skip port {port} cleanup")
        return set()

    if result.returncode not in (0, 1):
        stderr = result.stderr.strip()
        if stderr:
            print(f"  [warn] lsof failed for port {port}: {stderr}")
        return set()

    return {
        int(line.strip())
        for line in result.stdout.splitlines()
        if line.strip().isdigit() and int(line.strip()) > 0
    }


def listening_pids(port: int) -> set[int]:
    if sys.platform == "win32":
        return _listening_pids_windows(port)
    return _listening_pids_posix(port)


def stop_pid(pid: int, force: bool = False) -> None:
    if sys.platform == "win32":
        subprocess.run(
            ["taskkill", "/F", "/PID", str(pid)],
            capture_output=True,
            check=False,
        )
        return

    sig = signal.SIGKILL if force else signal.SIGTERM
    try:
        os.kill(pid, sig)
    except ProcessLookupError:
        pass
    except PermissionError:
        action = "force stop" if force else "stop"
        print(f"  [warn] no permission to {action} PID {pid}")


def kill_port(port: int) -> None:
    print(f"[clean] checking port {port} ...")
    try:
        pids = listening_pids(port)
    except subprocess.CalledProcessError as exc:
        print(f"  [warn] failed to inspect port {port}: {exc}")
        return

    if not pids:
        print(f"  port {port} is free")
        return

    current_pid = os.getpid()
    for pid in sorted(pids):
        if pid == current_pid:
            continue
        print(f"  stopping PID {pid} on port {port}")
        stop_pid(pid)

    time.sleep(0.8)
    remaining = listening_pids(port).intersection(pids)
    if remaining:
        for pid in sorted(remaining):
            print(f"  force stopping PID {pid} on port {port}")
            stop_pid(pid, force=True)
        time.sleep(0.5)

    print(f"  port {port} released")


def cleanup(*_args: object) -> None:
    print("\n\n[stop] shutting down services ...")
    for _name, proc in processes:
        if proc.poll() is not None:
            continue
        try:
            if sys.platform == "win32":
                proc.send_signal(signal.CTRL_BREAK_EVENT)
            else:
                proc.terminate()
        except OSError:
            pass

    for name, proc in processes:
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            print(f"  force killing {name}")
            proc.kill()
    print("[done] all services stopped")
    sys.exit(0)


def check_prerequisites(run_rust_services: bool, run_frontend: bool) -> None:
    errors: list[str] = []

    if run_rust_services:
        ok, version = run_probe(["cargo", "--version"])
        if ok:
            print(f"  cargo {version}")
        else:
            errors.append("Cargo/Rust is not installed or not on PATH")

    if run_frontend:
        ok, version = run_probe([npm_cmd(), "--version"])
        if ok:
            print(f"  npm {version}")
        else:
            errors.append("Node.js/npm is not installed or not on PATH")

    if run_rust_services and not BB_BACKEND_DIR.is_dir():
        errors.append(f"backend directory is missing: {BB_BACKEND_DIR}")
    if run_frontend and not BB_WEB_DIR.is_dir():
        errors.append(f"frontend directory is missing: {BB_WEB_DIR}")

    if errors:
        print()
        for error in errors:
            print(f"  [error] {error}")
        print()
        sys.exit(1)


def popen_kwargs() -> dict[str, object]:
    if sys.platform == "win32":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {}


def wait_http(url: str, name: str, timeout: float = 90.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1.0) as response:
                if 200 <= response.status < 500:
                    print(f"  {name} ready: {url}")
                    return True
        except (urllib.error.URLError, TimeoutError):
            pass
        time.sleep(0.5)

    print(f"  [warn] {name} did not answer before timeout: {url}")
    return False


def user_bb_root() -> Path:
    return Path.home() / ".bb"


def truthy_env(value: str | None) -> bool:
    return value is not None and value.strip().lower() in {"1", "true", "yes", "on"}


def copy_dir_missing(source: Path, target: Path) -> None:
    if not source.is_dir():
        return
    target.mkdir(parents=True, exist_ok=True)
    for child in source.iterdir():
        if child.name == "runtime":
            continue
        child_target = target / child.name
        if child.is_dir():
            copy_dir_missing(child, child_target)
        elif not child_target.exists():
            shutil.copy2(child, child_target)


def target_bb_data_root(root: Path) -> Path:
    return root if root.name in {".bb", ".bb_template"} else root / ".bb"


def ensure_global_bb_root(root: Path) -> None:
    data_root = target_bb_data_root(root)
    if data_root.exists() and not data_root.is_dir():
        print(f"  [error] bb root is not a directory: {data_root}")
        sys.exit(1)

    seed = REPO_TEMPLATE_ROOT
    if not seed.is_dir():
        print(f"  [error] repository template .bb_template is missing: {seed}")
        sys.exit(1)

    projects_root = data_root / "projects"
    data_root.mkdir(parents=True, exist_ok=True)
    projects_root.mkdir(parents=True, exist_ok=True)

    for entry in ["agents", "schemas", "task_graphs"]:
        copy_dir_missing(seed / entry, data_root / entry)

    manifest = data_root / "blackboard.json"
    if not manifest.exists():
        source_manifest = seed / "blackboard.json"
        if source_manifest.is_file():
            shutil.copy2(source_manifest, manifest)
        else:
            manifest.write_text(
                json.dumps(
                    {"schema_version": 1, "layout": "blackboard-dotbb"},
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )

    registry = projects_root / "__projects__.json"
    if not registry.exists():
        registry.write_text(
            json.dumps({"projects": {}, "machines": {}}, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    ensure_builtin_blackboard_project(data_root)


def ensure_builtin_blackboard_project(data_root: Path) -> None:
    source = REPO_TEMPLATE_ROOT / "projects" / "blackboard"
    target = data_root / "projects" / "blackboard"
    if not (source / "__project__.json").is_file():
        print(f"  [error] repository template blackboard project is missing: {source}")
        sys.exit(1)
    if not is_complete_project_capsule(target):
        copy_dir_missing(source, target)
    if not is_complete_project_capsule(target):
        print(f"  [error] builtin blackboard project is incomplete after init: {target}")
        sys.exit(1)

    registry_path = data_root / "projects" / "__projects__.json"
    try:
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
    except Exception:
        registry = {"projects": {}, "machines": {}}
    if not isinstance(registry, dict):
        registry = {"projects": {}, "machines": {}}
    projects = registry.setdefault("projects", {})
    if not isinstance(projects, dict):
        projects = {}
        registry["projects"] = projects
    registry.setdefault("machines", {})
    entry = projects.setdefault(
        "blackboard",
        {"uuid": "00000000-0000-0000-0000-000000000000", "locations": {}},
    )
    if not isinstance(entry, dict):
        entry = {"uuid": "00000000-0000-0000-0000-000000000000", "locations": {}}
        projects["blackboard"] = entry
    if not str(entry.get("uuid", "")).strip():
        entry["uuid"] = "00000000-0000-0000-0000-000000000000"
    locations = entry.setdefault("locations", {})
    if not isinstance(locations, dict):
        locations = {}
        entry["locations"] = locations
    locations["CURRENT"] = {"absolute_path": "", "relative_path": "./blackboard"}
    registry_path.write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def is_complete_project_capsule(root: Path) -> bool:
    return (
        (root / "__project__.json").is_file()
        and (root / "__tickets__.json").is_file()
        and (root / "__inbox__.json").is_file()
        and (root / "inbox").is_dir()
        and (root / "tickets").is_dir()
        and (root / "wiki").is_dir()
    )


def resolve_bb_root(args: argparse.Namespace) -> tuple[Path, bool, str]:
    env_root = os.environ.get(DEV_ROOT_ENV)
    env_user_root = truthy_env(os.environ.get(DEV_USER_ROOT_ENV))

    if args.root is not None:
        return args.root.expanduser(), True, "--root"
    if args.user_root:
        return user_bb_root(), True, "--user-root"
    if args.repo_root:
        return REPO_TEMPLATE_ROOT, False, "--repo-root"
    if env_root:
        return Path(env_root).expanduser(), True, DEV_ROOT_ENV
    if env_user_root:
        return user_bb_root(), True, DEV_USER_ROOT_ENV
    return REPO_TEMPLATE_ROOT, False, "repo default"


def start_daemon(bb_root: Path) -> subprocess.Popen:
    log(f"starting bb_daemon on {DAEMON_URL}")
    cmd = [
        "cargo",
        "run",
        "-p",
        "bb_daemon",
        "--",
        "--root",
        str(bb_root),
        "serve",
        "--addr",
        f"127.0.0.1:{DAEMON_PORT}",
    ]
    proc = subprocess.Popen(cmd, cwd=BB_BACKEND_DIR, **popen_kwargs())
    processes.append(("daemon", proc))
    return proc


def start_server(bb_root: Path) -> subprocess.Popen:
    log(f"starting bb_server on {SERVER_URL}")
    cmd = [
        "cargo",
        "run",
        "-p",
        "bb_server",
        "--",
        "--root",
        str(bb_root),
        "--addr",
        f"127.0.0.1:{SERVER_PORT}",
        "--daemon-url",
        DAEMON_URL,
    ]
    proc = subprocess.Popen(cmd, cwd=BB_BACKEND_DIR, **popen_kwargs())
    processes.append(("server", proc))
    return proc


def start_frontend() -> subprocess.Popen:
    log(f"starting bb_web on http://127.0.0.1:{FRONTEND_PORT}")
    env = os.environ.copy()
    env.setdefault("BLACKBOARD_API_TARGET", SERVER_URL)
    env.setdefault("BB_DAEMON_URL", DAEMON_URL)
    env.setdefault("BB_SERVER_URL", SERVER_URL)
    kwargs = popen_kwargs()
    kwargs["env"] = env
    proc = subprocess.Popen([npm_cmd(), "run", "dev"], cwd=BB_WEB_DIR, **kwargs)
    processes.append(("frontend", proc))
    return proc


def wait_for_processes() -> None:
    print("\n" + "-" * 56)
    print(f"  daemon:   {DAEMON_URL}")
    print(f"  server:   {SERVER_URL}")
    print(f"  frontend: http://127.0.0.1:{FRONTEND_PORT}")
    print("  cli:      cargo run -p bb_cli -- --root <bb-root> ...")
    print("  press Ctrl+C to stop all services")
    print("-" * 56 + "\n")

    try:
        while True:
            for name, proc in processes:
                ret = proc.poll()
                if ret is not None:
                    print(f"\n[warn] {name} exited with code {ret}")
                    cleanup()
            time.sleep(1)
    except KeyboardInterrupt:
        cleanup()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run the Blackboard local development stack.",
    )
    parser.add_argument(
        "--backend",
        action="store_true",
        help="Only start bb_daemon + bb_server. Compatibility alias for backend services.",
    )
    parser.add_argument("--daemon", action="store_true", help="Only start bb_daemon.")
    parser.add_argument(
        "--server",
        action="store_true",
        help="Only start bb_server. Also starts bb_daemon unless --reuse-daemon is set.",
    )
    parser.add_argument("--frontend", action="store_true", help="Only start bb_web.")
    parser.add_argument(
        "--reuse-daemon",
        action="store_true",
        help="Do not start or stop bb_daemon; use an existing daemon on 127.0.0.1:3001.",
    )
    parser.add_argument(
        "--install",
        action="store_true",
        help="Force frontend npm install before starting.",
    )
    parser.add_argument(
        "--no-clean-ports",
        action="store_true",
        help="Do not stop existing listeners on 3001/3002/8060 before starting.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help=f"Explicit Blackboard root for bb_daemon/bb_server. Also available as {DEV_ROOT_ENV}.",
    )
    parser.add_argument(
        "--repo-root",
        action="store_true",
        help="Use the repository .bb_template root. This is the default for local development.",
    )
    parser.add_argument(
        "--user-root",
        action="store_true",
        help=f"Use ~/.bb and initialize it from the repo .bb_template when missing. Also available as {DEV_USER_ROOT_ENV}=1.",
    )
    args = parser.parse_args()

    has_explicit_service = args.backend or args.daemon or args.server or args.frontend
    if has_explicit_service:
        run_frontend = args.frontend
        run_server = args.backend or args.server
        run_daemon = args.daemon or args.backend or (run_server and not args.reuse_daemon)
    else:
        run_daemon = not args.reuse_daemon
        run_server = True
        run_frontend = True

    if args.reuse_daemon and args.daemon:
        print("  [error] --daemon and --reuse-daemon cannot be used together")
        sys.exit(1)

    log("Blackboard development environment")
    bb_root, should_init_root, root_source = resolve_bb_root(args)
    print(f"  repo:     {REPO_ROOT}")
    print(f"  bb root:  {bb_root}")
    print(f"  root src: {root_source}")
    print(f"  daemon:   {'reuse' if args.reuse_daemon and run_server else ('yes' if run_daemon else 'no')}")
    print(f"  server:   {'yes' if run_server else 'no'}")
    print(f"  frontend: {'yes' if run_frontend else 'no'}")

    check_prerequisites(run_daemon or run_server, run_frontend)
    signal.signal(signal.SIGINT, cleanup)
    signal.signal(signal.SIGTERM, cleanup)

    if run_frontend:
        ensure_web_dependencies(force=args.install)
    if (run_daemon or run_server) and should_init_root:
        ensure_global_bb_root(bb_root)

    if not args.no_clean_ports:
        if run_daemon:
            kill_port(DAEMON_PORT)
        if run_server:
            kill_port(SERVER_PORT)
        if run_frontend:
            kill_port(FRONTEND_PORT)

    if run_daemon:
        start_daemon(bb_root)
        wait_http(f"{DAEMON_URL}/healthz", "bb_daemon")
    elif args.reuse_daemon and run_server:
        wait_http(f"{DAEMON_URL}/healthz", "bb_daemon", timeout=10.0)

    if run_server:
        start_server(bb_root)
        wait_http(f"{SERVER_URL}/healthz", "bb_server")

    if run_frontend:
        start_frontend()

    if processes:
        wait_for_processes()
    else:
        print("No services selected.")


if __name__ == "__main__":
    main()
