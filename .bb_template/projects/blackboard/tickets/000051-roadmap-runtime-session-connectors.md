+++
id = "000051"
lane = "bbt"
title = "Roadmap：Runtime 连接与 AgentSession 维护"
created_at = "2026-05-13"
updated_at = "2026-05-13"
status = "done"
+++

# 当前进展


- codex 完成阶段工作，handoff 写入 `2026-05-13-codex-codex-agent-session-runtime.md`。

- agent 开始执行：Implement CodeBuddy CLI TaskGraph AgentSession runtime path.

- codex 完成阶段工作，handoff 写入 `2026-05-13-codex-codebuddy-cli-taskgraph-agentsession-runtime.md`。

# 记录

- tickets 系统强化暂时放轻，优先建设 Runtime 连接、Session 维护和结构化事件底座。
- 第一优先级 runtime 是 OpenCode 和 Codex；后续 Claude Code / Gemini 预留扩展点，具体接入可交给开源社区。
- 目标是让 AgentSession 成为独立于 TaskGraph 的 runtime/session 管理层，承接实时事件、resume、usage、tool replay 和 provider event normalization。

- 验证：cargo test --manifest-path bb_backend/Cargo.toml agent_session passed.
- 验证：cargo test --manifest-path bb_backend/Cargo.toml task_graph passed.
- 验证：npm run build --prefix bb_web passed.
- 验证：Real Codex TaskGraph smoke with gpt-5.4-mini + low persisted status/text/usage_update events.

- 验证：cargo test --manifest-path bb_backend/Cargo.toml agent_session: passed.
- 验证：cargo test --manifest-path bb_backend/Cargo.toml task_graph: passed (106 tests; existing unused-variable warning in validation/pre_run.rs).
- 验证：cargo test --manifest-path bb_backend/Cargo.toml codebuddy: passed (15 tests).
- 验证：npm run build --prefix bb_web: passed with existing Vite chunk-size warning.
- 验证：git diff --check on touched CodeBuddy/TaskGraph/frontend files: passed; only CRLF normalization warnings.
- 验证：Real CodeBuddy direct captures: pure text, PowerShell tool call, and bb MCP list_projects all succeeded before TaskGraph smoke.
- 验证：cargo build -p bb_cli default target was blocked by running bb.exe lock; rebuilt with --target-dir bb_backend/target-codebuddy-smoke and removed that temporary target after smoke.

# 下一步

- 梳理 Runtime Provider 接口：启动参数、环境注入、MCP/skills 注入、事件解析、取消/超时和结果归一化。
- 补齐 OpenCode / Codex 两条 runtime 的能力矩阵，明确哪些走 AgentSession structured timeline，哪些暂留 legacy log。
- 设计 AgentSession resume / usage / tool replay 的持久化与 HTTP/SSE 读取边界。
