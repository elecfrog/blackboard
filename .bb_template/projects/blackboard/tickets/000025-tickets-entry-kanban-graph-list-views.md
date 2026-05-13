+++
id = "000025"
lane = "bbd"
title = "Tickets 入口整合：Kanban / Graph / List 三视图"
created_at = "2026-05-08"
updated_at = "2026-05-09"
status = "archived"
area = "Tickets"
assignee = "codex"
depends_on = "000016"
kind = "product-ui"
parent = "000016"
+++

# 当前进展

- 需求：将 Board、Graph、Tickets 合并到一个统一入口 Tickets，不再作为三个分散入口暴露。
- 目标形态：Tickets 下提供 Kanban、Graph、List 三种 view，用户在同一业务对象集合里切换视图。

- 2026-05-08：实现 Tickets 统一入口：左侧导航移除独立 Board / Graph，Tickets 内提供 Kanban / Graph / List 三种 view。
- 2026-05-08：新增 `/projects/:project/tickets/graph` 与 `/projects/:project/tickets/list` 路由，旧 `/projects/:project/graph` 重定向到新的 Graph 子视图。
- 2026-05-08：Tickets 三视图共用 lane/search 筛选、详情 preview、依赖图和列表能力；ProjectSwitcher 在 Graph/List 下切项目时保留当前子视图。

- 2026-05-08：移除 Tickets 统一入口各处冗余 header（List view Tickets 标题、Graph view 标题副标题）；将 status 列可见性控件移入共享 Tickets 工具栏；Graph 视图在 status 过滤变化时自动重新布局；过滤活跃时跳过保存的完整布局，强制重新计算可见节点布局；合并 AutoScale 和重布局为一个 Reset 操作；移除不再使用的 Graph/List 描述性 i18n 字符串

# 记录


- ImageGen 已生成 Tickets 三视图入口整合 UI mock 作为设计参考。
- 验证：npm run build --prefix bb_web 通过。
- browser-use 冒烟：Kanban 默认页、Graph 子视图、List 子视图、ticket 详情 preview、旧 Graph 路由重定向均通过；console error 为空。

- 来源：2026-05-08-codex-ticket-view-filter-polish.md

# 下一步

- 梳理现有 BoardView / Ticket list / Graph route 与导航入口，确定统一 Tickets section 的路由与状态模型。
- 实现 Tickets 入口中的 view switcher，并迁移 Kanban、Graph、List 三个视图的现有能力。
- 确保现有 ticket 详情、依赖编辑、重排布局、状态/lane 修改等交互不退化。
