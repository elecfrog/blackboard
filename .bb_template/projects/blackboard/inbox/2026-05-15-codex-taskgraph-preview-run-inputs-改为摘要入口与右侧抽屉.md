# 2026-05-15 codex 000064 Preview Run Inputs drawer layout

时间: 2026-05-15T03:38:38Z
来源: codex
项目: blackboard

## 做了什么

- 将 `TaskGraphPreviewPanel.vue` 的预览态 Run Inputs 从常驻大面板改为顶部摘要 chip：默认只显示 `Graph Inputs + 输入数量`，不再把 canvas 往下挤。
- 点击预览态 Graph Inputs 后打开右侧 `task-graph-preview-config-panel` 抽屉，内部继续使用 `TaskGraphRunInputsPanel` 编辑运行输入值。
- `TaskGraphRunInputsPanel.vue` 增加 `layout="drawer"` 模式，drawer 中每个运行输入改为更紧凑的一列布局，引用信息横向压缩显示。
- 预览态点击 canvas node 会自动收起 Run Inputs drawer，避免节点详情和输入面板同时争抢注意力。

## 验证了什么

- `npm run build --prefix bb_web`：通过；仅有 Vite 既有 large chunk warning。
- `git diff --check -- bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue bb_web/src/components/task-graph/TaskGraphRunInputsPanel.vue`：通过；仅 Windows LF/CRLF 提示。
- in-app browser cache-bust 访问 `/task-graphs/system/kb-wiki-build-workflow`：默认 `.task-graph-run-inputs-wide`=0、`.task-graph-preview-config-panel`=0；点击 `Graph Inputs 6 个输入` 后 `.task-graph-preview-summary-strip`=1、`.task-graph-preview-config-panel`=1、`.task-graph-run-inputs-drawer`=1、input rows=6、console error=0。

## 下一步

- 后续可考虑把编辑态 Graph Inputs drawer 和预览态 Run Inputs drawer 抽出一套共享 overlay shell，减少重复 CSS。

## 相关位置

- bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue
- bb_web/src/components/task-graph/TaskGraphRunInputsPanel.vue
