+++
id = "000052"
lane = "bbt"
title = "Roadmap：内置 Agent Chat 与任务下发"
created_at = "2026-05-13"
updated_at = "2026-05-13"
status = "in_progress"
+++

# 当前进展



# 记录

- 近期 roadmap 调整：Blackboard 需要补齐自身 Agent 能力，在系统内支持直接 chatting 和任务下发，而不是只能通过 TaskGraph 间接触发。
- Chat 需要建立在 Agent Profile + AgentSession 之上：用户选择 agent/profile，发起会话，持续接收结构化事件、usage 和 tool 结果。
- 任务下发要能连接 tickets、inbox、TaskGraph 和 AgentSession，让用户可以把一次 chat、一个 ticket 或一个小任务交给合适的 Agent 执行。

# 下一步

- 定义 Direct Chat 的最小产品形态：入口、Agent/Profile 选择、session 创建、消息流、历史读取和取消。
- 设计任务下发模型：从 ticket/inbox/task graph/chat 创建 AgentSession 或 TaskGraph run 的边界。
- 明确 UI 信息架构：Chat workspace、ticket 内下发入口、Agent run history 与 structured timeline 的复用关系。
