+++
id = "000064"
lane = "bbd"
title = "TaskGraph 前端 Graph 与 Run UI：节点编辑、状态覆盖与 Artifact 面板"
created_at = "2026-05-14"
updated_at = "2026-05-17"
status = "archived"
area = "TaskGraph"
assignee = "codex"
depends_on = "000055"
kind = "execution-subtrack"
parent = "000055"
requested_by = "user"
scope = "frontend-graph-run-ui"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出前端 Graph 与 Run UI 子单。

- codex 开始执行：2026-05-15：codex 接手 000064，先完善 ticket 描述、范围边界与实施计划；已对齐父单 000055 并盘点现有 TaskGraph 前端组件。

- 2026-05-15：已补充 064 的前端执行台定位、当前代码基线、范围边界、第一阶段验收标准与实施计划；本轮只更新 ticket，不改业务代码。

- 2026-05-15：补充总体编辑工作区设计方向：当前 TaskGraph Editor 纵向堆叠 Graph Inputs、Graph 设置、Canvas，导致画布只占页面下半段；064 的编辑态应调整为 canvas-first 工作台。

- 2026-05-15：按两张设计图完成 TaskGraph Editor 第一轮 canvas-first 优化：Graph Inputs/Graph 设置改为顶部摘要条 + 浮层编辑，canvas 成为默认主工作区；节点 palette 改为按类型分组；canvas node 统一骨架并通过色条、icon、type badge、meta 文案区分节点类型；右侧 Node Inspector 改为可收起抽屉。

- 2026-05-15：按 UE Blueprint Pin 风格调整 GraphCanvas pin 渲染：默认控制流 exec_in/exec_out 不再显示 In/Out 文字，改用边缘三角方向 glyph 表达；数据 pin 继续使用彩色圆点并保留 Output/Index/Action 等语义标签。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-task-node-视觉对齐-编辑-预览-运行-fixture-统一节点组件.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-editor-graph-inputs-改为右侧编辑抽屉.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-preview-run-inputs-改为摘要入口与右侧抽屉.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-run-节点状态-footer-避免-output-pin-重叠.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-node-去除左侧-accent-strip.md`。

- codex 开始执行：开始实现 pregel-topology-mutation 前端 spec；用户要求完成后不要跑前端 build，因为后端并行工作。

- codex 完成阶段工作，handoff 写入 `2026-05-16-codex-pregel-topology-mutation-frontend-implementation.md`。

- 2026-05-16：实现 TaskGraph run data contract 前端补充：current_graph_revision、active_nodes、topology mutation result types、pending graph mutation effects、legacy cursor fallback normalize、event/checkpoint normalize、mock Pregel mutation runs、HumanGate resume client

# 记录

- 目标：让前端能够表达节点类型、typed pins、运行状态、错误、事件时间线、artifact、approval/retry/cancel 操作。
- 范围：node palette、node inspector、typed pin 展示、run 状态覆盖、event timeline、artifact panel、approval/retry/cancel 操作。
- 定位：前端不是单纯画图，而是 TaskGraph 可观察、可调试、可验收的操作台。

- 补充任务描述：000064 负责把已有 TaskGraph catalog/editor/preview/run panel 升级为可观察、可调试、可操作的前端执行台；它既要支持图编辑时的节点分类、typed pins 和 inspector 配置，也要支持运行时的状态覆盖、错误定位、artifact 查看、审批/取消/重试等操作入口。
- 当前代码基线：`bb_web/src/data/taskGraphs.ts` 已有 TaskGraphRunDetail、TaskGraphRunNode、paused actions、artifact、cancel/watch/read API 类型与函数；`TaskGraphRunPanel.vue` 已能 watch run、显示 canvas 状态覆盖、节点详情、artifact/log/agent session timeline 与 cancel；`TaskGraphEditorPanel.vue`、`GraphCanvas.vue`、`task-graph/TaskGraphNodePalette.vue`、`TaskGraphNodeInspector.vue` 已具备节点编辑、palette/inspector、typed pin 显示与连接基础。
- 范围边界：本单不重做后端执行状态机、Runtime Session 调度、TaskRun/NodeRun/Event/Artifact 权威模型，也不替代 000060-000063 的 contract；064 以前端消费这些 contract 为主，必要时只补轻量类型适配、空态、错误态和交互状态。
- 依赖关系：状态/事件/API contract 依赖 000060；typed pins 与 artifact contract 依赖 000061；节点分类与配置 schema 依赖 000062；Shell/本地工具节点的 log、exit_code、artifact 呈现依赖 000063。前端实现应允许这些 contract 分阶段落地，用 mock/rest fallback 保持可演示。
- 第一阶段验收口径：palette/inspector/canvas 能按 Control、Runtime、Transform、Artifact、Integration 等分类呈现节点；typed pins 的 exec/data、value_type、required/invalid 连接在画布和错误信息里清晰可见；Run UI 能稳定显示 run/node 状态、cursor、branch active edge、duration、error、log tail、artifact preview 和 child run navigation。
- 第一阶段验收口径：paused/needs approval 场景要有明确的审批 action 区域，后端 action API 未就绪时保持 disabled + 原因说明；cancel 必须沿用现有 API 并有 busy/error 反馈；retry/run-again 在 contract 可用后接入，不用前端臆造执行语义。
- 非目标：不引入新的画布库，不大改现有 catalog/sidebar/preview 信息架构，不把 TaskGraph 前端做成独立路由；优先在现有 `TaskGraphCatalogPanel` 嵌入式体验里增强。

- 总体设计判断：TaskGraph 编辑器的主任务是编排和调试图，默认第一屏必须让 canvas 成为主要工作区；Graph Inputs、Graph 设置、Validation、Palette、Inspector 都应围绕 canvas 成为可折叠、可停靠、可召回的辅助面板。
- 建议布局：左侧 graph catalog 保留但压缩为窄列表；顶部保留 graph title、scope/version、Save/Run/Close 等主操作；中间 canvas 占据剩余宽高并具备稳定最小高度；Node Palette 改为 canvas 左上浮层/抽屉；Inspector 改为右侧可折叠 drawer；Graph Inputs/Settings 改为顶部薄 summary bar + 抽屉编辑，不再长期占用垂直空间。
- 画布可用性标准：默认视图中 canvas 应占主内容区 70% 以上高度；在 1440px 宽桌面下至少能同时看到 4-6 个标准节点和主要连线；新增/选择节点不应让画布跳动；配置面板展开时应压缩或覆盖辅助区，不能把 canvas 挤成小窗口。
- 交互评判标准：画布优先、配置可召回、选择即编辑、状态就地覆盖、辅助信息不抢主工作区、长表单不阻塞编排、窄屏时 drawer/inputs 进入分层抽屉。

- 实现文件：`bb_web/src/components/TaskGraphEditorPanel.vue`、`bb_web/src/components/task-graph/TaskGraphNodePalette.vue`、`bb_web/src/components/task-graph/TaskGraphNodeInspector.vue`、`bb_web/src/components/TaskGraphCatalogPanel.vue`、`bb_web/src/i18n.ts`。
- 布局变化：编辑器由纵向 Graph Inputs + Graph 设置 + canvas 改为 top toolbar + summary strip + full-height canvas；Inputs/Settings 只在点击摘要 chip 时以浮层出现，不再长期占用垂直空间。
- 节点视觉变化：普通节点保留统一 rounded-rect skeleton；差异通过 accent strip、compact icon、type badge、category/meta 文案和 typed pins 呈现；palette 按控制、运行、产物、审批、起止分组。
- 交互变化：节点详情作为右侧 overlay drawer，支持直接收起；左侧 catalog 宽度略收窄，为主 canvas 多让空间。

- 用户反馈：期望用明确图示表示 In/Out，而不是文字；参考 UE Blueprint，执行流 pin 去掉 In/Out 文案，其他语义 pin 标签保留。
- 实现点：`GraphCanvas.vue` 的 semantic pin 渲染增加 category/valueType/color 透传，`visiblePinLabel` 隐藏默认 exec_in/exec_out 标签；exec pin 使用 UE 风格三角 glyph，data pin 保留圆点和文字标签。
- 排查说明：064 主体实现没有被回退；当前不带 `/edit` 的 URL 是预览/查看态，不会出现 canvas-first editor。已切到 `/edit` 路由确认 `Task Graph Editor`、`Graph Inputs` 摘要、`Graph 设置` 摘要、`节点面板`、`控制流校验` 均存在。

- 验证：`npm run build --prefix bb_web`：通过；仅有 Vite 既有 large chunk warning。
- 验证：`git diff --check -- bb_web/src/components/TaskGraphEditorPanel.vue bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue bb_web/src/components/TaskGraphRunPanel.vue bb_web/src/components/TaskGraphCanvasFixture.vue bb_web/src/components/task-graph/TaskGraphNodeShape.vue bb_web/src/components/task-graph/taskGraphNodeVisuals.ts bb_web/src/components/GraphCanvas.vue`：通过；仅 Windows LF/CRLF 提示。
- 验证：in-app browser `/task-graphs/system/frontend-smoke/edit`：可见 Task Graph Editor、节点面板、控制流校验；DOM 统计 `.task-graph-node-shape`=4、`.graph-exec-pin-glyph`=6、`.graph-data-pin-glyph`=2、默认 In 文案=0。
- 验证：in-app browser `/task-graphs/system/frontend-smoke`：预览态 DOM 统计 `.task-graph-node-shape`=4、`.graph-exec-pin-glyph`=6、`.graph-data-pin-glyph`=2。

- 验证：`npm run build --prefix bb_web`：通过；仅有 Vite 既有 large chunk warning。
- 验证：`git diff --check -- bb_web/src/components/TaskGraphEditorPanel.vue bb_web/src/components/task-graph/TaskGraphInputsPanel.vue`：通过；仅 Windows LF/CRLF 提示。
- 验证：in-app browser `/task-graphs/system/kb-wiki-build-workflow/edit`：点击 Graph Inputs 后 `.task-graph-editor-config-panel`=1、`.task-graph-inputs-drawer`=1、input rows=6、node inspector=0、console error=0。

- 验证：`npm run build --prefix bb_web`：通过；仅有 Vite 既有 large chunk warning。
- 验证：`git diff --check -- bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue bb_web/src/components/task-graph/TaskGraphRunInputsPanel.vue`：通过；仅 Windows LF/CRLF 提示。
- 验证：in-app browser cache-bust 访问 `/task-graphs/system/kb-wiki-build-workflow`：默认 `.task-graph-run-inputs-wide`=0、`.task-graph-preview-config-panel`=0；点击 `Graph Inputs 6 个输入` 后 `.task-graph-preview-summary-strip`=1、`.task-graph-preview-config-panel`=1、`.task-graph-run-inputs-drawer`=1、input rows=6、console error=0。

- 验证：按用户要求未运行 `npm run build --prefix bb_web`，避免影响并发修改。
- 验证：`git diff --check -- bb_web/src/components/task-graph/TaskGraphNodeShape.vue bb_web/src/components/TaskGraphRunPanel.vue`：通过；仅 Windows LF/CRLF 提示。

- 验证：按并发要求未运行 `npm run build --prefix bb_web`。
- 验证：`rg -n "task-graph-node-accent-strip" bb_web/src/components bb_web/src/views bb_web/src/styles.css`：无残留匹配。
- 验证：`git diff --check -- bb_web/src/components/task-graph/TaskGraphNodeShape.vue`：通过；仅 Windows LF/CRLF 提示。

- 验证：bb_list_projects equivalent mcp__bb__.list_projects: passed; visible project blackboard found.
- 验证：bb_find_work_context equivalent mcp__bb__.find_work_context(project=blackboard, query=pregel topology mutation frontend implementation spec): passed; relevant active ticket 000064 found.
- 验证：bb_begin_ticket_work equivalent mcp__bb__.begin_ticket_work(project=blackboard, id=000064): passed; ticket moved to in_progress and consistency checks passed.
- 验证：npm exec vue-tsc -- -p tsconfig.app.json --noEmit from bb_web: passed.
- 验证：rg "run\\.cursor|cursor_before|cursor_after|run-cursor|taskGraphCursor" bb_web/src: passed for UI migration; remaining hits are only legacy checkpoint type fields and normalize fallback in taskGraphs.ts.
- 验证：Browser check via in-app browser at http://localhost:8060/#/projects/blackboard/task-graphs: passed; mock Pregel run showed Run Timeline, Active Nodes, Revision, Applied mutation, Review node, and enabled Approve action.
- 验证：npm run build --prefix bb_web: intentionally not run per user instruction because backend work is running concurrently.

- 来源：inbox/2026-05-15-codex-taskgraph-node-去除左侧-accent-strip.md

- 来源：inbox/2026-05-15-codex-taskgraph-task-node-视觉对齐-编辑-预览-运行-fixture-统一节点组件.md

- 来源：inbox/2026-05-15-codex-taskgraph-preview-run-inputs-改为摘要入口与右侧抽屉.md

- 来源：inbox/2026-05-15-codex-taskgraph-run-节点状态-footer-避免-output-pin-重叠.md

- 来源：inbox/2026-05-16-codex-pregel-topology-mutation-frontend-implementation.md
- 代码位置：bb_web/src/data/taskGraphs.ts, bb_web/src/components/TaskGraphRunPanel.vue, bb_web/src/components/task-graph/TaskGraphMutationTimeline.vue

# 下一步

- 盘点现有 TaskGraphEditorPanel、RunPanel、GraphCanvas 与子组件缺口。
- 定义不同节点分类在 palette/inspector/canvas 上的 UI 规则。
- 设计 run timeline、artifact panel、approval/retry/cancel 的第一阶段交互。

- 阶段 1：梳理前端 contract gap，统一 `TaskGraphRunStatus`/`TaskGraphNodeRunStatus` 与 000055 目标状态（pending/running/blocked/needs_approval/failed/ready_for_review/done/cancelled）的映射，列出需要后端补充的字段和前端可先 mock 的字段。
- 阶段 2：抽出节点类型元数据（分类、颜色、默认 pins、配置摘要、是否产生 artifact/是否需要 approval），让 palette 分组、inspector header、canvas node meta 和 preview 复用同一来源。
- 阶段 3：增强 Graph/Run 画布覆盖层：节点状态 badge、cursor/paused/failed 高亮、active branch edge、duration/error 小型提示，并保证节点尺寸由 pins 稳定计算，不因状态文字抖动。
- 阶段 4：重整 Run detail drawer：把 node state、error、artifact preview、context output、agent/session timeline、log tail/child run 分成稳定区块；artifact 支持 markdown/json/text 预览和空态。
- 阶段 5：补操作入口：保留 cancel；为 paused actions 预留 approve/reject/continue 调用层；为 failed/cancelled/done run 预留 retry/run again 入口，所有未接 API 的操作必须 disabled 并展示原因。
- 阶段 6：补 i18n、样式和响应式检查，验证 `npm run build --prefix bb_web`；手动冒烟 catalog preview、editor、run panel、mock/rest fallback、窄屏 drawer 布局。

- 实现前先做编辑器布局重排：把 `TaskGraphEditorPanel.vue` 从纵向表单 + canvas 改成 top toolbar + canvas workspace + dock panels。
- 为 Graph Inputs/Settings 增加 collapsed summary 状态，默认折叠；需要编辑时用 drawer 或 slide-over 打开。
- 为 Node Inspector 增加右侧 drawer/overlay 模式，并保证 canvas 宽高基于 viewport flex 计算。

- 继续 064 时建议补 viewport/minimap/zoom controls，以及 Run UI 的状态覆盖和 artifact detail drawer；本轮主要完成 Editor canvas-first 基础形态。

- 后续可继续细化 exec/data 连线颜色：exec 线更偏白/灰或主状态色，data 线按 value_type 走颜色，以进一步贴近 UE Blueprint。

- 继续细化 Run UI 的状态色表达：节点类型色保持稳定，运行状态用 badge/outline/glow/edge active state 表达，避免颜色语义冲突。
- 后续如果新增 input_var 或更多业务节点类型，应只扩展 `taskGraphNodeVisuals.ts`，不要在各 panel 内重复定义节点外观。

- 如果后续 inputs 数量继续增长，可以进一步升级为左侧 input list + 右侧 selected input detail 的 master/detail 编辑器；当前先保留所有字段直接可编辑，减少交互跳转。

- 后续可考虑把编辑态 Graph Inputs drawer 和预览态 Run Inputs drawer 抽出一套共享 overlay shell，减少重复 CSS。

- 并发修改结束后建议补跑 `npm run build --prefix bb_web` 并在 running run 页面视觉确认 footer 与 Output pin 间距。

- 并发修改结束后建议统一补跑前端 build 和 running 节点视觉冒烟。

- Backend contract merge can remove legacy cursor fallback later once active_nodes/current_graph_revision/checkpoint revision fields are guaranteed.
- After backend settles, run npm run build --prefix bb_web and a full manual smoke against real task graph runs.
