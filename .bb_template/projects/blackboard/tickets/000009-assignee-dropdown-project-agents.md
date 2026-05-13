+++
id = "000009"
lane = "bbd"
title = "Assignee 下拉与项目成员过滤"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000016"
parent = "000005"
+++

# 当前进展

- 2026-05-07：从用户夜间任务拆出，准备把 assignee 自由输入升级成项目成员下拉。

- 2026-05-07：完成 ticket 详情页 assignee 项目成员下拉与 legacy assignee 显示。

- 2026-05-07：根据用户验收反馈，修复 ticket detail dark mode 视觉破损。
- 2026-05-07：根据验收反馈，把处理人控件改成与 workflow 一致的 `UnifiedPopupSelect`，移除保存按钮，切换即保存；处理人显示稳定 agent id，避免 `display name · id` 重复表达。

# 记录

- 范围：Ticket detail/board assignee 使用当前 project 可分配 agent 列表，支持清空与 legacy 显示。
- 验收：切换 project 时候选人随 /api/projects/{project}/agents 更新；保存后刷新仍保留。

- 实现：前端通过 /api/projects/{project}/agents 获取当前 project 可分配成员，保存仍走受控 ticket update。
- 验证：browser-use 与 computer-use 均打开 /projects/blackboard/tickets/000009，确认处理人下拉显示 Codex · codex，Graph 入口可见。

- 返工：补齐 detail panel/header/meta/progress/dependency/MarkdownRenderer/TOC 在 dark mode 下的背景、文本、链接与代码块样式，避免浅色卡片和低对比正文。
- 验证：browser-use 打开 /projects/blackboard/tickets/000009，确认 detail 弹层、assignee 下拉、依赖卡片、Markdown 正文和 TOC 在 dark mode 下可读。
- 返工：处理人下拉和 pill 统一显示稳定 id（如 `bb-pm`、`code-dispatcher`），保存仍走受控 `PATCH /api/projects/{project}/tickets/{id}`。
- 字体：全局 UI 字体切到 Google `Noto Sans SC`，保留系统字体 fallback。
- 验证：`npm run build --prefix bb_web` 通过。

- inbox/2026-05-06-codex-graph-agents-detail-dark-ui-polish.md 来源：codex 验收三处 UI 返工（dark mode detail panel/header/meta/依赖/MarkdownRenderer/TOC/链接/代码块暗色样式）

# 下一步

- 接入 project agents API，改造 TicketDetailPanel 控件。

- 等待用户验收。

- 等待用户复验 dark mode ticket detail。
