+++
id = "000048"
lane = "bbt"
title = "Agent Profile 数字身份牌：显式注册完整 Agent 运行时配置"
created_at = "2026-05-11"
updated_at = "2026-05-16"
status = "archived"
assignee = "codebuddy"
blocks = "000010"
parent = "000005"
+++

# 当前进展



# 记录

- 需求来源：用户认为 010 (Agent 工作台视图) 的前置定义不完整——当前 agents.toml 只记录了 id/kind/runtime/roles 等元数据，缺少将 Agent 真正"跑起来"所需的完整运行时配置。
- 目标：为每个 Agent 建立"数字员工身份牌"，显式注册其完整 Profile，包括：
- 1. Model 绑定 — 该 Agent 默认使用的 LLM model（如 minimax/MiniMax-M2.7-highspeed）
- 2. Runtime 绑定 — 执行器类型与路径（opencode / codex / claude）
- 3. System Prompt — 该 Agent 的系统 prompt 文件路径或内联内容
- 4. Tool 权限 — 该 Agent 可使用的 bb_* MCP 工具白名单或黑名单
- 5. Skill 列表 — 该 Agent 可调用的 skill 集合
- 6. 组织关系 — 上级(supervisor)、下级(subordinates)、同事(peers) 的 agent id 引用
- 7. 调度约束 — 并发限制、超时、重试策略等运行时参数
- 示例：BB-PM = Minimax + OpenCode + bb-pm.md prompt + inbox/ticket 工具权限 + pm skills + supervisor=owner, peers=[codex,codebuddy]
- 前置条件：daemon 已重构为 Task Graph headless runner（已完成），interpreter_config 已支持 overrides 透传（已完成）
- 与 010 的关系：010 (Agent 工作台视图) 的 UI 层将消费本 ticket 产出的 Profile 数据，展示完整的 Agent 身份信息

# 下一步

- 设计 agents.toml schema v2：在现有字段基础上扩展 [agents.profile] 子表，定义 model/prompt/tools/skills/org 字段
- 实现 bb_core AgentProfile 结构体扩展，支持读写新字段
- 实现 HTTP API 和 MCP 工具对 Profile 字段的 CRUD
- 实现 daemon/Task Graph LLM 节点从 Profile 自动解析 InterpreterOverrides
- 前端 Agent 工作台 (010) 接入 Profile 数据展示
