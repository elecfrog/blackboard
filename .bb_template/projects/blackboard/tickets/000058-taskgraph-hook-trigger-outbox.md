+++
id = "000058"
lane = "bbt"
title = "TaskGraph Trigger：Hook 触发与内部事件 Outbox"
created_at = "2026-05-14"
updated_at = "2026-05-14"
status = "todo"
area = "TaskGraph"
depends_on = "000056"
kind = "trigger-subtrack"
parent = "000056"
requested_by = "user"
scope = "hook-trigger"
+++

# 当前进展

- 2026-05-14：从 Trigger System 拆出 Hook 触发与内部事件子单。

# 记录

- 目标：支持 ticket_created、ticket_status_changed、inbox_note_created、agent_session_finished、task_run_failed 等内部事件触发 TaskGraph。
- 第一阶段场景：inbox 创建后触发 bb-pm 清理/归档流程；ticket 创建后可触发 triage 或默认上下文补全流程。
- 实现原则：业务写入只追加 TriggerEvent/Outbox，不同步执行 Agent；dispatcher 异步消费事件、去重、创建 TaskIntent/RunRequest。
- 外部文件系统监听可复用 notify，但内部写操作优先走结构化 outbox，避免平台差异和编辑器事件噪音。

# 下一步

- 定义 TriggerEvent outbox 的存储位置、schema、状态字段和去重 key。
- 确定 inbox_note_created -> bb-pm cleanup 的第一条 hook demo。
- 讨论 hook 规则 UI：事件类型、过滤条件、目标 TaskGraph、并发/重试策略。
