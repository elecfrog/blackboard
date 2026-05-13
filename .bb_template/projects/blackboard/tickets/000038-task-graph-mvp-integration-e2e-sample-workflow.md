+++
id = "000038"
lane = "bbq"
title = "Task Graph MVP 集成验收：内置样例工作流端到端跑通"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
assignee = "codebuddy"
depends_on = "000033 000036 000037"
kind = "quality"
parent = "000028"
+++

# 当前进展

- 交付目标：用一个 BB 内置 system graph 样例完成 Task Graph MVP 端到端验收，覆盖 catalog、customize、编辑、运行、branch、loop、human gate、日志和 artifact。
- 样例工作流：Start -> LLM 生成检查计划 -> registered_task frontend-smoke-test -> Branch 判断失败 -> LLM 生成修复建议 -> Human Gate -> Loop 最多重试 3 次 -> End。
- 端到端范围：system graph 可直接运行；system graph 可 fork 成 project graph；project graph 可编辑保存；run snapshot 可追溯；run UI 展示状态。
- 回归范围：Ticket Graph 依赖编辑、Tickets 三视图、daemon 现有 tasks.toml 单点任务不退化。

- 2026-05-09：完成 Task Graph MVP E2E 验收实施，新增 run API HTTP 验收测试
- 2026-05-09：新增 architecture/task-graph-mvp-e2e-validation.md 并更新 wiki index
- 2026-05-09：完成 core/interpreter、HTTP API、前端 build 和 8060 浏览器端到端 smoke 自测
- 2026-05-09：保持 status 为 review，未转 done

- Task Graph cleanup: 完成 038 Task Graph MVP 端到端验收实施，新增 run API create/list/read/cancel HTTP 验收测试。
- Task Graph cleanup: 新增 `architecture/task-graph-mvp-e2e-validation.md`，沉淀 frontend-smoke-loop 样例工作流验收记录。
- Task Graph cleanup: 更新 blackboard wiki index，将 Task Graph MVP E2E 验收记录纳入导航。
- Task Graph cleanup: 完成 core/interpreter、HTTP API、前端 build 和 8060 浏览器端到端 smoke 自测。

# 记录

- 本单是 000028 MVP 的收口单，只在前后端核心能力合并后开始。
- 验收标准：用户可以不写代码完成一次 Task Graph 的查看、customize、运行和结果查看；失败/暂停/循环上限均有可读反馈。
- 验证要求：python3 scripts/check_ticket_ids.py --project blackboard、qmd embed、npm run build --prefix bb_web、后端相关 cargo test、browser-use 端到端冒烟均通过。

- 来源：inbox/2026-05-09-codex-task-graph-mvp-e2e-review.md

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-mvp-e2e-review.md。

# 下一步

- 准备 system graph 样例与 project graph fixture。
- 补端到端 smoke checklist。
- 联调后端 run API 与前端 run UI。
- 把验收结论沉淀回 000028 和本单记录。
