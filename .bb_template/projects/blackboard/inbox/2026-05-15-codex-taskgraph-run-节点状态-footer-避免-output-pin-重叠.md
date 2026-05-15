# 2026-05-15 codex 000064 Run node footer sandwich layout

时间: 2026-05-15T04:24:25Z
来源: codex
项目: blackboard

## 做了什么

- 将 `TaskGraphNodeShape.vue` 的运行状态从内容层右下角 badge 改为独立 footer band：节点结构变为 header / content / footer，footer 内显示状态 badge 和状态点。
- `TaskGraphRunPanel.vue` 为运行态节点额外增加 26px footer 高度，使 `Output` 等 data pin label 留在 content/pin 区，不再与 `running` 状态 footer 重叠。
- footer 背景使用底部圆角 path，避免覆盖节点 body 的圆角视觉。

## 验证了什么

- 按用户要求未运行 `npm run build --prefix bb_web`，避免影响并发修改。
- `git diff --check -- bb_web/src/components/task-graph/TaskGraphNodeShape.vue bb_web/src/components/TaskGraphRunPanel.vue`：通过；仅 Windows LF/CRLF 提示。

## 下一步

- 并发修改结束后建议补跑 `npm run build --prefix bb_web` 并在 running run 页面视觉确认 footer 与 Output pin 间距。

## 相关位置

- bb_web/src/components/task-graph/TaskGraphNodeShape.vue
- bb_web/src/components/TaskGraphRunPanel.vue
