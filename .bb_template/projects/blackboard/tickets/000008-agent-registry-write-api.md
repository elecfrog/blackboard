+++
id = "000008"
lane = "bbt"
title = "Agent Registry 写入 API"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000006"
parent = "000005"
+++

# 当前进展

- 2026-05-07：Agent Registry 写入能力已落地并验收，`agents/agents.toml` 继续作为注册事实源。
- 2026-05-07：HTTP 与 MCP 写接口已覆盖全局 agent upsert、project membership upsert/removal，前端不再拼 TOML。
- 2026-05-07：项目 assignee 下拉已接入 `/api/projects/{project}/agents`，ticket assignee 更新仍走受控 `patch_ticket`。
- 2026-05-07：本轮复核后将 ticket 状态从 `review` 更新为 `done`。

- 2026-05-06：codex 验收 check-ticket-ids.sh 通过，确认 per-project DAG 门禁生效

# 记录

- 范围：为 `agents/agents.toml` 提供结构化写入入口，覆盖 agent profile 与 project-agent membership，避免前端或 Agent 手拼 TOML。
- 后端实现：`bb_core::agents_registry` 提供 `upsert_agent`、`upsert_project_agent`、`remove_project_agent`、`list_agents`、`list_project_agents`，并校验 project、agent id、lane、必填字符串和重复项。
- HTTP 实现：`POST /api/agents`、`POST /api/projects/{project}/agents`、`DELETE /api/projects/{project}/agents/{id}` 已接入；`GET /api/agents` 与 `GET /api/projects/{project}/agents` 用于前端读取。
- MCP 实现：`list_agents`、`upsert_agent`、`upsert_project_agent`、`remove_project_agent` 已暴露给 bb-server stdio 工具，除全局 list 外均保留显式 project 约束。
- 前端消费：Settings 的 Agent Registry 区块读取 registry 统计；Ticket detail/Board assignee 控件读取 project agents 并保存稳定 agent id。
- 兼容行为：project agents 列表会合并显式 project membership 与全局 assignable agent，历史 assignee 仍可显示为 legacy 值。

- inbox/2026-05-06-codex-bb-mcp-frontend-rdg-验收.md 来源：codex 夜间开发验收 handoff

# 验证

- `cargo test -p bb_server rest_agent_registry_write_roundtrips_project_agents`：通过，覆盖 REST 写入 agent、写入/删除 project membership 并重新读取。
- `GET http://127.0.0.1:3001/api/agents`：通过，live bb-server 返回 registry 与 `project_agents`。
- `GET http://127.0.0.1:3001/api/projects/blackboard/agents`：通过，live bb-server 返回 blackboard 可分配 agent 列表。
- `npm run build --prefix bb_web`：通过，确认 assignee 下拉与相关前端类型仍可构建。
- `scripts/check-ticket-ids.sh`：通过，确认本次 ticket 状态更新后 ID/frontmatter 一致。

# 风险

- 当前 live `bb-server` 仍是旧进程，`/api/agents` 的 `source_path` 仍显示 Windows `\\?\` 前缀；代码侧展示清洗已修复，需要重启后端服务后才会体现在页面与 API 返回中。

# 下一步

- 无。000008 的写 API 目标已完成，后续 UI/连接器体验问题拆到对应 Settings、Ticket detail 或 Agent workspace ticket 处理。
