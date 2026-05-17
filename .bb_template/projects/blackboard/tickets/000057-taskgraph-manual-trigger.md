+++
id = "000057"
lane = "bbt"
title = "TaskGraph Trigger：手动触发入口（Chat/Web/CLI）"
created_at = "2026-05-14"
updated_at = "2026-05-17"
status = "archived"
area = "TaskGraph"
depends_on = "000056"
kind = "trigger-subtrack"
parent = "000056"
requested_by = "user"
scope = "manual-trigger"
+++

# 当前进展

- 2026-05-14：从 Trigger System 拆出手动触发入口子单。

- 2026-05-17:???????????? Chat/FAB -> intent resolver -> graph inputs -> task run ????????

# 记录

- 目标：把 Chat FAB、Web 按钮、Terminal CLI 三类人工入口统一成 TaskIntent/RunRequest。
- 边界：手动入口负责收集 goal、project、worktree、context_refs、template_hint、permission_policy、notify_policy，不直接绕过 Compile/Execution。
- Chat 特例：LLM 可以辅助解析意图和推荐模板，但最终必须产出结构化 TaskIntent，再交给 Task Creation & Compile。

- ????:????????? FAB ?? Runtime?Agent Profile?Graph ???????,? pre-start intent resolver ??? graph inputs,??? run????? depends_on=000056?

# 下一步

- 定义 POST /task-intents 或 /task-runs 的最小请求体。
- 讨论 Chat FAB 里如何选择 Run Now / Create Workflow / Ask Only。
- 讨论 CLI 命令形态，例如 bb task run --project devkit --goal ...。

- ?????????;???????????,?????,????????
