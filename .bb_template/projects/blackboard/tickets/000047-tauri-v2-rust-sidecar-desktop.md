+++
id = "000047"
lane = "bbt"
title = "Tauri v2 Desktop：Rust sidecar 打包与本地工作台"
created_at = "2026-05-11"
updated_at = "2026-05-12"
status = "archived"
area = "bb_desktop bb_cli static-serving scripts"
assignee = "codex"
depends_on = "000046"
+++

# 当前进展

- 2026-05-11 Codex 开始实现 Tauri v2 Desktop：新增桌面壳、打包 bb sidecar、静态前端服务和用户数据目录启动流程。

- Windows trial release now builds via Tauri NSIS target; installer produced at bb_desktop/src-tauri/target/release/bundle/nsis/Blackboard_0.1.0_x64-setup.exe.
- Release desktop app was launched from bb_desktop/src-tauri/target/release/blackboard-desktop.exe and confirmed to start the bb sidecar on 127.0.0.1:3001 using %APPDATA%/Blackboard as data root.
- Docs now call out the NSIS release path and note that MSI/WiX remains non-default because bundled seed filenames can contain non-ASCII characters.

- Added a Windows portable self-contained release path: `python scripts/desktop.py portable` now builds a single `Blackboard-portable-x64.exe` launcher embedding the desktop executable, bb sidecar, web resources, and `.bb` seed.
- Added `bb_desktop/portable-launcher`, a dependency-free Rust launcher that extracts the embedded payload into `%LOCALAPPDATA%/Blackboard/portable/<payload-hash>/` and starts the real Tauri desktop from there.
- Documented the portable artifact path and runtime/cache behavior in README.md and bb_desktop/README.md.

- Open Folder workspace model implemented: Desktop now opens selected code folders as workspace roots and injects .bb on demand.

- 2026-05-11 Codex aligned Desktop and legacy Server+Web data-root semantics: Desktop initializes and runs against user-home ~/.bb, while scripts/dev.py defaults back to the repo root .bb for Blackboard self-development.
- Added explicit local-dev user-root controls: scripts/dev.py --user-root, BB_DEV_USER_ROOT=1, and BB_DEV_ROOT=<path>; the opt-in path initializes missing .bb data from the repo seed without copying runtime or the blackboard demo project.
- Desktop state is now runtime-only under ~/.bb/runtime/desktop/state.json and records sidecar_addr/sidecar_url/mcp_url only; project restoration/discovery flows through ~/.bb/projects/__projects__.json.
- Portable release was rebuilt after the root-model changes and verified to inject folder .bb projects into the global ~/.bb registry without emitting Windows \\?\ path prefixes.

- 2026-05-11 Codex hotfixed Desktop /api/projects 500 caused by stale global registry entries whose absolute_path no longer exists on the current machine.
- Workspace project listing now treats unreachable registered project data roots as unavailable and skips them, while exact open_project for that name returns ProjectNotFound instead of taking down the whole project list.
- Rebuilt the portable executable with the fixed sidecar.

- 2026-05-11 Codex converted the source-checkout system template from repo `.bb` to `.bb_template`; repo `.bb` is now runtime-only/ignored, `scripts/dev.py` defaults to `.bb_template`, and Desktop seed/home-template resources are generated from `.bb_template`.
- Folder Open project data now uses a direct project capsule layout at `<folder>/.bb` with `__project__.json`, `__tickets__.json`, `__inbox__.json`, `inbox/`, `tickets/`, and `wiki/`; it no longer creates `<folder>/.bb/projects/<project>` or copies global agents/templates/task_graphs into the project capsule.
- Windows path normalization now strips `\\?\` from canonicalized paths before they reach logs, config-facing strings, API responses, or project registry entries.
- Source-checkout runtime for `.bb_template` now writes to repo `.bb/runtime`, so opening `.bb_template` for Web dev does not create `.bb_template/runtime`.
- 2026-05-11 Codex removed the Open Folder initialization naming choice: the confirmation dialog no longer exposes Project Name or display name fields, and the backend derives both from the selected folder name while ignoring legacy request fields for compatibility.

- 2026-05-11：Desktop prefers 127.0.0.1:3001, falls back through 3002..3099; sidecar_addr/sidecar_url/mcp_url persisted in %APPDATA%/Blackboard/desktop/state.json; backend desktop-state writes preserve URL fields; agent connector sync uses active sidecar mcp_url for fallback-port launches; portable exe rebuilt at bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe.

- 2026-05-11：Implemented folder-root .bb initialization/validation; REST and MCP handlers read switchable workspace per request; connected Desktop startup, app state, workspace-template resources, and portable packaging; moved web entry points to Open Folder routing.

- 2026-05-11：Added python scripts/desktop.py portable for Windows self-contained artifact; added bb_desktop/portable-launcher Rust launcher; generated Blackboard-portable-x64.exe; verified self-extraction to %LOCALAPPDATA%/Blackboard/portable/<payload-hash>/ and sidecar on 127.0.0.1:3001; updated README docs.

- 2026-05-11：完成 `.bb_template` 隔离改造：源码仓库内置系统模板从 `.bb` 隔离到 `.bb_template`，repo `.bb` 保留为 runtime-only/ignored；调整 `scripts/dev.py` 默认以 `.bb_template` 启动；调整 Desktop prepare/portable 资源生成链路从 `.bb_template` 生成；Open Folder 注入模型改成 `<folder>/.bb` direct project capsule，不再生成 `<folder>/.bb/projects/<project>`；补上 Windows canonical path 清洗去掉 `\\?\` 前缀；`.bb_template` 打开时 runtime 改写到 repo `.bb/runtime`

# 记录


- Validation passed: npm run build --prefix bb_desktop produced the NSIS installer; /healthz, /, /api/projects, and MCP initialize/tools-list smoke checks passed against the release sidecar.
- Validation passed: normal CloseMainWindow shutdown stops both the Tauri desktop process and the bb sidecar and releases port 3001.
- Validation passed: cargo fmt for bb_backend and bb_desktop, cargo test --manifest-path bb_backend/Cargo.toml --workspace, cargo check --manifest-path bb_desktop/src-tauri/Cargo.toml.

- Validation passed: `python scripts/desktop.py portable` rebuilt the release payload and produced `bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe`.
- Validation passed: running the portable exe self-extracted to `%LOCALAPPDATA%/Blackboard/portable/2b683eb9c0eea11d/`, started the desktop app, and served `/healthz`, `/`, `/api/projects`, and MCP `tools/list` on 127.0.0.1:3001.
- Validation passed: normal window close stops the portable-launched desktop sidecar and releases port 3001.
- Validation passed: `cargo fmt --manifest-path bb_desktop/portable-launcher/Cargo.toml`, `python -m py_compile scripts/desktop.py`, `cargo check --manifest-path bb_desktop/portable-launcher/Cargo.toml`, `cargo check --manifest-path bb_desktop/src-tauri/Cargo.toml`, and `python scripts/desktop.py portable --skip-desktop-build`.

- Added folder workspace initialization and dynamic HTTP/MCP workspace switching in bb_cli/bb_core.
- Updated Tauri startup to use desktop state/log paths, bundled workspace-template, and sidecar --workspace-template/--desktop-state args.
- Updated web root, HomeView, and ProjectSwitcher to use Open Folder instead of hardcoded blackboard routing.
- Rebuilt portable self-contained artifact after release resource cleanup.

- Implemented Desktop sidecar port fallback: 127.0.0.1:3001 remains preferred, but occupied 3001 now causes Desktop to scan 3002..3099 and open the WebView on the selected URL without terminating the existing process.
- Persisted sidecar_addr, sidecar_url, and mcp_url in %APPDATA%/Blackboard/desktop/state.json and preserved those fields when the backend updates last_workspace_root.
- Updated Agent connector sync to use the active sidecar mcp_url so fallback ports do not write stale 127.0.0.1:3001 MCP entries.
- Rebuilt the portable self-contained exe at bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe.

- Validation passed: cargo fmt --manifest-path bb_backend\\Cargo.toml --all; cargo test --manifest-path bb_backend\\Cargo.toml --workspace; npm run build --prefix bb_web; cargo check --manifest-path bb_desktop\\src-tauri\\Cargo.toml.
- Validation passed: python -m py_compile scripts\\dev.py scripts\\desktop.py; python scripts\\dev.py --help; script helper smoke confirmed repo default and explicit custom user-root initialization.
- Validation passed: python scripts\\desktop.py prepare --release and python scripts\\desktop.py portable rebuilt bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe.
- Validation passed: isolated portable smoke created UserHome\\.bb, selected fallback http://127.0.0.1:3002/ while 3001 was occupied, opened a sample folder, created folder .bb, registered Sample-App globally, and found no last_workspace_root or \\?\ paths.
- Validation passed: old Server+Web smoke started repo-root server on 127.0.0.1:3098 and loaded blackboard from repo .bb; explicit user-root server on 127.0.0.1:3099 started with an initialized empty global registry.

- Validation passed: cargo fmt --manifest-path bb_backend\\Cargo.toml --all.
- Validation passed: cargo test --manifest-path bb_backend\\Cargo.toml --workspace (68 bb_cli tests, 194 bb_core tests).
- Validation passed: python scripts\\desktop.py prepare --release --skip-web-build rebuilt the sidecar and staged resources.
- Validation passed: sidecar smoke against real C:\\Users\\ornizhou\\.bb returned HTTP 200 for /api/projects and skipped the stale C:\\Dev\\kb project entry.
- Validation passed: python scripts\\desktop.py portable rebuilt bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe with payload sha256 740e4b7e7a7660057ea2d871c876073910ac70aac0debafa60d9f451d53b73b6.
- Validation passed: launching the rebuilt portable exe against the real user root returned HTTP 200 for /api/projects at http://127.0.0.1:3001/.
- Validation passed: cargo fmt --manifest-path bb_backend\\Cargo.toml --all.
- Validation passed: cargo test --manifest-path bb_backend\\Cargo.toml --workspace.
- Validation passed: npm run build --prefix bb_web.
- Validation passed: python scripts\\desktop.py prepare --dev --skip-web-build regenerated sidecar/resources from `.bb_template`.
- Validation passed: source dev HTTP smoke using --root .bb_template returned 200 for /healthz, /, /api/projects, MCP initialize, notifications/initialized, and tools/list; sidecar logs contained no `\\?\` prefixes and `.bb_template/runtime` stayed absent.
- Validation passed: python scripts\\desktop.py portable rebuilt bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe with payload sha256 791d00aa62d02579c9ce9028042e24f6914fa9e54604913bb7e1d59347414324.
- Validation passed: cargo fmt --manifest-path bb_backend\\Cargo.toml --all.
- Validation passed: cargo test --manifest-path bb_backend\\Cargo.toml --workspace.
- Validation passed: npm run build --prefix bb_web.
- Validation passed: built-resource grep found `workspaceInitializeName` in bb_web/dist and bb_desktop/resources/web; the previous Project Name/display-name fields are no longer referenced by the Open Folder dialog source.
- Validation passed: python scripts\\desktop.py portable rebuilt bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe with payload sha256 d1aa6ed64ec7588afe0e5f9a4b9a16af28698372aaac2abdb500f5d01d5a0b3b.
- Validation passed: python scripts\\check_ticket_ids.py.
- Validation passed: qmd embed.

- Inbox note: 2026-05-11-codex-desktop-dynamic-sidecar-port.md

- Inbox note: 2026-05-11-codex-desktop-open-folder-workspace.md

- Inbox note: 2026-05-11-codex-desktop-portable-single-file-release.md

- 来源：2026-05-11-codex-dotbb-template-isolation.md

- 来源：2026-05-11-codex-tauri-v2-rust-sidecar-desktop.md

- 来源：2026-05-11-codex-folder-name-project-default.md

# 下一步

- User acceptance: run `bb_desktop/src-tauri/target/release/portable/Blackboard-portable-x64.exe` on a fresh Windows machine, open a real code folder, and confirm the visible Open Folder workflow.
