+++
id = "000016"
lane = "bbd"
title = "前端美化专项"
created_at = "2026-05-07"
updated_at = "2026-05-08"
status = "archived"
area = "bb_web"
assignee = "codex"
kind = "umbrella"
scope = "frontend-ui-ux"
+++

# 当前进展

- 2026-05-07：创建 BB 系统前端总单，用于收纳和管理前端体验优化、视觉打磨、交互一致性与现代化 UI 改造。
- 该总单用于减少零碎 UI 问题直接撑满依赖关系图；后续小型 UI 反馈优先作为本单记录或子范围处理，只有形成独立工程风险时再拆子 ticket。

- 2026-05-08：完成 Graph 标题副标题、toolbar 引导文案、Board summary 路径隐藏与 Settings Agent connector 文案美化，保持中英文 i18n 同步

- 2026-05-08：完成 Graph dark mode pin rings 修复，仅可见小圆点描边，hit target 保持透明

- 2026-05-08：完成 bb_web/DESIGN.md 改写为 Blackboard Notion-like workbench 设计规范，含视觉 token、typography、icon、layout、content 规范

- 2026-05-08：放松 Lucide 替换后侧栏导航行间距，Board/Tickets/Graph 行更高可读

- 2026-05-08：修复 Board topbar action 布局，locale/theme/lane/refresh 按钮在 i18n 切换时保持固定宽度；桌面端顶栏操作按钮单行排列；缩减桌面搜索最大宽度，扩大 lane/refresh 按钮空间；添加移动端 CSS override 使固定宽度按钮可正常换行。

- 2026-05-08：移除 sidebar Board/Tickets 计数胶囊，新增 Inbox 计数胶囊，调用当前 project inbox notes API 获取数量；样式统一为 Blackboard 黑白品牌色。

- 2026-05-08：统一 workspace sidebar 活动态、Agent workbench 选中态/hero/头像/指标、Settings connector 行/同步按钮/计数胶囊为黑白中性色，并在 Inbox/Agents/Settings 页面完成 light/dark 模式 browser 验证

- 2026-05-08：Sidebar 导航顺序调整保持 route targets、icons、active states、count badges 不变；在 Blackboard inbox 页面 in-app browser 完成验证。

# 记录

- 2026-05-08：Lucide 图标统一替换工作挂到本单下，作为前端美化专项的一部分推进。

- 2026-05-08：完成 Blackboard Web 普通 UI 图标统一替换为 lucide-vue-next；保留 TicketDependencyGraph 的业务 SVG 画布。
- 验证：npm run build --prefix bb_web 通过；browser-use 刷新 /projects/blackboard/graph，确认侧栏与 topbar 图标正常渲染，侧栏间距已放松。

- 2026-05-08：完成 Graph、Board summary 与 Settings Agent connector 文案美化；Graph 标题说明功能价值，toolbar 承载操作说明。
- 验证：npm run build --prefix bb_web 通过；browser-use 检查 /projects/blackboard/graph、/projects/blackboard、/projects/blackboard/settings，确认目标文案与路径隐藏符合批注。

- 2026-05-08：将 bb_web/DESIGN.md 从 Notion 官网风格参考改写为 Blackboard Web 自有 Notion-like workbench 设计规范。
- 2026-05-08：按设计规范抽取 bb_web 全局视觉 token，并收敛 shell、sidebar、topbar、toolbar、summary、filter、ticket meta 等高频 surface 的颜色、边框与间距。
- 验证：npm run build --prefix bb_web 通过；browser-use 检查 Graph、Board、Settings 页面无明显视觉回归。

- 2026-05-08：完成 Blackboard Web dark mode 对比度集中修复，统一暗色 token，并补齐侧栏、topbar、Settings connector、banner 与路径文本的暗色可读性。
- 验证：npm run build --prefix bb_web 通过；browser-use 手动冒烟 Board/Tickets/Inbox/Agents/Graph/Settings/ticket detail 暗色模式，确认主要页面文本、按钮、卡片、表格、图节点和提示条可读。

- 2026-05-08：修正 Graph dark mode 节点着色策略，统一 light/dark 为“整节点轻度状态色 + 左侧状态条强色 + 状态 pill 同源色”，避免暗色模式丢失主题色。
- 2026-05-08：修复 Graph dark mode 四向 pin 的可见小圆点 ring，透明 hit circle 不再继承可见描边。

- inbox/2026-05-07-codex-frontend-umbrella-ticket.md：Codex 创建本单并设置 in_progress / kind=umbrella / area=bb_web / scope=frontend-ui-ux

- 2026-05-08：Project switcher badge 改为前端稳定项目色：普通 project 由 project name hash 映射到固定色板；blackboard 品牌项目特化为 light 黑底白字、dark 白底黑字。
- 验证：npm run build --prefix bb_web 通过；browser-use 检查 /projects/blackboard/tickets 的 light/dark badge 显示符合黑板品牌规则。

- 2026-05-08：Sidebar 品牌区文案调整为 i18n：品牌名固定为 BLACKBOARD，副标题中文为“协作指挥台”、英文为 Command desk。
- 验证：npm run build --prefix bb_web 通过；browser-use 检查 /projects/blackboard/tickets 暗色模式下品牌区显示正常。

- 2026-05-08：Sidebar 品牌副标题微调：中文改为“协作工作台”，英文改为 Cowork Desk。
- 验证：npm run build --prefix bb_web 通过；browser-use 刷新 /projects/blackboard/tickets，中文 locale 下显示“协作工作台”。

- inbox/2026-05-08-codex-copy-polish.md

- inbox/2026-05-08-codex-dark-contrast.md：Codex 完成 dark mode 集中修复，含设计 token、侧栏/topbar/搜索/过滤器/看板暗色对比度、Graph 节点着色策略、pin ring 修复、project badge 颜色确定性、i18n 品牌文案

- inbox/2026-05-08-codex-design-system.md：Codex 完成 CSS design token 全局落地，shell/sidebar/topbar/toolbar/summary/filter/status toggles/ticket meta 均已迁移至新 token 方向；Graph/Board/Settings 页面 browser 验证通过

- inbox/2026-05-08-codex-lucide-icon-polish.md：Codex 完成 lucide-vue-next 集成并替换 sidebar/topbar/popup/inbox/connector/detail-panel 手绘图标；保留 TicketDependencyGraph SVG 画布

- inbox/2026-05-08-codex-topbar-i18n-stable-actions.md

- 2026-05-08：根据品牌位实测重新生成第二版 BB 图标，改为更清晰的单 B 主体，并替换前端 blackboard-icon.png 资源。
- 验证：npm run build --prefix bb_web 通过；browser-use 在 /projects/blackboard/graph 明暗模式下确认 sidebar 品牌位可识别。

- 2026-05-08：按批注调整 sidebar 工作区导航顺序为 Inbox、Board、Graph、Tickets、Agents。
- 验证：npm run build --prefix bb_web 通过；browser-use 在 /projects/blackboard/inbox 确认左侧顺序为 Inbox > Board > Graph > Tickets > Agents。

- 2026-05-08：调整 sidebar 工作区计数语义，移除 Board/Tickets 计数，仅 Inbox 显示未处理 inbox note 数量。
- 验证：npm run build --prefix bb_web 通过；browser-use 在 /projects/blackboard 确认仅 Inbox 显示黑白计数胶囊。

- 2026-05-08：按批注统一 sidebar active、Agent workbench 选中态/hero、Settings connector target row、同步按钮和 connector 小胶囊的颜色到 Blackboard 黑白中性色。
- 验证：npm run build --prefix bb_web 通过；browser-use 在 Inbox、Agents、Settings 明暗模式确认批注位置不再使用青绿/蓝色选中态。

- 2026-05-08：将本轮 UI 美化经验沉淀进 bb_web/DESIGN.md，补充黑白品牌色、暗色一致性、选中态、sidebar 计数语义、图标缩放、workspace 顺序和验证清单。
- 验证：文档人工复核，未触碰 bb_web/src；python3 scripts/check_ticket_ids.py --project blackboard 与 qmd embed 待本次收口执行。

- inbox/2026-05-08-codex-design-guidelines-ui-polish.md：细化 DESIGN.md 补充内容，含黑白品牌色强调、暗色一致性、选中态语义、sidebar badge 语义、workspace 导航顺序、graph 操作-copy 放置、小号品牌图标要求及 light/dark 模式验证清单

- inbox/2026-05-08-codex-inbox-count-badge.md：Codex 完成 sidebar 计数语义调整，移除 Board/Tickets，仅 Inbox 显示未处理数量；browser-use 在 /projects/blackboard 验证通过。

- inbox/2026-05-08-codex-neutral-selection-colors.md：Codex 将 sidebar 活动态、Agent workbench 选中态/hero/头像/指标、Settings connector 行/同步按钮/计数胶囊统一为黑白中性色，完成 Inbox/Agents/Settings 明暗模式验证

- inbox/2026-05-08-codex-sidebar-order.md：Codex 完成 sidebar 工作区导航顺序调整，保持原有 route targets/icons/active states/count badges 不变，in-app browser 验证通过。

- 2026-05-08：收紧 WikiPanel / WikiTreeItem 的颜色与布局样式，去掉旧的浅色局部变量，改为跟 Blackboard 全局 token 走。
- 2026-05-08：同步收短 wiki i18n 文案，避免英文切换时右上操作区和空状态文案过长。

# 下一步

- 持续收纳 Blackboard Web 前端的可用性、视觉层级、响应式、暗色模式、交互一致性等优化项。
- 执行具体 UI 优化时，优先把范围、验证和完成记录沉淀到本单或明确关联到本单，保持依赖图可维护。
