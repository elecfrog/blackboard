+++
id = "000027"
lane = "bbt"
title = "Codex Runtime Backend：实现 app-server JSON-RPC 2.0 通信层"
created_at = "2026-05-08"
updated_at = "2026-05-09"
status = "archived"
depends_on = "000018"
parent = "000018"
+++

# 当前进展

- 2026-05-09：完成调研，确认必须走 app-server 路径（exec 不支持 computer-use/browser-use）

- 2026-05-09：发现 codex exec 已支持 computer-use/browser-use（features list 确认 stable+true），不需要走 app-server JSON-RPC
- 2026-05-09：实现 codex runtime backend 走 `codex exec --json -C <dir> -s danger-full-access --ephemeral` 路径
- 2026-05-09：daemon 新增 build_runtime_command helper，根据 tasks.toml 中的 runtime 字段分支到 opencode/codex
- 2026-05-09：CLI 新增 --codex 参数指定 Codex 可执行文件路径
- 2026-05-09：创建 frontend-smoke-test prompt 模板，dry-run 验证通过

- 2026-05-09：将 frontend-smoke-test 从 browser-use 验收改为 Computer Use 驱动的 UI 视觉验收；禁止用 shell/curl/Playwright 替代视觉验收
- 2026-05-09：新增 frontend-smoke-interactive 独立 prompt，保留实验性 Computer Use 视觉冒烟入口
- 2026-05-09：将 frontend-smoke-test 定位调整为 codex exec 一次性前端健康检查，移除对 Computer Use/Browser Use/Playwright 的依赖

# 记录

- 调研来源：Codex 官方 GitHub docs/exec.md → 重定向到 developers.openai.com/codex/noninteractive
- 调研来源：DeepWiki openai/codex 4.2 Headless Execution Mode 文档
- 调研来源：KB multica wiki — agent-backends.md、agent-cli-invocation.md（codex.go 完整实现）

- 关键发现：codex features list 显示 computer_use=stable/true, browser_use=stable/true, unified_exec=stable/true
- 这意味着 codex exec 模式已具备完整能力，无需 app-server JSON-RPC 复杂路径
- Multica 走 app-server 可能是历史原因（unified_exec 当时还未 stable）

- 来源：inbox/2026-05-09-codex-computer-use-smoke-prompt.md
- 来源：inbox/2026-05-09-codex-one-shot-smoke-strategy.md

# 下一步

- 在 daemon.rs 中加 runtime=codex 分支，启动 codex app-server --listen stdio://
- 实现最小 JSON-RPC 2.0 client：initialize → thread/start(developerInstructions) → turn/start(input) → 读 notifications
- 解析 JSONL notifications 映射到统一 AgentRunResult
- 验证 computer-use/browser-use 能力在 app-server 模式下可用
- 编写前端冒烟测试 prompt 模板（agents/prompts/frontend-smoke-test.md）
- 端到端验证：bb daemon --task frontend-smoke --agent codex --project blackboard
