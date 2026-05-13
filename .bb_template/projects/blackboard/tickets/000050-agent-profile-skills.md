+++
id = "000050"
lane = "bbt"
title = "Agent Profile — Skills：per-agent 技能列表声明与注入"
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
- 为每个 Agent 声明其可用的 Skill 列表（技能名 + 描述 + 触发条件）
- Skill 是可动态加载的指令/脚本/资源集合，提升 Agent 在特定任务上的表现
- 参考 Multica 的 skills 配置模式 + Blackboard 现有 sub-agent skill 概念

# 下一步

- 设计 agents.toml 中 skills 字段 schema（skill id + description + trigger）
- 后端 AgentProfile 模型扩展 skills 字段
- daemon 从 Profile 读取 skills 并注入到 Agent 运行时上下文
- MCP 工具 upsert_agent schema 支持 skills
- 前端类型同步
- 与 OpenCode sub-agent 的 skill 机制对齐
