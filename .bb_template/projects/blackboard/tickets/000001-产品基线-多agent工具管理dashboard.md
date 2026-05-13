+++
id = "000001"
lane = "bbp"
title = "Blackboard 产品基线：本地多 Agent 工具管理 Dashboard"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
doc_type = "product-baseline"
+++

# 当前进展

- project `blackboard` 已落地（`projects/blackboard/project.toml`、`.ticket-id`、空 inbox/tickets 骨架）。
- 本基线 ticket 首版写就，锁定"本地多 Agent 工具管理 Dashboard"的产品定位与 P0 能力边界。
- 竞品章节 2026-05-06 补调研：Paperclip（AI 公司控制平面）、Multica（Agent 当队友派 issue），明确 Blackboard 不走"代管 Agent 执行"的差异路线。
- 2026-05-06 增补 Agent 配置分发能力的事实源（`blackboard/agents/AGENTS.md`）并把详细 spec 拆到 `000002`（后端）/ `000003`（前端）两张子 ticket。基线 ticket 自身只保留指针，避免成为第二事实源。
- 后续子 ticket（多 Agent 心跳、看板按 Agent 视图、超期提醒、冒烟基线、IA 细化）待从基线展开并落号。
- 2026-05-06：Board preview 路由修复，Board 点击 card 改为导航到 `/projects/:project?preview=:id`，不再跳 Tickets 语境
- 2026-05-06：Dashboard 单页重构，Board/Tickets/Inbox 三个工作区合并为同一 Dashboard shell，新增 TicketDetailPanel 抽屉
- 2026-05-06：固定开发端口，后端 127.0.0.1:3001，前端 127.0.0.1:8060，更新 AGENTS.md
- 2026-05-06：preview panel 宽度从 `min(920px, 92vw)` 调整为 `min(1120px, calc(100vw - 72px))`
- 2026-05-06：响应式重叠修复，`.bb-dashboard-content` 增加 `container-type: inline-size`，kanban 列宽响应式断点 1180px/820px/560px
- 2026-05-06：ticket 拖拽，后端 PATCH `/api/projects/:project/tickets/:id` 持久化 bucket/status/lane，前端 HTML5 drag/drop 实现乐观更新

# 记录

## 1. 愿景

Blackboard 的目标不是再造一个工单系统，而是做**本地多 Agent 工具管理 Dashboard**：

- 在一台开发机上，串联 Codex、Claude、OpenCode、Rider、VSCode、Minimax 等并发 Agent 的工作进度。
- 以 project 为隔离边界，以 ticket 为最小协作凭证，以 lane 为并行分工轴，以 inbox 为低摩擦交接信道。
- 永远以本地 Markdown 文件为事实源，服务端（bb-server）、看板（web）、脚本（scripts）只是这套事实的结构化视图与写入通道。
- 让"哪个 Agent 正在做什么 / 做到哪步 / 下一步谁接手"在单一 Dashboard 上一眼看清，彻底解决多工具之间的上下文断流。

一句话：**在本地把多 Agent 协作的进度、责任、交接可视化**。

## 2. 用户画像

| 画像 | 场景 | 对 Blackboard 的关键诉求 |
|---|---|---|
| 主控开发者（ornizhou） | 同时用 3~5 个 Agent/IDE 推进多个 project | 一眼看清全量在途工作；快速定位阻塞；低摩擦给下一个 Agent 交接 |
| 业务 Agent（Codex/Claude/OpenCode/Rider/VSCode） | 做完一段实质工作需要留痕给下一棒 | 低门槛丢 inbox、结构化创建/更新 ticket，不关心全流程 |
| 整理 Agent（Minimax 等） | 周期性把 inbox 消化为 ticket、更新进度、归档 | 稳定的结构化 MCP 接口、一致的 frontmatter、可审计的变更链 |
| 人类 reviewer | 偶尔上看板巡检 / 决策 | 只读即时概览、按 project/lane/状态过滤、离线也能看 |

非用户：团队协作、跨机器同步、工单审批、权限管控——都不在本产品范围内。

## 3. 核心场景

1. **Agent 交接闭环**：业务 Agent 完成一段工作 → 丢 inbox 笔记（或直接 `create_ticket`/`update_ticket`）→ 看板/服务端立刻可见 → 整理 Agent 抽取合并 → 下一个 Agent 起步时 `list_tickets` 找到上下文。
2. **多 Agent 并行**：同一个 project 下，不同 lane（bbp/bbt/bbd/bbq）的工作由不同 Agent 并行推进，不串行等待；Dashboard 按 lane 分栏展示当前 Active/Done/Archived 状态。
3. **project 切换**：顶部切换 project，看板、inbox 列表、lane catalog、ID 空间全部切换；不存在"全局视图混淆 project"的场景。
4. **进度巡检**：人类打开 web 看板，一眼看清每个 project 的 Active/Done/Archived 状态分布、每个 lane 正在做什么、有哪些 ticket 超期未更新。
5. **离线降级**：bb-server 未启动时，web 看板从 `public/blackboard/projects/<name>/{tickets,inbox,lanes}.json` 快照回落，仍可只读巡检。
6. **lane 演化**：用户随业务演化增删 lane（`upsert_lane` / `archive_lane`），历史 ticket 不受影响，新 ticket 不能挂到归档 lane。

## 4. 核心能力清单（P0）

| 层 | 能力 | 已具备 | 待建 |
|---|---|---|---|
| 事实源 | project 化 Markdown（inbox/tickets/lanes），per-project ID，KV frontmatter | 是 | — |
| 事实源 | Ticket 文件名 `<id>-<slug>.md`，lane 通过 frontmatter 记录 | 是 | — |
| 后端 | bb-server stdio / remote MCP 16 个结构化工具，支持 ticket / inbox / lane 管理 | 是 | 多 Agent 心跳/占用字段 |
| 后端 | bb-server HTTP REST 读写 ticket / inbox / board_summary / lane，并暴露 `/mcp` remote 入口 | 是 | — |
| 前端 | 多 project 顶部切换 + project 元数据面板 | 是 | 多 Agent 视图（谁在做哪条 ticket） |
| 前端 | LaneManager UI + 四桶看板 + 过滤 | 是 | 按 Agent 维度的二级视图 |
| 脚本 | next-ticket-id / check-ticket-ids / export-tickets-json / migrate-family-to-lane | 是 | — |
| 工作流 | 跨 Agent 共享 AGENTS.md（事实源 `blackboard/agents/AGENTS.md`）对齐 project 化 + lane + KV frontmatter | 是 | 后端 + 前端的 Agent 连接器（详见 `000002`/`000003`） |
| 工作流 | Agent 配置分发（codex / codebuddy / opencode 一键 sync + 状态灯） | 否 | bb-server `agents_config` 模块（`000002`） + web Settings "Agent 连接器"区块（`000003`） |

关键约束：
- 一切以 Markdown 为事实源，bb-server / web 只是视图。
- 除 `list_projects` 外每次 MCP/REST 调用必须显式传 `project`。
- Ticket 写入只走 stdio MCP（`create_ticket` / `update_ticket`），不接受 raw Markdown patch；HTTP 只在 inbox 和 lane 两个面开放写入。
- frontmatter 是 KV 结构：核心字段固定 6 个（id/lane/title/status/created_at/updated_at），其余一律走 `extra`；不再引入顶层 `current`，当前进展写正文 `# 当前进展`。

## 5. 非目标（显式排除）

- **不做团队协作 / 多机同步**：Blackboard 是"单机多 Agent"的 Dashboard，不是 Jira/Linear/飞书工单的替代。
- **不做审批流 / 权限 / 角色**：唯一"权限"是人类对本地文件系统的访问权限。
- **不做富文本编辑 / WYSIWYG**：一切都是 Markdown + frontmatter，编辑通过 Agent/IDE 完成。
- **不做 board.md 这样的第二事实源**：派生视图（web 看板、board_summary）只读，不回写为单独的聚合文件。
- **不做跨 project 的统一 ID / 统一看板**：每个 project 的 ID 空间独立；跨 project 引用用 `<project>/<id>` 的方式在正文里写。
- **不做 MCP Streamable HTTP / SSE / HTTP JSON-RPC**：stdio MCP 是唯一的富接口面，HTTP 严格克制只做 inbox + summary + lane。
- **不追求"自动化整理一切"**：整理 Agent 仍是人工触发的工具调用，不自动推进工单状态。

## 6. 关键指标（P0）

围绕"多 Agent 工作流是否真的跑顺了"三个方向。

| 维度 | 指标 | 目标 |
|---|---|---|
| 交接闭环 | 业务 Agent 完成一段工作到下一个 Agent 能通过 MCP 看到上下文的延迟 | < 10s（纯本地文件系统 + stdio） |
| 事实一致性 | `check-ticket-ids.sh` 在任意 project 报错数 | 0 |
| 事实一致性 | `board_summary.metadata_warning_count`（frontmatter 未知字段告警） | 0 |
| 并行性 | 同一 project 下不同 lane 能否真正并行推进（阻塞/等待率） | 非阻塞 lane 无串行等待 |
| 离线降级 | bb-server 关闭时 web 看板能否从 json 快照渲染四桶 | 100% 可渲染 |
| 覆盖率 | 主控开发者的在途工作在看板上的可见率（不依赖 Agent 自述） | ≥ 90% |

## 7. 竞品 / 参考

多 Agent 管理这一带近半年冒出了几个直接对位的工具，需要先把"我们不做什么"说清楚。

### 7.1 Paperclip（<https://paperclipai.net> / `paperclipai/paperclip`）

- **定位**：AI Agent 的"控制平面"、"零人工公司操作系统"。Node.js server + React UI。
- **核心能力**：组织架构 / 角色 / 汇报线、Agent 心跳、跨团队任务委托、审批链、预算与成本归因、VM 沙箱、完整工具调用审计。
- **典型用法**：Dashboard 就是"董事会"，人类点审批、下目标，多个 Agent 在平台里跑自己的岗位。
- **与 Blackboard 差异**：Paperclip 要当 Agent 的**老板**——它承担派单、审批、预算、沙箱执行；Blackboard 不派单、不执行、不审批，只做**凭证与可视化**。

### 7.2 Multica（<https://multica.ai> / `multica-ai/multica`）

- **定位**："开源版 Claude Managed Agents"，把 Claude Code / Codex / OpenCode / Gemini 等编码 Agent 接入统一的 issue 系统。开源 AI 原生项目管理平台。
- **核心能力**：给 Agent 分配 issue、Agent 自动领单 → 写代码 → 报 blocker → 更新状态、WebSocket 实时推送、技能沉淀、自托管部署。
- **典型用法**：把 Agent 当远程同事，**issue 指派给 Agent**，Agent 自己在 runtime 里跑完反馈回来。
- **与 Blackboard 差异**：Multica 把代码工作和全流程都交给 Dashboard 代管；Blackboard 坚持**"人自己打开 Agent 工具干活"**，Dashboard 只在"开工前找上下文"和"收工后留凭证"两个切面出现，不拦截 Agent 的执行路径。

### 7.3 其它参考

- **Jira / Linear / 飞书工单**：团队、云端、审批重。Blackboard 反方向：单机、零审批、Markdown 事实源。
- **Obsidian + Dataview / Tasks**：同为本地 Markdown 事实源，但没有结构化 MCP/HTTP 写入面，不适合给多个 Agent 写。
- **Logseq**：block 结构利于笔记，不利于"工单 + lane + 四桶"的聚合视图。
- **TaskWarrior / GTD 工具**：个人任务管理，没有 project / lane / 多 Agent 语义。
- **CrewAI / LangGraph / Open Spec / Conductor**：多 Agent 编排框架，侧重执行编排，不提供跨会话的持久协作凭证。

### 7.4 Blackboard 生态位对比

| 维度 | Paperclip | Multica | Jira/Linear | Blackboard |
|---|---|---|---|---|
| 部署形态 | 本地/云端服务 | 本地/云端服务 | 云端 SaaS | **纯本地，Markdown 事实源** |
| 对 Agent 的态度 | 当员工管理（派单/审批/预算） | 当队友指派 issue（自动领单执行） | 不感知 Agent | **当协作同侪，不派单、不拦执行** |
| 代码工作怎么跑 | 平台内 VM 沙箱跑 | Agent runtime 跑 | 人在 IDE 跑 | **人自己打开 IDE + Agent 工具跑** |
| 状态推进 | Agent 自动更新 | Agent 自动更新 | 人工流转 | **显式 `create_ticket` / `update_ticket`，可人可 Agent** |
| 跨 Agent 上下文 | 内部调度协议 | issue 评论流 | 评论 + @人 | **Markdown ticket + inbox 笔记（跨工具可读）** |
| 事实源 | 数据库 | 数据库 | 数据库 | **Git 可追踪的 Markdown** |
| 目标规模 | 零人工公司 | AI 原生团队 | 人类团队 | **单机开发者 + 多 Agent 并发** |

**一句话生态位**：Paperclip 和 Multica 都在往"让 Dashboard 把 Agent 管起来、让 Agent 替你干活"的方向走；Blackboard 故意不走那条路——**代码工作仍然由我自己打开 Agent 工具完成，Dashboard 只解决"多 Agent 之间的上下文断流"**，靠 Markdown 事实源 + MCP 结构化写入做到跨会话、跨工具、跨 IDE 的持久凭证。

结论：两类产品不是同一生态位，可以互补（比如 Paperclip / Multica 负责编排执行时，Blackboard 仍可作为本地开发者自己的凭证层）；但在"单机 × 多 Agent × Markdown 事实源 × 我自己开 Agent 干活"这个交集上，目前没有直接对手。

## 8. 风险与依赖

- **frontmatter KV 演化风险**：新增字段要保持在 `extra` 里，才能零迁移前进；一旦落到核心字段即需迁移脚本。
- **lane 删改风险**：lane 归档后历史 ticket 仍引用归档 ID，前端与脚本要持续兼容 `resolveLaneMeta` 的兜底逻辑。
- **多 Agent 并发写入冲突**：目前靠"一次只一个 Agent 在写同一条 ticket"的约定 + `next-ticket-id.sh` 的锁目录，尚未有强并发控制。
- **Agent 配置分发风险**：分发覆盖会替换 OpenCode 现有软链 → 普通文件，软链关系丢失；每个 Agent 工具升级配置目录路径时需要后端 catalog 跟着更新。详见 `000002` / `000003`。
- **依赖**：Rust workspace（bb-server）、Node 前端（web）、qmd 索引、Codex / CodeBuddy / OpenCode 等 Agent 的全局规则同步分发能力。

# 下一步

> 下列子 ticket 尚未分配 ID，留给整理 Agent 或后续基线展开时通过 `create_ticket` 正式落号。

- **[bbt] 多 Agent 心跳 / 占用**：在 ticket 的 `extra` 里增加 `current_agent` / `heartbeat_at` 约定，`bb-server` 提供 `claim_ticket` / `release_ticket` 工具。
- **[bbd] web 看板"按 Agent 视图"**：新增一个维度，展示每个 Agent 当前持有的 doing ticket，支持从 Dashboard 端看到并发占用。
- **[bbd] 看板超期提醒**：在 board 视图上给 `updated_at` 超过阈值的 active ticket 打红标。
- **[bbt+bbd] Agent 配置分发能力**：拆为独立子 ticket：
  - `000002` [bbt] bb-server `agents_config` 模块（catalog + 状态机 + stdio MCP `list/sync/disconnect_agent_connector` + HTTP `GET/POST/DELETE /api/agents/connectors`）。
  - `000003` [bbd] web Settings "Agent 连接器" 区块（状态灯 + Connect/Disconnect/Sync All + 离线降级）。
  - 首版连接器 catalog：codex / codebuddy / opencode。事实源：`blackboard/agents/AGENTS.md`。
- **[bbq] 冒烟基线**：一条脚本化的"多 Agent 并发写入 → 看板四桶一致"端到端冒烟。
- **[bbp] Dashboard 信息架构细化**：二级页面（ticket 详情、inbox 详情、lane 管理、Agent 视图、Settings）的交互与跳转路径画清。

# 参考

- `blackboard/README.md`：面向人类的入口文档、目录结构、lane 说明。
- `blackboard/AGENTS.md`：最小规则，供 Agent 起步阅读。
- `blackboard/agents/AGENTS.md`：跨 Agent 共享规则事实源（codex / codebuddy / opencode 的分发源）。
- `blackboard/bb-server/README.md`：stdio MCP / HTTP 接口清单。
- 同 project 下未来新建的 ticket（002…）承担具体能力实现。
