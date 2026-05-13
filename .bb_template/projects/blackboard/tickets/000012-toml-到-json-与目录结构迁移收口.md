+++
id = "000012"
lane = "bbt"
title = "TOML 到 JSON 与目录结构迁移收口"
created_at = "2026-05-07"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000004"
parent = "000004"
+++

# 当前进展

- 2026-05-07：从用户反馈拆出迁移收口单，聚焦 project.toml → __project__.json、tickets/active|done|archived → flat tickets、.ticket-id → 后端索引/分配逻辑的残留清理。
- 2026-05-07：确认 `__tickets__.json.current_counter` 是持久化权威计数；后端只在索引缺失或 `000000` + 已有 tickets 的迁移场景 bootstrap 写回。
- 2026-05-07：完成后端分配逻辑改造：`create_ticket` 不再写 `.ticket-id`，改读 `__tickets__.json.current_counter` 并结合现有最大 ticket ID 防撞。
- 2026-05-07：清理活跃规则和文档里的 `tickets/active|done|archived`、`project.toml`、`export:tickets`、`next-ticket-id.sh` 旧模型引用。
- 2026-05-07：移除 `directory_status` 兼容字段，`__tickets__.json` 与 API 只保留 `bucket`。

# 记录

- 范围：清理活跃规则、README、MCP/REST 描述、脚本、测试、后端缓存维护与运行时 API 中仍指向旧 TOML/目录/计数器模型的内容。
- 原则：能由 bb_backend 在读写时主动维护的索引和计数，不再要求 Agent 或用户运行导出脚本来补事实。
- 实现：`read_ticket_index` 保留非零 `current_counter`，仅在缺失/损坏或迁移残留 `000000` 时重建；`check-ticket-ids.sh` 只检查 `current_counter >= max(ticket id)`。
- 实现：清理 `directory_status`，更新 bb_core/types/tests/stdout fixtures，避免 JSON 索引继续输出废字段。
- 文档：README、AGENTS、MCP 描述和迁移脚本描述已对齐 `__project__.json`、flat `tickets/`、后端运行时索引维护模型。
- 验证：`cargo test -p bb_core`、`cargo test -p bb_server rest_agent_registry_write_roundtrips_project_agents`、`scripts/check-ticket-ids.sh`、`qmd embed`、`npm run build --prefix bb_web` 均通过。

# 下一步

- 重启正在运行的 bb-server，使 Windows `\\?\` 展示清理和新的 `__tickets__.json.current_counter` 计数逻辑进入 live 服务。
- 继续审计历史 inbox/ticket 里的旧路径描述；只在它们会误导当前规则或 UI 时迁移，纯历史记录不批量改写。
