#!/usr/bin/env python3
"""Prepare and verify the Blackboard Tauri desktop bundle inputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import struct
import subprocess
import sys
from pathlib import Path

from setup import ensure_web_dependencies, npm_cmd


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
BB_BACKEND_DIR = REPO_ROOT / "bb_backend"
BB_WEB_DIR = REPO_ROOT / "bb_web"
BB_DESKTOP_DIR = REPO_ROOT / "bb_desktop"
REPO_TEMPLATE_ROOT = REPO_ROOT / ".bb_template"
SRC_TAURI_DIR = BB_DESKTOP_DIR / "src-tauri"
DESKTOP_RESOURCES_DIR = BB_DESKTOP_DIR / "resources"
SIDECAR_TARGET_DIR = SRC_TAURI_DIR / "sidecar-target"
PORTABLE_DIR = BB_DESKTOP_DIR / "portable"
PORTABLE_PAYLOAD = PORTABLE_DIR / "Blackboard-portable-payload.bbpack"
PORTABLE_LAUNCHER_DIR = BB_DESKTOP_DIR / "portable-launcher"
PORTABLE_LAUNCHER_TARGET_DIR = SRC_TAURI_DIR / "portable-target"
PORTABLE_OUTPUT_DIR = SRC_TAURI_DIR / "target" / "release" / "portable"
PORTABLE_PAYLOAD_MAGIC = b"BBPORTABLE1\n"

SEED_ENTRIES = ["blackboard.json", "agents", "projects", "schemas", "task_graphs"]
HOME_TEMPLATE_ENTRIES = ["blackboard.json", "agents", "schemas", "task_graphs"]


def log(message: str) -> None:
    print(f"  {message}", flush=True)


def run(cmd: list[str], cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess:
    log("-> " + " ".join(cmd))
    return subprocess.run(cmd, cwd=cwd, check=True)


def rust_host_triple() -> str:
    result = subprocess.run(
        ["rustc", "-vV"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.split(":", 1)[1].strip()
    raise RuntimeError("could not determine Rust host target triple from `rustc -vV`")


def exe_suffix(target: str) -> str:
    return ".exe" if "windows" in target else ""


def cargo_binary_path(target_dir: Path, target: str | None, profile: str, binary: str) -> Path:
    if target:
        return target_dir / target / profile / f"{binary}{exe_suffix(target)}"
    host = rust_host_triple()
    return target_dir / profile / f"{binary}{exe_suffix(host)}"


def sidecar_binary_name(binary: str, target: str) -> str:
    return f"{binary}-{target}{exe_suffix(target)}"


def build_web(skip: bool = False) -> None:
    if skip:
        log("Skipping bb_web build")
        return
    ensure_web_dependencies()
    run([npm_cmd(), "run", "build", "--prefix", str(BB_WEB_DIR)], cwd=REPO_ROOT)


def build_sidecar_package(
    package: str,
    binary: str,
    target: str | None,
    release: bool,
    skip: bool = False,
) -> tuple[str, Path]:
    resolved_target = target or rust_host_triple()
    output_dir = SRC_TAURI_DIR / "binaries"
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / sidecar_binary_name(binary, resolved_target)

    if not skip:
        cmd = [
            "cargo",
            "build",
            "--manifest-path",
            str(BB_BACKEND_DIR / "Cargo.toml"),
            "--target-dir",
            str(SIDECAR_TARGET_DIR),
            "-p",
            package,
        ]
        if target:
            cmd.extend(["--target", target])
        if release:
            cmd.append("--release")
        run(cmd, cwd=REPO_ROOT)

    profile = "release" if release else "debug"
    source_path = cargo_binary_path(SIDECAR_TARGET_DIR, target, profile, binary)
    if not source_path.is_file():
        raise FileNotFoundError(f"built sidecar binary is missing: {source_path}")

    shutil.copy2(source_path, output_path)
    log(f"Copied sidecar: {output_path}")
    return resolved_target, output_path


def build_sidecars(target: str | None, release: bool, skip: bool = False) -> tuple[str, list[Path]]:
    resolved_target, cli_path = build_sidecar_package("bb_cli", "bb", target, release, skip)
    _, daemon_path = build_sidecar_package("bb_daemon", "bb-daemon", target, release, skip)
    return resolved_target, [cli_path, daemon_path]


def replace_dir(source: Path, target: Path) -> None:
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(source, target)


def prepare_web_resource() -> None:
    source = BB_WEB_DIR / "dist"
    if not (source / "index.html").is_file():
        raise FileNotFoundError(f"bb_web build output is missing index.html: {source}")
    target = DESKTOP_RESOURCES_DIR / "web"
    replace_dir(source, target)
    log(f"Prepared web resource: {target}")


def prepare_seed_resource() -> None:
    source_root = REPO_TEMPLATE_ROOT
    target_root = DESKTOP_RESOURCES_DIR / "seed" / ".bb"

    if not (source_root / "blackboard.json").is_file():
        raise FileNotFoundError(f"Blackboard seed manifest is missing: {source_root / 'blackboard.json'}")

    if target_root.exists():
        shutil.rmtree(target_root)
    target_root.mkdir(parents=True, exist_ok=True)

    for entry in SEED_ENTRIES:
        source = source_root / entry
        target = target_root / entry
        if not source.exists():
            raise FileNotFoundError(f"required seed entry is missing: {source}")
        if source.is_dir():
            shutil.copytree(source, target)
        else:
            shutil.copy2(source, target)

    write_builtin_blackboard_registry(target_root)

    runtime_path = target_root / "runtime"
    if runtime_path.exists():
        raise RuntimeError(f"seed resource must not include runtime data: {runtime_path}")
    log(f"Prepared seed resource: {target_root}")


def write_builtin_blackboard_registry(target_root: Path) -> None:
    blackboard_project = target_root / "projects" / "blackboard"
    if not (blackboard_project / "__project__.json").is_file():
        raise FileNotFoundError(f"desktop seed is missing builtin blackboard project: {blackboard_project}")
    registry = {
        "projects": {
            "blackboard": {
                "uuid": "00000000-0000-0000-0000-000000000000",
                "locations": {
                    "CURRENT": {
                        "absolute_path": "",
                        "relative_path": "./blackboard",
                    }
                },
            }
        },
        "machines": {},
    }
    (target_root / "projects" / "__projects__.json").write_text(
        json.dumps(registry, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def prepare_home_template_resource() -> None:
    source_root = REPO_TEMPLATE_ROOT
    target_root = DESKTOP_RESOURCES_DIR / "home-template" / ".bb"

    if not (source_root / "blackboard.json").is_file():
        raise FileNotFoundError(f"Blackboard home manifest is missing: {source_root / 'blackboard.json'}")

    if target_root.exists():
        shutil.rmtree(target_root)
    target_root.mkdir(parents=True, exist_ok=True)

    for entry in HOME_TEMPLATE_ENTRIES:
        source = source_root / entry
        target = target_root / entry
        if not source.exists():
            raise FileNotFoundError(f"required home template entry is missing: {source}")
        if source.is_dir():
            shutil.copytree(source, target)
        else:
            shutil.copy2(source, target)

    projects_root = target_root / "projects"
    projects_root.mkdir(parents=True, exist_ok=True)
    (projects_root / "__projects__.json").write_text(
        json.dumps({"projects": {}, "machines": {}}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    runtime_path = target_root / "runtime"
    blackboard_project = projects_root / "blackboard"
    if runtime_path.exists():
        raise RuntimeError(f"home template must not include runtime data: {runtime_path}")
    if blackboard_project.exists():
        raise RuntimeError(f"home template must not include demo project: {blackboard_project}")
    log(f"Prepared home template resource: {target_root}")


def verify_resources(target: str) -> None:
    checks = [
        SRC_TAURI_DIR / "binaries" / sidecar_binary_name("bb", target),
        SRC_TAURI_DIR / "binaries" / sidecar_binary_name("bb-daemon", target),
        DESKTOP_RESOURCES_DIR / "web" / "index.html",
        DESKTOP_RESOURCES_DIR / "seed" / ".bb" / "blackboard.json",
        DESKTOP_RESOURCES_DIR / "home-template" / ".bb" / "blackboard.json",
        DESKTOP_RESOURCES_DIR / "home-template" / ".bb" / "projects" / "__projects__.json",
    ]
    for path in checks:
        if not path.exists():
            raise FileNotFoundError(f"desktop resource check failed: {path}")
    runtime_path = DESKTOP_RESOURCES_DIR / "seed" / ".bb" / "runtime"
    if runtime_path.exists():
        raise RuntimeError(f"desktop seed resource unexpectedly contains runtime: {runtime_path}")
    seed_registry_path = DESKTOP_RESOURCES_DIR / "seed" / ".bb" / "projects" / "__projects__.json"
    seed_registry = json.loads(seed_registry_path.read_text(encoding="utf-8"))
    seed_projects = seed_registry.get("projects", {})
    if sorted(seed_projects.keys()) != ["blackboard"]:
        raise RuntimeError(f"desktop seed registry must only contain blackboard: {seed_registry_path}")
    blackboard_location = seed_projects["blackboard"].get("locations", {}).get("CURRENT", {})
    if blackboard_location.get("absolute_path"):
        raise RuntimeError(f"desktop seed registry must not contain absolute paths: {seed_registry_path}")
    if blackboard_location.get("relative_path") != "./blackboard":
        raise RuntimeError(f"desktop seed registry must register blackboard as ./blackboard: {seed_registry_path}")
    template_root = DESKTOP_RESOURCES_DIR / "home-template" / ".bb"
    if (template_root / "runtime").exists():
        raise RuntimeError(f"desktop home template unexpectedly contains runtime: {template_root / 'runtime'}")
    if (template_root / "projects" / "blackboard").exists():
        raise RuntimeError(
            f"desktop home template unexpectedly contains blackboard project: {template_root / 'projects' / 'blackboard'}"
        )


def prepare(args: argparse.Namespace) -> None:
    release = args.release and not args.dev
    build_web(skip=args.skip_web_build)
    target, _ = build_sidecars(
        target=args.target,
        release=release,
        skip=args.skip_sidecar_build,
    )
    prepare_web_resource()
    prepare_seed_resource()
    prepare_home_template_resource()
    verify_resources(target)
    print("\nDesktop bundle inputs are ready.", flush=True)


def build_desktop_release_no_bundle() -> None:
    clean_release_resource_dirs()
    run([npm_cmd(), "run", "build", "--prefix", str(BB_DESKTOP_DIR), "--", "--no-bundle"], cwd=REPO_ROOT)


def release_dir() -> Path:
    return SRC_TAURI_DIR / "target" / "release"


def clean_release_resource_dirs() -> None:
    root = release_dir()
    for dirname in ["web", "seed", "home-template", "workspace-template"]:
        path = root / dirname
        if path.exists():
            shutil.rmtree(path)
            log(f"Removed stale release resource: {path}")


def release_payload_roots() -> list[Path]:
    root = release_dir()
    return [
        root / "blackboard-desktop.exe",
        root / "bb.exe",
        root / "web",
        root / "seed",
        root / "home-template",
    ]


def collect_payload_files() -> list[tuple[str, Path]]:
    files: list[tuple[str, Path]] = []
    for root in release_payload_roots():
        if not root.exists():
            raise FileNotFoundError(f"portable payload entry is missing: {root}")
        if root.is_file():
            files.append((root.name, root))
            continue
        for path in sorted(root.rglob("*")):
            if path.is_file():
                archive_name = Path(root.name) / path.relative_to(root)
                files.append((archive_name.as_posix(), path))

    if not files:
        raise RuntimeError("portable payload would be empty")
    return files


def write_portable_payload() -> str:
    files = collect_payload_files()
    PORTABLE_DIR.mkdir(parents=True, exist_ok=True)
    hasher = hashlib.sha256()

    with PORTABLE_PAYLOAD.open("wb") as output:
        output.write(PORTABLE_PAYLOAD_MAGIC)
        output.write(struct.pack("<Q", len(files)))
        hasher.update(PORTABLE_PAYLOAD_MAGIC)
        hasher.update(struct.pack("<Q", len(files)))

        for archive_name, path in files:
            name_bytes = archive_name.encode("utf-8")
            data = path.read_bytes()
            output.write(struct.pack("<I", len(name_bytes)))
            output.write(struct.pack("<Q", len(data)))
            output.write(name_bytes)
            output.write(data)
            hasher.update(struct.pack("<I", len(name_bytes)))
            hasher.update(struct.pack("<Q", len(data)))
            hasher.update(name_bytes)
            hasher.update(data)

    digest = hasher.hexdigest()
    log(f"Prepared portable payload: {PORTABLE_PAYLOAD} ({len(files)} files, sha256={digest[:16]})")
    return digest


def portable_arch_name(target: str) -> str:
    if target.startswith("x86_64"):
        return "x64"
    if target.startswith("aarch64"):
        return "arm64"
    return target


def build_portable_launcher(target: str) -> Path:
    if "windows" not in target:
        raise RuntimeError(f"portable self-contained exe is currently Windows-only, got target {target}")

    run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(PORTABLE_LAUNCHER_DIR / "Cargo.toml"),
            "--target-dir",
            str(PORTABLE_LAUNCHER_TARGET_DIR),
            "--release",
        ],
        cwd=REPO_ROOT,
    )

    built = PORTABLE_LAUNCHER_TARGET_DIR / "release" / f"blackboard-portable{exe_suffix(target)}"
    if not built.is_file():
        raise FileNotFoundError(f"portable launcher build output is missing: {built}")

    PORTABLE_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    output = PORTABLE_OUTPUT_DIR / f"Blackboard-portable-{portable_arch_name(target)}{exe_suffix(target)}"
    shutil.copy2(built, output)
    log(f"Prepared portable self-contained exe: {output}")
    return output


def portable(args: argparse.Namespace) -> None:
    target = rust_host_triple()
    if not args.skip_desktop_build:
        build_desktop_release_no_bundle()
    digest = write_portable_payload()
    output = build_portable_launcher(target)
    print("\nDesktop portable artifact is ready.", flush=True)
    print(f"  exe: {output}", flush=True)
    print(f"  payload_sha256: {digest}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Prepare Blackboard Desktop bundle inputs.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare_parser = subparsers.add_parser("prepare", help="Build and stage desktop bundle inputs.")
    profile = prepare_parser.add_mutually_exclusive_group()
    profile.add_argument("--dev", action="store_true", help="Build a debug sidecar for tauri dev.")
    profile.add_argument("--release", action="store_true", help="Build a release sidecar for tauri build.")
    prepare_parser.add_argument("--target", help="Rust target triple for sidecar naming/cross build.")
    prepare_parser.add_argument("--skip-web-build", action="store_true", help="Use existing bb_web/dist.")
    prepare_parser.add_argument(
        "--skip-sidecar-build",
        action="store_true",
        help="Use existing sidecar-target build output and only copy/stage resources.",
    )
    prepare_parser.set_defaults(func=prepare)

    portable_parser = subparsers.add_parser(
        "portable",
        help="Build a Windows self-contained portable exe that embeds the desktop release payload.",
    )
    portable_parser.add_argument(
        "--skip-desktop-build",
        action="store_true",
        help="Use the existing Tauri release executable, sidecar, and resources.",
    )
    portable_parser.set_defaults(func=portable)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
