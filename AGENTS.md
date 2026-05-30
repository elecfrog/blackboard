# Blackboard

Blackboard 是一个本地多 Agent 协作工作台，将 Codex、CodeBuddy、OpenCode 等 AI 编程工具的上下文（project、ticket、inbox、wiki）统一聚合管理。

## 技术栈

### 后端 (`bb_backend`)

基于 Rust 构建的高性能后端服务，负责：
- 多 Agent 上下文管理与聚合
- Project、Ticket、Inbox、Wiki 数据模型与 API
- 与 Codex/CodeBuddy/OpenCode 等工具的数据同步

### 前端 (`bb_web`)

基于 Vue 构建的 Web 界面，提供：
- 统一的上下文浏览与搜索
- Project、Ticket、Inbox、Wiki 可视化展示
- 响应式设计，支持多视图切换

### 桌面端 (`bb_desktop`)

基于 Tauri 构建的跨平台桌面应用，特性：
- 原生桌面体验，系统托盘集成
- 本地数据存储，离线可用
- 轻量级打包

## 快速运行

```
python3 scripts/dev.py
```

## 常用 TaskGraph 主图注册

当用户要求启动常见 Blackboard 管线时，优先使用下面的主图引用，不要让用户每次重复 graph 名称。Graph ref 使用 `<scope>/<id>` 形态；启动仍通过 TaskGraph API / UI / daemon，不要手改 graph 或运行产物。

- Inbox 批量清理：`system/inbox-batch-cleanup`。触发语义包括“清理 inbox”、“批量清理 inbox”、“inbox cleanup”。默认 `batch-count=5`、`max-iterations=5`，除非用户指定更大的批次。
- 项目维护 / bbpm 深度维护：`system/bbpm-maintain`。用于 inbox/ticket 日常维护、ticket cleanup/audit 的综合维护请求。
- Ticket schema 巡检修复：`system/task-ticket-audit`。用于“ticket audit”、“schema repair”、“检查 ticket JSON”这类请求。
- 向内调研：`system/research-internal`。用于基于当前工作区源码、文档、历史 artifacts 的内部调研。
- 向外调研：`system/research-external`。用于官方文档、论文、仓库、URL、市场或新闻等外部资料调研。
- 内外调研到迭代草案：`system/research-iteration-draft`。用于从 internal/external research 合成 draft 草案。
- Spec Arena 4+1：`project/spec-arena-4-plus-1`。用于已准入 draft 到完整 implementation spec 的 4 reviewer + 互评 + attacker + human gate + merge 流程。
- Rust/Vue 代码审查：`system/rust-vue-code-review`。用于只读收集 diff、Rust/Vue 静态检查并产出审查报告。
- Review Scout Fix：`system/code-review-scout-fix`。用于基于 code review finding 的保守修复闭环。
- 代码监控检查修复：`system/code-monitor-check-fix`。用于前端 i18n、主题、Rust fmt/clippy 等常规质量检查修复。
- 前端 UI 收敛：`system/frontend-ui-convergence-codex`。用于按 `DESIGN.md` 和 shared primitives 收敛 UI 风格。
- 前端冒烟：`system/frontend-smoke`。用于本地前端可视化冒烟验证。
- KB / Wiki 路由：`system/kb-workflow-router`。用于不确定应走 research、wiki build 还是 wiki update 的 KB/Wiki 请求。
- KB 外部调研：`system/kb-research-workflow`。用于写入 KB research Markdown 并刷新 QMD。
- KB Wiki 从零构建：`system/kb-wiki-build-workflow`。用于基于结构化输入生成新的 Wiki。
- KB Wiki 增量更新：`system/kb-wiki-update-workflow`。用于从 git/local changes 增量更新已有 Wiki。
- 夜间自动优化：Blackboard 项目内优先 `project/nightly-auto-code-optimization-custom-mpmtlzwa`；没有项目自定义图时使用 `system/nightly-auto-code-optimization`。

## Practice in Blackboard

Ticket 是轻量 BDD

在 Blackboard 项目里，ticket 不是任务备忘录，也不是历史记录。  
ticket 是当前可执行的产品/工程行为定义，承担轻量 BDD 的角色：描述行为、边界和验收标准，让 Agent 能直接按它实现。

Agent 写 ticket 时应满足：

- 只描述当前有效定义，过时内容应删除或替换。
- 用用户语言定义行为，不用 Agent 自己重定义产品方向。
- 明确 MVP 行为、非目标、验收标准。
- 子票只拆实现范围，不重新解释父票产品定义。
- handoff 记录过程，ticket 保存当前事实。
- 创建和更新 ticket 必须走 Blackboard MCP 结构化工具；新 ticket 使用 JSON BDD 形态，显式包含 `summary`、`stories`、`risks`、`progress_record`、`attachments`。即使 `risks`、`progress_record`、`attachments` 为空，也传 `[]`。
- 附件是顶层 `attachments` 字段，不写入 `extra.attachments`。过程记录通过 `append_ticket_sections` 追加到 `progress_record`，不是 Markdown 正文。
