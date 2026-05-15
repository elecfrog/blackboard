# 2026-05-15 codex 000064 remove task node accent strip

时间: 2026-05-15T04:49:09Z
来源: codex
项目: blackboard

## 做了什么

- 从 `TaskGraphNodeShape.vue` 移除 `task-graph-node-accent-strip` 小竖条 SVG rect。
- 删除对应 `.task-graph-node-accent-strip` scoped CSS；节点类型表达继续由 header tint、icon disc、type badge、pins 和 footer 状态承担。

## 验证了什么

- 按并发要求未运行 `npm run build --prefix bb_web`。
- `rg -n "task-graph-node-accent-strip" bb_web/src/components bb_web/src/views bb_web/src/styles.css`：无残留匹配。
- `git diff --check -- bb_web/src/components/task-graph/TaskGraphNodeShape.vue`：通过；仅 Windows LF/CRLF 提示。

## 下一步

- 并发修改结束后建议统一补跑前端 build 和 running 节点视觉冒烟。

## 相关位置

- bb_web/src/components/task-graph/TaskGraphNodeShape.vue
