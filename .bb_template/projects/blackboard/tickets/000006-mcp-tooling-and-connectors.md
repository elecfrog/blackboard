+++
id = "000006"
lane = "bbt"
title = "MCP 重构与增强：工具 Schema 模块化 + 三家 Agent MCP 注入"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000005"
parent = "000005"
+++

# 当前进展

- 2026-05-07：扩范围。第 1 阶段（MCP 工具 Schema 模块化）已经完成，第 2 阶段新增：让 Agent 连接器顺手把 bb-server 的 remote MCP endpoint 注入到三家 Agent 的用户级配置里，并支持 disconnect。前端 Settings 页里 MCP 状态与 AGENTS.md 状态并排显示。

- - 2026-05-07：完成 mcp_config 模块（三格式inspect/upsert/remove）+ agents_config 扩（McpServer变体）+ 13条新测试 + 前端状态灯，cargo test 全绿

# 记录

## 1. 一句话范围

把 Blackboard 的 Agent 连接器从"只会分发 AGENTS.md"升级到"顺手把 bb-server 的 remote MCP endpoint 也注入进三家 Agent 的用户级配置里"，并做对应的 disconnect 和前端状态展示。

## 2. 第 1 阶段（已完成）：MCP 工具 Schema 模块化

- 把 MCP 工具定义、参数 schema、调用分发从 `stdio.rs` 传输层拆到 `bb-server/src/mcp_tools.rs`。
- stdio 与 remote `/mcp` 共用同一套工具目录。新增工具只改 `mcp_tools.rs`，不再动传输层。
- 实测 `tools/list` 返回 26 个工具；`cargo test` 通过。

## 3. 第 2 阶段（本次新增）：三家 Agent MCP 配置注入

### 3.1 要注入的 server（首版）

- id：`bb`
- transport：remote HTTP
- url：`http://127.0.0.1:3001/mcp`
- 说明：只分发 remote 形态，不分发 stdio。stdio 形态要求各 Agent 按需拉起 bb-server 进程，三家行为不一致；remote 只需用户自己跑一次 `bb-server http`，三家共享同一个入口。bb-server 的 HTTP 进程守护不在本票范围。

### 3.2 三家目标文件与注入策略

所有注入一律走"增量 upsert，其它 key 原样保留"，绝不整文件覆盖。

| Agent | 文件 | 格式 | MCP 段 | 注入内容 |
|---|---|---|---|---|
| Codex | `~/.codex/config.toml` | TOML | `[mcp_servers.bb]` | `url = "http://127.0.0.1:3001/mcp"` |
| CodeBuddy | `~/.codebuddy/mcp.json` | JSON | `mcpServers.bb` | `{"type":"streamable-http","url":"http://127.0.0.1:3001/mcp","disabled":false,"timeout":30000}` |
| OpenCode | `~/.config/opencode/opencode.json` | JSON | `mcp.bb` | `{"type":"remote","url":"http://127.0.0.1:3001/mcp","enabled":true}` |

### 3.3 跨平台

复用 `bb-core/src/agents_config.rs` 现有的 `resolve_home_from_env_vars`（`$HOME` → `%USERPROFILE%` → `%HOMEDRIVE%%HOMEPATH%` 三路回退）。不新写 home 解析逻辑。

### 3.4 实现路径（不新造抽象层）

在连接器现有的 multi-target 模型上原地扩：

- `AgentConnectorType` 新增变体 `McpServer`，与现有的 `AgentsMd` / `Rules` 并列。
- 每个 Agent 在 catalog 里追加一个 `McpServer` 类型的 target，`target_template` 指向 `~/.codex/config.toml` / `~/.codebuddy/mcp.json` / `~/.config/opencode/opencode.json`。
- 新增模块 `bb-core/src/mcp_config.rs`，只负责三件事：读取现有文件 → upsert 一段 server 定义 → 写回，保留无关 key；或从现有文件里按 key 删除一段。
- 状态机复用：`synced` 表示 `bb` key 存在且 `url` / `type` 与事实源一致；`drift` 表示存在但字段被人改过；`missing` 表示文件可写但 `bb` key 不存在；`unreachable` 表示目标文件的父目录不存在（Agent 未安装）。
- `sync_agent_connector` / `disconnect_agent_connector` 已经是逐 target 循环，只要给 `McpServer` 新加一个分支。

### 3.5 前端（聚合在本票，不另开）

- `AgentConnectorRow.vue` 的 target 列表原本就是循环渲染，`McpServer` 类型自然多出一行。
- 类型标签新增 "MCP"；path 显示注入目标文件（不显示 url，避免 url 重复出现）。
- Connect / Disconnect 按钮行为不变，靠聚合后的连接器状态驱动。

## 4. 验收清单

1. 在当前机器跑一次 "Sync All"。
   - `~/.codex/config.toml` 里原有的 `[mcp_servers.qmd]`、`[mcp_servers.rider]`、以及顶层 `model`、`notify`、`projects.*`、`plugins.*`、`marketplaces.*` 等字段**逐项保留**，diff 只应包含新增的 `[mcp_servers.bb]`。
   - `~/.codebuddy/mcp.json` 里原有的 `mcpServers.kuikly-mcp` / `mcpServers.qmd` 保留，diff 只应包含新增的 `mcpServers.bb`。
   - `~/.config/opencode/opencode.json` 里原有的 `mcp.qmd` / `mcp.context7` / `mcp.gh_grep` / `mcp.MiniMax`，以及顶层的 `tools` / `permission` / `command` 段**完整保留**；原有的 `mcp.bb`（如果已存在）被我们的 upsert 无损覆盖为事实源值；diff 只应包含 `mcp.bb` 的变化。
2. 跑一次 Disconnect：三家配置文件里 `bb` key 全部消失，其余 key 原样保留。
3. bb-server 以 `http --addr 127.0.0.1:3001` 起来后，三家 Agent 都能通过 `bb` MCP 调用 `list_projects` 并拿到当前 workspace 的 project 列表。
4. `cargo test -p bb-core` 与 `cargo test -p bb-server` 全绿，新增单测至少覆盖：
   - 三种格式的 upsert（含"文件不存在时自动创建空骨架")
   - 三种格式的 disconnect（仅删 `bb` key，其他 key 不动）
   - Drift 识别（外部把 `bb` 的 url 改坏后，状态返回 `drift`）
5. Settings 页连接器行里能看到 AGENTS.md 与 MCP 两行 target，任一 drift 会让顶部状态灯变黄。

## 5. 显式不做

- bb-server HTTP 进程的守护 / launchd / systemd 封装：由用户自行管理。
- `<bb-root>/agents/connectors.toml` 覆盖 catalog：保持 `000002` 里"留给后续 ticket"的口径。
- 跨机 remote、反向代理、TLS：不做。
- Agent UI 里显示注入的 url 明文：不做，避免 url 和后端定义漂移。

- 2026-05-07 补丁：sync 推平软链不再静默。bb-core 新增 `AgentConnectorSyncEvent::FlattenedSymlink { target_path, previous_link_target }`，`AgentConnector` / `AgentConnectorTarget` 加 `events: Vec<_>`（空则不序列化，向后兼容）。`sync_with_home` 在 `fs::remove_file` 之前 `read_link` 拿原指向、推平后推事件到 per-target map 并注入回 inspect 快照；严格推平语义一字未改。测试：`sync_replaces_symlink_with_regular_file` 扩展断言 events 契约。`cargo test -p bb-core` 24/24、`cargo build --workspace`、`cargo test -p bb-server` 30/30 全绿。

- 来源：inbox/2026-05-07-claude-mcp-connector-injection.md

- 来源：inbox/2026-05-07-codex-sync-event-structuring.md（codex 独立实现，同步事件结构与 symlink 推平逻辑，测试全绿）

# 下一步

- 按第 3 阶段实现路径施工，完成后等待用户按第 4 节清单做端到端验收。
