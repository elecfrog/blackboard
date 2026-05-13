+++
id = "000046"
lane = "bbt"
title = "Workspace 数据根迁移到 .bb 并为 Desktop sidecar 初始化铺路"
created_at = "2026-05-11"
updated_at = "2026-05-12"
status = "archived"
area = "bb_core bb_cli scripts"
assignee = "codex"
scope = "dotbb-workspace-root-desktop-seed"
+++

# 当前进展

- 2026-05-11 Codex 开始实现：将 agents/projects/task_graphs/templates/.bb_runtime 收束到 .bb 数据根，并保持开发/发布启动路径兼容。
- 2026-05-11 Codex 完成数据目录迁移：`agents/`、`projects/`、`task_graphs/`、`templates/` 移入 `.bb/`，`.bb_runtime/` 迁入 `.bb/runtime/`，并新增 `.bb/manifest.json`。
- 2026-05-11 Codex 更新 `Workspace::open/discover`：传 repo root 时优先解析 `.bb/projects`，同时保留旧 `root/projects` 兼容；新增 `Workspace::init_from_seed` 和 `bb init --seed`，供 Desktop 启动器把 seed `.bb` 初始化到用户数据目录。
- 2026-05-11 Codex 更新 task graph run state、HTTP SSE watcher、daemon 状态文件、ticket ID 检查脚本、README/后端文档和前端 Wiki 空态文案到新 `.bb` 布局。
- 2026-05-11 Codex 修复 `GraphCanvas.vue` 中阻塞前端构建的既有类型收窄问题，使本轮前端 build 门禁可通过。

- 2026-05-11：重命名 `.bb/manifest.json` 为 `.bb/blackboard.json`，同步 WORKSPACE_MANIFEST、workspace 测试和 README 布局说明到 blackboard.json

# 记录

- PASS：`cargo fmt --manifest-path bb_backend\Cargo.toml --all`。
- PASS：`cargo test --manifest-path bb_backend\Cargo.toml --workspace`（bb_cli 63 tests；bb_core 190 tests；doc-tests 0）。
- PASS：`cargo run --manifest-path bb_backend\Cargo.toml --target-dir <temp> -p bb_cli -- --root <temp-user-data> init --seed .`，临时用户数据目录生成 `.bb/projects/__projects__.json`，未复制 seed runtime。
- PASS：`python scripts\check_ticket_ids.py --project blackboard`。
- PASS：`python scripts\check_ticket_ids.py`。
- PASS：`qmd embed`（All content hashes already have embeddings）。
- PASS：`npm run build --prefix bb_web`（仅 chunk-size warning）。
- PASS：`git diff --check`。
- PASS：HTTP 冒烟：启动新构建 `bb http --root C:\Dev\blackboard --addr 127.0.0.1:3001` 后 `GET /api/projects` 返回 blackboard，`meta.data_root` 指向 `C:\Dev\blackboard\.bb\projects\blackboard`。
- PASS：dev 栈已启动，`http://127.0.0.1:8060/api/projects` 返回 blackboard project，后端监听 `127.0.0.1:3001`，前端监听 `0.0.0.0:8060`。

- 来源：2026-05-11-codex-dotbb-blackboard-json-manifest.md
- C:\Dev\blackboard\.bb\blackboard.json
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\workspace.rs
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\tests\mod.rs

- 来源：2026-05-11-codex-dotbb-workspace-root.md

# 下一步

- Desktop sidecar 接入时直接调用 `bb --root <user-data-dir> init --seed <bundled-resource-root>`，随后启动 `bb --root <user-data-dir> http --addr 127.0.0.1:3001`。
- 后续可把 `bb http` 增加静态前端 dist 托管，让 WebView 直接打开同源 `http://127.0.0.1:3001/`。
