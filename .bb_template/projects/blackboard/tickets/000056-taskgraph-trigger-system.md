+++
id = "000056"
lane = "bbp"
title = "TaskGraph Trigger System：统一任务触发层"
created_at = "2026-05-14"
updated_at = "2026-05-14"
status = "todo"
area = "TaskGraph"
depends_on = "000053"
kind = "platform-track"
parent = "000053"
requested_by = "user"
scope = "trigger-system"
+++

# 当前进展

- 2026-05-14：从 TaskGraph Vision 拆出 Trigger System 主干 ticket。

# 记录

- 目标：把 Chat FAB、Ticket、Inbox、Hook、Schedule、Watch、External Webhook、Agent Follow-up 等入口统一成 TriggerEvent/TaskIntent。
- 边界：Trigger 层只回答为什么现在启动、携带什么上下文、允许自动化到什么程度、完成后通知谁；不直接执行任务。
- 核心输出：TaskIntent，包含 source、project、worktree、actor、goal、payload、template_hint、permission_policy、notify_policy。

# 下一步

- 讨论 TriggerEvent 与 TaskIntent 的最小字段。
- 确定第一阶段只做 Manual Chat Trigger，还是同时预留 Hook/Schedule。
- 定义 Trigger 到 Task Creation/Compile 的交接契约。
