+++
id = "000005"
lane = "bbp"
title = "Blackboard 产品优化：UI/i18n/assignee/remote MCP/Agent 规范"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000002 000003 000004"
parent = "000001"
+++

# 当前进展

- 2026-05-06：期望继续产品化打磨：i18n、明暗模式、前端改 assignee、remote MCP、Agent prompt 与模板规范。
- 2026-05-06：完成 i18n、暗黑模式、assignee、remote MCP 首轮实现
- 2026-05-06：测试通过，进入 review，assignee 设为 codex
- 2026-05-06：Detail meta grid 的处理人格子已改为直接编辑 assignee，移除下方重复编辑区。
- 2026-05-06：左侧导航改为「工作区」/「系统」两组，Inbox 移回工作区
- 2026-05-06：Dashboard 改为左侧产品 Sidebar + 右侧 Topbar + Workspace Content 左右结构
- 2026-05-06：Project 切换迁入左侧 Sidebar，改为自绘 UnifiedPopupSelect
- 2026-05-06：Topbar 承载全局搜索、管理 Lane 和刷新，图标改为 SVG 绘制
- 2026-05-06：Settings Agent 连接器 UI 改为 summary + 折叠 row-item；OpenCode 默认展开，依靠内层固定高度列表滚动查看 metadata/source/target。
- 2026-05-06：Agent registry schema 从 employee 迁到 agents/project_agents；Codex 连接器显示名统一为 Codex，OpenCode 源文件迁入 blackboard/agents/opencode。
- 2026-05-06：修正 Settings 折叠区语义为 Agent manager；Codex/CodeBuddy 只展示 metadata 定义，OpenCode 只展示 22 个 agent 文件，AGENTS.md/Rules 不再作为 row-item。
- 2026-05-06：Settings connector summary 补回注册文件展示；CodeBuddy 顶部显示 AGENTS.md 与 Rules.mdc 两个下发目标，Agent manager 仍只展示 metadata agent。
- 2026-05-06：Settings connector summary 状态灯从 top-left 改为 center 对齐，避免与标题/注册文件块视觉错位。
- 2026-05-06：后端 Rust review 首轮：抽离内联测试模块，清理 clippy 告警，生产文件明显瘦身。
- 2026-05-06：修复 dark mode 下 Board/Tickets/Inbox/Diagnostics 的漏网浅色文本与控件。
- 2026-05-06：Tickets dark panel 进一步压回 neutral dark，移除表格区偏蓝面板感。
- 2026-05-07：夜间拆出的 6 张子票 000006-000011 已实现并推进到 review，可进入用户验收。
- 2026-05-07：处理用户 diff comments，完成 graph、Agent 工作台、ticket detail dark mode 三处 UI 返工。
- 2026-05-07：按用户继续验收要求，把 ticket dependency graph 升级成可编辑依赖画布。

- 2026-05-07：修正 blackboard/agents/AGENTS.md 第38行措辞，“静态 Blackboard Web 看板”→“Blackboard Web 前端（dist 构建产物）”，显式注明 000004 已把 tickets/inbox/lanes/projects 运行时数据源收敛到 bb-server /api/**，export:tickets 仅保留为导出/审计/调试 artifact
- 2026-05-07：同步修正 blackboard/agents/opencode/bb-pm.md 同类旧措辞“web 静态快照”→“Web 前端 dist bundle”，指回 000004

# 记录

## 背景

Blackboard 已从“离线快照优先”的只读页面转向正常 C/S 工单系统。下一阶段目标是把它变成其他 Agent 更容易接入、用户日常更舒服使用的产品化工具。

## 范围

- 前端 UI 继续打磨，引入 i18n 和明暗模式切换。
- Ticket 支持在前端修改 `assignee`。
- bb-server 从 stdio-only MCP 扩展为 remote streaming 接入，并让 Agent 可以用结构化接口创建/更新 ticket，避免手搓 Markdown。
- 优化 Blackboard 相关 Agent prompt，强调胖 ticket、瘦 handoff。
- 优化 ticket 与 handoff 模板。

## 验收

- Web 可切换中英文与明暗模式，并持久化偏好。
- Ticket detail/preview 里能编辑 assignee，刷新后仍保留。
- bb-server 暴露可供 remote Agent 接入的 streaming MCP 或兼容入口。
- Agent prompt 明确禁止无依据创建 ticket，要求 inbox 清理后删除 note，并把完成事实凝练到已有 ticket。
- ticket / handoff 模板体现“胖 ticket，瘦 handoff”。

- 通过 remote /mcp append_ticket_sections 写入本票

- 验证：cargo test、npm run build、/mcp JSON 与 SSE、REST assignee patch 通过

- 2026-05-06：修正 archived 000001 里过期的状态桶与 MCP 工具描述，避免后续 Agent 继续沿用旧口径。

- 2026-05-06：Codex in-app browser 自动化未完成：browser-use Node REPL 未暴露，Computer Use 禁止操作 com.openai.codex；保留人工 UI 验收。

- 2026-05-06：清理 Inbox 旧失败文案与 archived 000001 的旧状态桶叫法，统一到当前 C/S 与 Active/Done/Archived 语义。

- 2026-05-06：用户指出 detail meta 已有 Assignee 语义，应直接在该位置修改；TicketDetailPanel 已将输入框与保存按钮内联到处理人 meta card。

- 来源：2026-05-06-codex-dashboard-status-sidebar-polish.md

- 涉及路径：blackboard/agents/agents.toml、blackboard/agents/opencode/、bb-core agents_registry/agents_config、web AgentConnectorPanel/Row/data agents。
- 验证：cargo test；cargo build；npm run build --prefix blackboard/web；REST /api/agents、/api/projects/blackboard/agents、/api/agents/connectors；headless desktop/mobile screenshots。

- 视觉复验：dark desktop screenshot 显示 Codex/CodeBuddy 为 1 agents，OpenCode 为 22 agents，折叠标题统一为 Agent manager。

- 视觉复验：headless screenshot 确认 CodeBuddy row 显示 1 agents / 2 files，并列出 /Users/ornizhou/.codebuddy/AGENTS.md 与 /Users/ornizhou/.codebuddy/rules/blackboard-rules.mdc。

- 视觉复验：headless screenshot 确认 Codex/CodeBuddy 状态灯相对 summary 内容垂直居中。

- 后端维护：`bb-core/src/lib.rs` 从 3834 行降到 2503 行；`agents_config.rs` 1014→697，`http.rs` 1117→580，`stdio.rs` 1000→609；测试移入对应 `tests.rs`/`tests/` 文件。
- 后端 review 结论：下一步应继续把 `bb-core/src/lib.rs` 按 domain/repository/parser 分层，把 `stdio.rs` 的 MCP tool schema 从 transport handler 中拆出。
- Dark mode：补齐 select、lane filter、ticket table、Inbox row、Diagnostics 标题和值的暗色 token；headless Chrome 验证关键 computed style。
- Tickets dark 复验：工作区/header/table/row 背景为 neutral #151518/#202024，截图路径 `/tmp/blackboard-tickets-dark-neutral.png`。

- 覆盖：MCP schema 模块化、多 project workflow tools、Agent Registry 写 API、assignee project-agent 下拉、Agent 工作台、RDG ticket dependency graph。
- 验证：cargo test、npm build、check-ticket-ids、export:tickets、qmd embed、browser-use、computer-use 均跑过；最终门禁将在 handoff 后重跑。

- 用户反馈：graph 表达糟糕；Agent 工作台 UI 绘制粗糙；ticket detail dark mode 几乎坏掉。
- 返工范围：TicketDependencyGraph.vue、AgentWorkbench.vue、styles.css。
- 验证：browser-use 已逐页截图复验；最终 Blackboard 门禁脚本随后重跑。

- 范围：前端 TicketDependencyGraph.vue、BoardView.vue、tickets.ts、i18n.ts、styles.css；后端 http.rs 与 HTTP tests。
- 验证：cargo test、npm build、browser-use 交互保存、live DAG 拒绝测试已通过；最终 Blackboard 门禁随后重跑。

- inbox/2026-05-06-codex-bb-mcp-frontend-rdg-验收.md：codex 最终验收交接，6 张子票 000006-000011 完成 browser-use + computer-use 全页面验证（graph、agents、ticket detail）；check-ticket-ids.sh 门禁通过

- 来源：inbox/2026-05-07-codebuddy-agents-rules-wording-fix.md

# 下一步

- 如需要在 UI 保存 metadata，单独补 agents.toml 写入 API，避免直接在前端拼 TOML。

- 用户验收 000006-000011。

- 用户复验三处 UI。

- 用户复验依赖画布交互。
