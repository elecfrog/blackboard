+++
id = "000015"
lane = "bbd"
title = "BoardView 专项优化单"
created_at = "2026-05-07"
updated_at = "2026-05-08"
status = "archived"
area = "BoardView"
assignee = "codex"
depends_on = "000016"
source = "browser-diff-comments"
+++

# 当前进展

- 2026-05-07：用户希望把顶部统计已有的状态着色下沉到下方每个 Lane/状态列，让 Lane 本身更清晰可扫。

- 2026-05-07：TicketDetailPanel Workflow 格改为 UnifiedPopupSelect，5 enum 下拉触发 @status-change；BoardView 新增 statusSavingId 与 updateTicketStatus，走 patchTicket(..., { status }) 调 bb-server PATCH。
- 2026-05-07：i18n 新增 workflowStatusUpdateFailed；Graph pill 显示 statusLabels 本地化文本而非原始 enum。
- 2026-05-07：颜色令牌收敛到 --bb-status-*，TicketDetailPanel 与 TicketDependencyGraph 共享。

- 2026-05-07：移除 Board 页面顶部 status metric grid，消除与下方 Kanban 列的重复表达
- 2026-05-07：在 BoardView.vue 各 Kanban 列添加 status-specific data-kind 标记
- 2026-05-07：把 workflow status 着色下沉到 Kanban 列（accent、列背景色、header dots、count pills）
- 2026-05-07：为 Kanban 列添加 dark-mode 颜色适配，确保暗色主题下 status accent 可见
- 2026-05-07：npm run build --prefix bb_web 成功完成

- 2026-05-07：Board Kanban 改为单排横向滚动，lane 筛选条同步横向滚动
- 2026-05-07：移除 Board 主视图 Diagnostics 面板和 compact Inbox

- 2026-05-08：新增 ProjectMeta/ProjectIndex board_view.hidden_statuses 字段，作为 project 级 BoardView 偏好配置
- 2026-05-08：新增 GET/PATCH /api/projects/{project}/board/view 接口，写入前校验 status、去重、canonical 顺序，禁止隐藏全部状态列
- 2026-05-08：BoardView 状态列 checkbox 改为调用后端持久化接口，reload 时从 meta.board_view.hidden_statuses 恢复可见列
- 2026-05-08：补全 REST round-trip 与非法隐藏全部状态列测试，覆盖配置写入和拒绝逻辑

- 2026-05-08：BoardView 状态列顺序固定为 Todo → In progress → Review → Done → Blocked → Archived
- 2026-05-08：BoardView 增加状态列 checkbox 可见性控制，可隐藏 Archived 等状态列
- 2026-05-08：保持顶部 Kanban 横向滚动条与可见状态列同步

# 记录


- inbox/2026-05-07-codebuddy-workflow-status-ui.md

- inbox/2026-05-07-codex-board-ui-lane-colors-implemented.md

- inbox/2026-05-07-codex-board-ui-polish-ticket.md（ticket 创建确认 + 门禁通过）

- inbox/2026-05-07-codex-kanban-horizontal-lanes.md

- inbox/2026-05-08-codex-boardview-settings-persistence.md

- inbox/2026-05-08-codex-boardview-status-columns.md

# 下一步

- 清理 BoardView 顶部 status metric grid，避免与下方 Kanban Lane 重复表达。
- 优化下方 Kanban Lane/状态列 UI：每列使用对应 workflow status 的 accent、边框或轻量底色，空状态也保持清晰层级。
