+++
id = "000017"
lane = "bbd"
title = "Graph View 专项优化：UE 风格连线与交互打磨"
created_at = "2026-05-07"
updated_at = "2026-05-08"
status = "archived"
area = "TicketDependencyGraph"
assignee = "codex"
depends_on = "000011"
kind = "专项优化"
parent = "000016"
+++

# 当前进展

- 2026-05-07：用户确认 Graph view 需要接在 000011 后面的专项优化单，用于持续收纳依赖图视觉、连线、布局和交互打磨。
- 首轮范围：将当前实心三角箭头改为 UE 蓝图风格，弱化常驻箭头噪声，用柔和 spline 与节点 pin/端点表达连接方向。

- 2026-05-07：首轮 UE 风格连线已实现：移除 Graph SVG marker 实心三角箭头，依赖线改为圆角 spline，hover 时加粗高亮。
- 2026-05-07：拖拽中的 preview edge 改为实线圆角 wire，继续保留 pin 拖拽创建依赖与边中点删除控件。

- 2026-05-08：修复重排布局中 000017 这类专项后续单跑到同层顶部的问题；auto layout 增加 sole successor 自顶向下校正，让单后续叶子节点贴近唯一 parent 的水平线。

- 2026-05-08：Graph pin hover 视觉继续打磨：四向 pin 去掉内部箭头，只保留小圆点。
- 2026-05-08：Graph pin 小圆点改为继承当前 node workflow status 颜色；依赖边改为 source status color -> target status color 的 SVG linearGradient 彩虹渐变线。

- Graph node hover pins polished: removed the inner arrow glyphs, leaving status-colored circular connection handles.
- Graph edges now render per-edge SVG linearGradient strokes from source node status color to target node status color.
- Graph node title layout now uses visual-width wrapping with extra inset and first-line room for the status pill, reducing cramped text around 000017.

- 2026-05-07：Graph 节点新增左侧 accent bar 与按 workflow status 着色的 pill；颜色令牌放在 --bb-status-*，与 TicketDetailPanel 共享。
- 2026-05-07：i18n 新增 workflowStatusUpdateFailed；graph pill 显示 statusLabels 本地化文本而非原始 enum。

- 2026-05-07：auto layout 改为按 parent/child 图压力排序，替代 plain id 顺序
- 2026-05-07：重排布局后对 umbrella/source 节点做父级对齐，使其在垂直方向上居中于依赖节点组
- 2026-05-07：dependency edge pin 选择改为节点水平间距充足时优先右到左路由
- 2026-05-07：缩小依赖路径的曲线控制距离，避免大型扇出连接形成过大弧形

- 2026-05-08：Graph pin 去除内部箭头，改为 status 颜色圆点；依赖边改用 source→target status 颜色 linearGradient 渐变线；node title 增加水平边距和首行空间。

- 2026-05-08：按依赖连通分量分块计算纵向位置，避免新增根单被既有依赖簇挤到上方或中间

- 2026-05-08：修复 Graph 自动重排，避免连通分量内部继承全图 level 碰撞产生的大纵向空洞；分量内部改为先按本分量 level 紧凑排列，再做 parent/child 视觉中心校正；修复 root 与多子节点中心对齐，多 parent 汇聚到同一 child 时不被错误压到同一水平线；保留多 root 分块逻辑，组件间仍按分量顺序纵向分隔。

- 2026-05-08：移除依赖线和拖拽预览线的 `vector-effect: non-scaling-stroke`，让线宽随滚轮 zoom 与节点一起缩放

# 记录


- 验证：npm run build --prefix bb_web 通过。

- 验证：npm run build --prefix bb_web 通过。

- 验证：npm run build --prefix bb_web 通过。

- Verification: npm run build --prefix bb_web passed after the pin, gradient edge, and node title spacing changes.

- Verification: python3 scripts/check_ticket_ids.py --project blackboard && qmd embed passed from /Users/ornizhou/development/blackboard after the inbox handoff was added.

- inbox/2026-05-07-codebuddy-workflow-status-ui.md

- inbox/2026-05-07-codex-graph-edge-layout-polish.md

- inbox/2026-05-07-codex-graph-ue-wire-ticket.md

- inbox/2026-05-08-codex-graph-pin-gradient-spacing.md

- inbox/2026-05-08-codex-graph-component-layout.md

- inbox/2026-05-08-codex-graph-compact-centered-layout.md

- inbox/2026-05-08-codex-graph-scaled-wire-stroke.md

# 下一步

- 移除 Graph 依赖线常驻实心三角箭头，改成圆角曲线和轻量端点表达。
- 保持 hover、删除边、拖拽创建依赖、重排布局等现有交互不退化。
