+++
id = "000028"
lane = "bbp"
title = "Task Graph：可编排 Agent 工作流总单"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000018"
kind = "umbrella"
parent = "000018"
+++

# 当前进展

- 需求：接在 000018 本地 Agent Runtime Manager 能力底座之后，作为 Task Graph 系统总单承载后续拆分。
- 核心方向：复用现有 Graph 基底，引入 LLM / registered task / human gate 等可拖拽节点，把 bb daemon 的单点任务串成可编排工作流。
- 边界：Task Graph 表示执行编排关系，Ticket Graph 继续表示工单依赖关系；两者可互相引用，但不直接复用 ticket depends_on 语义。

- 2026-05-09：已拆分 Task Graph MVP 子单：契约 000029；后端 000030/000031/000032/000033；前端 000034/000035/000036/000037；集成验收 000038。
- 2026-05-09：拆分原则为契约先行、前后端并行、集成验收收口；每个子单按胖 ticket 写入交付目标、范围、验收标准与验证要求。

- Task Graph cleanup: 创建 000029 前后端契约 ticket，作为 Task Graph MVP 前后端并行工作的共同依赖。
- Task Graph cleanup: 创建后端子单 000030/000031/000032/000033，覆盖 graph store/API、run state 与 workflow interpreter。
- Task Graph cleanup: 创建前端子单 000034/000035/000036/000037，覆盖 Graph Canvas、Catalog、Editor/Inspector 与 Run UI。
- Task Graph cleanup: 创建 000038 集成验收单，用内置样例工作流收口 MVP。

- Task Graph cleanup: 通过 bb stdio MCP `create_ticket` 创建 `000028` Task Graph 总单。
- Task Graph cleanup: 将 `000028` 放在 `000018` 之后，写入 `depends_on = "000018"` 与 `parent = "000018"`。
- Task Graph cleanup: 在 `000028` 中记录 Task Graph 总方向：复用 Graph 基底，引入 LLM / registered task / human gate 节点，串联 bb daemon 单点任务。
- Task Graph cleanup: 在 `000028` 下一步中预留数据模型、Graph Canvas 抽取、daemon executor、节点最小闭环等后续拆单方向。

# 记录

- 来源：2026-05-09 用户讨论 000018 与 bb daemon 工作成果后，确认将 Graph 基底与 daemon runtime/task registry 串成 Task Graph 系统。

- MVP 依赖关系：000029 -> 后端并行线(000030/000032) 与前端并行线(000034/000035)；后端执行器 000033 依赖 API 与 run state；集成验收 000038 依赖执行器、编辑器和 Run UI。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-mvp-ticket-split.md。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-umbrella.md。

# 下一步

- 拆分 Task Graph 数据模型与 project-local 存储设计。
- 拆分 Graph Canvas 基底抽取与 Task Graph UI 节点编辑器。
- 拆分 daemon Task Graph executor、node run state 与运行日志。
- 拆分 LLM 节点 / registered task 节点 / human gate 节点的最小闭环。

- 优先推进 000029，输出前后端共同使用的 schema、API、run state fixture。
- 契约稳定后，后端从 000030/000032 并行开始，前端从 000034/000035 并行开始。
