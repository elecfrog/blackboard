+++
id = "000030"
lane = "bbt"
title = "Task Graph 后端：System/Project Graph 存储与校验"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000029"
kind = "backend"
parent = "000028"
+++

# 当前进展

- 交付目标：在 bb_core 中实现 Task Graph 领域模型、system graph registry、project graph store 和结构化校验，作为后续 API 与 executor 的基础。
- 存储范围：system graph 从 repo 级内置目录读取；project graph 从 projects/<project>/task_graphs/ 读取和写入。
- 校验范围：graph id/node id/edge id 合法，边端点存在，node config 与 type 匹配，registered_task 引用 agents/tasks.toml 中存在的 task，runtime/agent 字段合法。
- 控制流校验：允许 Branch/Loop 表达受控回边；禁止无法解释的任意环；Loop 必须配置 max_iterations、body_entry/body_exit 或等价边界。
- 安全边界：不接受 raw Markdown 或任意文件 patch；所有写入通过结构化 API 完成。

- 2026-05-09：新增 bb_core/src/task_graph/ 模块（mod.rs, types.rs, store.rs, validation.rs, tests.rs），types.rs 含 TaskGraphDefinition/Node/Edge/NodeConfig/Error/Summary，store.rs 支持 system graph 只读与 project graph CRUD（含版本递增），validation.rs 实现 13 条控制流规则 + node config 类型匹配 + task_id/runtime 引用校验，27 个单元测试全通过
- 2026-05-09：新增 task_graphs/system/frontend-smoke-loop.json fixture，在 lib.rs 注册 pub mod task_graph

- Task Graph cleanup: 新增 `bb_core/src/task_graph/` 模块目录（mod.rs, types.rs, store.rs, validation.rs, tests.rs）
- Task Graph cleanup: types.rs: TaskGraphDefinition, TaskGraphNode(7类), TaskGraphEdge, typed NodeConfig structs, TaskGraphError, TaskGraphSummary, KNOWN_RUNTIMES
- Task Graph cleanup: store.rs: list/read system graphs (只读), list/read/save/delete project graphs (含版本递增)
- Task Graph cleanup: validation.rs: 13条控制流规则 + node config 类型匹配 + task_id/runtime 引用校验 + graph/node ID 格式校验

- Task Graph cleanup: 新增 bb_core/src/task_graph/ 模块目录（mod.rs, types.rs, store.rs, validation.rs, tests.rs）
- Task Graph cleanup: types.rs: TaskGraphDefinition, TaskGraphNode(7类), TaskGraphEdge, NodeConfig structs, TaskGraphError, TaskGraphSummary
- Task Graph cleanup: store.rs: list/read system graphs, list/read/save/delete project graphs, version bump on update
- Task Graph cleanup: validation.rs: 13条控制流规则 + node config 类型匹配 + task_id/runtime 引用校验 + graph/node ID 格式校验

# 记录

- 后端可以在契约样例定稿后独立实现本单，不依赖前端代码。
- 验收标准：能列出 system/project graph，能保存 project graph，能拒绝非法节点、非法边、非法 task 引用和不受控循环。
- 验证要求：补 bb_core 单测覆盖 system graph read、project graph write、schema validation、control-flow validation。

- inbox: 2026-05-09-codebuddy-task-graph-store-validation.md
- bb_core/src/task_graph/ (mod.rs, types.rs, store.rs, validation.rs, tests.rs)
- task_graphs/system/frontend-smoke-loop.json

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codebuddy-task-graph-store-validation.md。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codebuddy-task-graph-后端-store-validation-实现.md。

# 下一步

- 新增 TaskGraph、TaskGraphNode、TaskGraphEdge、TaskGraphScope、TaskGraphValidationError 等 core 类型。
- 实现 system graph registry 读取与 project graph store CRUD。
- 实现控制流合法性校验，包括 Branch 出口与 Loop 上限。
- 补充 fixtures，复用契约 ticket 中的 system/project graph 样例。
