+++
id = "000044"
lane = "bbq"
title = "Rust core 模块清理：Agent MCP connector 归位"
created_at = "2026-05-10"
updated_at = "2026-05-10"
status = "archived"
assignee = "codex"
+++

# 当前进展

- codex 开始梳理 core/cli MCP 命名边界，目标是把 Agent MCP 配置注入语义从 core 顶层 mcp_config 移入 agents_config 语义范围。

- codex 开始执行：确认本轮按 BBQ 票据 000044 执行，重点是 Rust 模块移动/重命名，不扩大到前端或 ticket 数据清理。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-rust-core-agent-mcp-connector-模块清理.md`。

- 2026-05-10：将 bb_core/src/mcp_config.rs 移入 agents_config/mcp_connector.rs，测试迁入 agents_config/mcp_connector/tests.rs；收紧类型命名为 AgentMcpConfigFormat/AgentMcpServerConfig/AgentMcpTransport/AgentMcpServerTarget；调用点改为 mcp_connector::{inspect,upsert,remove}_server

# 记录


- 验证：`cargo fmt --manifest-path bb_backend/Cargo.toml --all` 通过。
- 验证：`cargo check --manifest-path bb_backend/Cargo.toml --workspace` 通过。
- 验证：`cargo test --manifest-path bb_backend/Cargo.toml --workspace` 通过：bb_cli 63 tests，bb_core 189 tests，doc-tests 0。
- 验证：`git diff --check -- bb_backend/crates/bb_core/src/agents_config.rs bb_backend/crates/bb_core/src/agents_config bb_backend/crates/bb_core/src/lib.rs` 通过。
- 验证：旧入口扫描无命中：`mcp_config`、`McpConfigFormat`、`McpServerConfig`、`McpServerTarget`、`McpTransport`、旧函数名均未残留。

- 来源：inbox/2026-05-10-codex-rust-core-agent-mcp-connector-模块清理.md
- 相关路径：bb_backend/crates/bb_core/src/agents_config.rs, bb_backend/crates/bb_core/src/agents_config/mcp_connector.rs, bb_backend/crates/bb_core/src/lib.rs

# 下一步

- 移动/重命名 Rust 模块，修正引用与导出。
- 运行 cargo fmt、cargo check、cargo test 验证 workspace。

- 后续如继续做模块形态统一，可再单独评估是否把 `agents_config.rs`、`ticket.rs`、`wiki.rs` 等 hybrid module 根文件迁成 `mod.rs`，避免与当前大规模未提交拆分混在一张票里。
