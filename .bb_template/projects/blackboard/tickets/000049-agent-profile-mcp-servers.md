+++
id = "000049"
lane = "bbt"
title = "Agent Profile — MCP Servers：per-agent MCP 服务器声明与注入"
created_at = "2026-05-12"
updated_at = "2026-05-13"
status = "done"
area = "AgentProfile"
depends_on = "000048"
parent = "000048"
+++

# 当前进展



# 记录

- 从 048 Agent Profile 数字身份牌中拆出的后继 ticket
- 为每个 Agent 声明其可用的 MCP server 列表，daemon 启动时自动注入对应的 --mcp-config
- 参考 Multica 的 mcp_servers 配置模式：per-agent 声明 + daemon 注入

# 下一步

- 设计 agents.toml 中 mcp_servers 字段 schema（server name + transport + env）
- 后端 AgentProfile 模型扩展 mcp_servers 字段
- daemon 从 Profile 读取 mcp_servers 并生成 --mcp-config JSON
- MCP 工具 upsert_agent schema 支持 mcp_servers
- 前端类型同步
