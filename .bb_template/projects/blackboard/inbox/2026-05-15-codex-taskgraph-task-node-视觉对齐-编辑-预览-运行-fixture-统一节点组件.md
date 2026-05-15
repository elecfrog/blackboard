# 2026-05-15 codex 000064 Task Node UI surfaces aligned

时间: 2026-05-15T03:19:07Z
来源: codex
项目: blackboard

## 做了什么

- 抽出并接入共享 Task Node 视觉来源：`taskGraphNodeVisuals.ts` 负责类型颜色、分类、icon、badge label、meta label；`TaskGraphNodeShape.vue` 负责 SVG 节点骨架。
- `TaskGraphEditorPanel.vue` 移除本地 nodeColors/nodeVisual/nodeMetaLabel 规则，canvas node 改用共享颜色/meta 与 `TaskGraphNodeShape`。
- `TaskGraphPreviewPanel.vue`、`TaskGraphRunPanel.vue`、`TaskGraphCanvasFixture.vue` 均改用 `TaskGraphNodeShape`，避免预览态、编辑态、运行态出现不同 Task Node 外观。
- Run UI 保留运行状态作为节点 status badge，节点本体颜色回到节点类型色；UE 风格 exec pin/data pin 由 `GraphCanvas.vue` 继续统一渲染。

## 验证了什么

- `npm run build --prefix bb_web`：通过；仅有 Vite 既有 large chunk warning。
- `git diff --check -- bb_web/src/components/TaskGraphEditorPanel.vue bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue bb_web/src/components/TaskGraphRunPanel.vue bb_web/src/components/TaskGraphCanvasFixture.vue bb_web/src/components/task-graph/TaskGraphNodeShape.vue bb_web/src/components/task-graph/taskGraphNodeVisuals.ts bb_web/src/components/GraphCanvas.vue`：通过；仅 Windows LF/CRLF 提示。
- in-app browser `/task-graphs/system/frontend-smoke/edit`：可见 Task Graph Editor、节点面板、控制流校验；DOM 统计 `.task-graph-node-shape`=4、`.graph-exec-pin-glyph`=6、`.graph-data-pin-glyph`=2、默认 In 文案=0。
- in-app browser `/task-graphs/system/frontend-smoke`：预览态 DOM 统计 `.task-graph-node-shape`=4、`.graph-exec-pin-glyph`=6、`.graph-data-pin-glyph`=2。

## 下一步

- 继续细化 Run UI 的状态色表达：节点类型色保持稳定，运行状态用 badge/outline/glow/edge active state 表达，避免颜色语义冲突。
- 后续如果新增 input_var 或更多业务节点类型，应只扩展 `taskGraphNodeVisuals.ts`，不要在各 panel 内重复定义节点外观。

## 相关位置

- bb_web/src/components/task-graph/taskGraphNodeVisuals.ts
- bb_web/src/components/task-graph/TaskGraphNodeShape.vue
- bb_web/src/components/TaskGraphEditorPanel.vue
- bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue
- bb_web/src/components/TaskGraphRunPanel.vue
- bb_web/src/components/TaskGraphCanvasFixture.vue
- bb_web/src/components/GraphCanvas.vue
