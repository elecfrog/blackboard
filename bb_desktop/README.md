# Blackboard Desktop

Tauri v2 desktop launcher for Blackboard. The desktop app owns the native
window and starts the Rust `bb-daemon` sidecar HTTP server. The `bb` sidecar
is still bundled for one-shot workspace initialization.

## Development

```powershell
python ..\scripts\desktop.py prepare --dev
npm run dev
```

`prepare` builds `bb_web`, builds the `bb` and `bb-daemon` sidecars into an isolated target
directory, copies the target-triple binary into `src-tauri/binaries/`, and
creates curated bundle resources under `resources/`: production web assets,
the demo seed, and the user-level home template generated from the repo
`.bb_template`.

## Release

```powershell
npm run build
```

The Windows trial build emits an NSIS installer at
`src-tauri\target\release\bundle\nsis\Blackboard_0.1.0_x64-setup.exe` and a
release executable at `src-tauri\target\release\blackboard-desktop.exe`.

The bundle target is intentionally `nsis` for now. MSI/WiX is stricter about
Windows installer code pages and currently fails when bundled seed resources
contain non-ASCII project filenames.

## Portable

```powershell
python ..\scripts\desktop.py portable
```

This emits a single self-contained launcher at
`src-tauri\target\release\portable\Blackboard-portable-x64.exe`.

The portable launcher embeds the release desktop executable, the `bb`/`bb-daemon` sidecars,
the bundled web resources, the demo seed, and the user-level home template. On
first run it extracts that payload into
`%LOCALAPPDATA%\Blackboard\portable\<payload-hash>\` and launches Blackboard
from there. The portable cache only stores application files.

## Data Root

Desktop uses an Open Folder workspace model. The user selects a code folder;
Blackboard opens an existing project-local `<folder>\.bb` capsule or, after
confirmation, injects a new one:

```text
<folder>\.bb\
├── __project__.json
├── __tickets__.json
├── __inbox__.json
├── inbox\        # JSON inbox handoff notes
├── tickets\
└── wiki\
```

The project slug and display name are derived from the folder name in v1, and
`repos` points at the selected folder. Desktop never writes a `.gitignore`; the
UI only reminds users that cache, tmp, and lock files are runtime state worth
ignoring.

Desktop uses a user-level Blackboard home, similar to `.codex` or `.config`:

- Windows: `C:\Users\<you>\.bb`
- macOS/Linux: `~/.bb`

Desktop starts the sidecar with this global root. Its
`~\.bb\projects\__projects__.json` file is the global project registry. When a
folder is opened, Blackboard initializes or reads the project capsule at
`<folder>\.bb`, then registers that capsule root into the global registry. This
lets `/api/projects` and MCP `list_projects` see projects from every folder the
user has opened.

`agents\`, `schemas\`, `task_graphs\`, and `runtime\` belong to the global
home by default. `templates\` is deprecated; schema-owned contracts live under
`schemas\`. Project-local versions may be created later as overrides, but Open
Folder does not copy global system configuration into project capsules.

Desktop runtime state and logs live under `~\.bb\runtime\desktop\`: the active
sidecar URLs are recorded in `state.json`, and logs are written under `logs\`.
Opened projects are not stored in `state.json`; the only project registry is
`~\.bb\projects\__projects__.json`. Agent connector sync uses the current
sidecar `mcp_url`, so fallback ports do not write stale `127.0.0.1:3001`
connector entries.

Desktop prefers `127.0.0.1:3001` for compatibility. If that port is already in
use, it does not terminate the existing process; it scans `3002..3099`, starts
the sidecar on the first free port, opens the WebView there, and records
`sidecar_url` plus `mcp_url` in `state.json`.
