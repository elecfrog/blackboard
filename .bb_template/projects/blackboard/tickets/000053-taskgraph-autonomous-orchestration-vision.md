+++
id = "000053"
lane = "bbp"
title = "TaskGraph Vision：多 Runtime 自主协作执行图"
created_at = "2026-05-14"
updated_at = "2026-05-14"
status = "todo"
area = "TaskGraph"
depends_on = "000051 000052"
kind = "vision-umbrella"
requested_by = "user"
scope = "autonomous-taskgraph-orchestration"
+++

# 当前进展

- 2026-05-14：建立 TaskGraph 新一轮平台级 Vision，作为后续拆分的北极星。

# 记录

- Vision：用户只说几句话，TaskGraph 把任务拆成可恢复、可审计、可验收的多 Runtime 协作流；用户离开也能推进，回来只需要看结果、批权限、验收。
- 定义：TaskGraph = 围绕一个 project/ticket/worktree 的多 runtime 协作执行图。
- 产品定位：Chat FAB 是入口和观察窗；TaskGraph 是任务状态机；Runtime 是执行器；Permission 是安全边界；Artifact 是验收材料；Checkpoint 是可恢复性。
- 关键差异化：不复刻单一 Agent Chat，而是在 Project、Ticket、Worktree、Runtime、Session、Permission、Artifact 之间建立长期协作秩序。
- 目标体验：用户给出目标后可以离开，系统自动推进到下一个需要人判断、授权或验收的位置。

# 下一步

- 围绕最小闭环拆分子 Ticket：任务创建入口、Runtime 能力模型、Permission/Interrupt、Session/Checkpoint、Artifact/Run Timeline、验收 UI。
- 先定义 TaskGraph run 的状态模型和验收协议，再决定第一阶段接 OpenCode、Shell、Codex 的执行顺序。
- 讨论第一阶段模板：Explorer → Implementer → Verifier → Reviewer → Handoff。
