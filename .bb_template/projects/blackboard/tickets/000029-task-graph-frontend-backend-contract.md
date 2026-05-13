+++
id = "000029"
lane = "bbp"
title = "Task Graph 前后端契约：模型、API 与运行态协议"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
assignee = "codex"
deliverable = "wiki/architecture/task-graph-mvp-contract.md"
depends_on = "000028"
kind = "contract"
parent = "000028"
+++

# 当前进展

- 交付目标：定义 Task Graph MVP 的前后端共同契约，使后端可独立实现存储/执行，前端可并行用 mock/fixture 开发编辑器与运行态 UI。
- 契约范围：System Graph 与 Project Graph 的 catalog 语义、graph definition JSON schema、node/edge/control-flow schema、run snapshot/run context schema、错误码与只读/可编辑规则。
- 控制流范围：MVP 将 Task Graph 定义为受约束 Control Flow Graph，而不是纯 DAG；Start/End/Branch/Loop/Human Gate 是基础控制流节点。
- 系统图规则：BB 内置 system graph 只读、可直接运行、可 fork/customize 到 project graph；project graph 可编辑并归属于当前 project。
- 运行规则：每次 run 必须冻结 graph.snapshot.json，历史运行不受 system graph 后续升级影响。
- 并行规则：前端所有编辑与展示可以先对齐本契约 fixture；后端 API 必须以本契约作为兼容目标。

- 2026-05-09：已完成 Task Graph MVP 前后端契约文档，交付物位于 projects/blackboard/wiki/architecture/task-graph-mvp-contract.md。
- 2026-05-09：契约覆盖 system/project graph 分层、graph definition、节点/边模型、Branch/Loop 控制流、API、run snapshot/run context、错误模型和三组 JSON 示例。
- 2026-05-09：已更新 projects/blackboard/wiki/index.md，将 Task Graph MVP 契约纳入 blackboard wiki 导航。

- Task Graph cleanup: 完成 000029 Task Graph MVP 前后端契约交付物。
- Task Graph cleanup: 新增 wiki/architecture/task-graph-mvp-contract.md，覆盖 system/project graph、graph definition、控制流节点、API、run state、错误模型和 JSON 示例。
- Task Graph cleanup: 更新 wiki/index.md，将 Task Graph MVP 契约加入 Blackboard Wiki 导航。
- Task Graph cleanup: 通过 bb stdio MCP 将交付物路径写回 000029，并将 000029 状态更新为 done。

# 记录

- 来源：000028 Task Graph 总单要求前后端可并行推进，并用独立契约 ticket 串联两边工作。
- 验收标准：产出一份可落地的数据/API/状态协议，包含至少一个 system graph 示例、一个 project graph 示例和一个 run snapshot 示例。
- 验证要求：契约样例应能被后端单测读取校验，也能被前端 mock 数据直接渲染。

- 交付产物：wiki/architecture/task-graph-mvp-contract.md。后续 000030-000038 均以该契约作为前后端并行开发依据。
- 关键约束：Task Graph 是受约束 CFG；system graph 只读且可 fork；project graph 可编辑；run 永远保存 graph.snapshot.json；Branch deterministic；Loop 必须 max_iterations 并只允许受控回边。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-contract-delivery.md。

# 下一步

- 确定 graph definition 顶层字段：id、scope、title、version、nodes、edges、layout、metadata、origin。
- 确定 node types：start、end、llm、registered_task、human_gate、branch、loop。
- 确定 API 形态：catalog list、graph CRUD、system fork、run create、run read。
- 确定 run status 与 node status 枚举，并写清 Branch decision 与 Loop iteration 的记录方式。

- 000030/000031/000032/000033 按契约实现后端存储、API、运行态与 executor。
- 000034/000035/000036/000037 按契约实现 Graph Canvas、Catalog、Editor 与 Run UI。
