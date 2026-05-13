+++
id = "000034"
lane = "bbd"
title = "Task Graph 前端：抽取通用 Graph Canvas 基底"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
deliverable = "bb_web/src/components/GraphCanvas.vue; bb_web/src/components/TaskGraphCanvasFixture.vue"
depends_on = "000029"
kind = "frontend"
parent = "000028"
+++

# 当前进展

- 交付目标：从现有 TicketDependencyGraph 中抽取可复用 Graph Canvas 基底，使 Ticket Graph 与 Task Graph 共用拖拽、连线、缩放、布局和选择态能力。
- 抽取范围：SVG canvas、pan/zoom、node drag、pin drag edge、edge hover/delete、auto layout、layout persistence、viewport fit。
- 兼容要求：Ticket Graph 现有依赖编辑、pin 交互、缩放与布局保存不能退化。
- Task Graph 适配：Canvas 需要支持不同 node renderer、edge label、node status ring、readonly mode、invalid edge feedback。
- 并行要求：本单可基于契约 fixture 和现有 graph 独立推进，不阻塞后端 API。

- 已抽取 bb_web/src/components/GraphCanvas.vue 作为通用 SVG Graph Canvas 基底，承接 pan/zoom、节点拖拽、pin 连线、edge hover/remove、viewport fit 与布局保存入口。
- 已将 TicketDependencyGraph.vue 迁移到 GraphCanvas，业务层只保留 ticket dependency 计算、保存与循环校验。
- 已补 TaskGraphCanvasFixture.vue，用契约样例覆盖 LLM 节点、分支节点、循环节点、edge label 与只读状态的 Task Graph 渲染需求。
- 已修复迁移期间旧 Dependency Graph/List 不渲染的回归：清除残留旧画布函数与失效 viewport 引用。

- Task Graph cleanup: 完成 000034：抽取 `bb_web/src/components/GraphCanvas.vue`，把 pan/zoom、节点拖拽、pin 连线、edge hover/remove、viewport fit 与布局保存入口沉到通用 Graph Canvas。
- Task Graph cleanup: 迁移 `bb_web/src/components/TicketDependencyGraph.vue` 到新 GraphCanvas，保留 ticket dependency 计算、保存与循环校验在业务层。
- Task Graph cleanup: 新增 `bb_web/src/components/TaskGraphCanvasFixture.vue`，用契约样例覆盖 LLM 节点、分支节点、循环节点、edge label 与只读状态渲染。
- Task Graph cleanup: 修复迁移中造成的旧 Dependency Graph/List 不渲染回归，清掉残留旧画布函数与失效 viewport 引用。

# 记录

- 这是前端并行线的基础单，后续 Task Graph editor/inspector/run UI 都依赖它。
- 验收标准：Ticket Graph 改用抽出的 canvas 后视觉和交互保持一致；Task Graph 可用 fixture 渲染节点和边。
- 验证要求：npm run build --prefix bb_web 通过；browser-use 冒烟 Ticket Graph 节点拖拽、连线、缩放、重排。

- 验证：npm run build --prefix bb_web 通过。
- 验证：browser-use 打开 http://127.0.0.1:8060/#/projects/blackboard/tickets/graph，渲染 1 个 graph canvas、18 个节点、20 条边，控制台无 error。
- 验证：browser-use 打开 http://127.0.0.1:8060/#/projects/blackboard/tickets/list，渲染 1 个 ticket table、18 行，控制台无 error。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-canvas-foundation.md。

# 下一步

- 设计 GraphCanvas 组件 props/events 与 node/edge renderer slot。
- 迁移 TicketDependencyGraph 的通用交互逻辑。
- 保留 Ticket Graph 的业务依赖保存逻辑在外层组件。
- 添加 Task Graph fixture demo 以验证通用 canvas。

- 后续 035/036 可基于 GraphCanvas 的 slot/events 接入 Task Graph catalog、editor inspector 与控制流校验。
