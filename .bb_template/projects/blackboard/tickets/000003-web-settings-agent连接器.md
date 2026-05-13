+++
id = "000003"
lane = "bbd"
title = "web Settings：Agent 连接器区块（状态灯 + Connect/Disconnect/Sync All）"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
depends_on = "000001"
parent = "000001"
+++

# 当前进展

- 子 ticket 落号 2026-05-06，从产品基线 `000001` 拆出。
- **前端实现完成**（2026-05-06）：
  - `web/src/data/agentConnectors.ts`：load / sync / disconnect 三个 API 包装，离线降级返回 `source: 'offline'`。
  - `web/src/components/AgentConnectorRow.vue`：单行 UI，状态灯 + 路径 + Connect/Disconnect 按钮 + hover tooltip + symlink 角标。
  - `web/src/components/AgentConnectorPanel.vue`：列表容器，loading / offline / source_missing / error 四类降级 + Sync All 一键 + 单点交互。
  - `web/src/views/SettingsView.vue`：复用 AppNav，挂载 AgentConnectorPanel。
  - `web/src/main.ts`：新增路由 `/settings`。
  - `web/src/components/AppNav.vue`：actions slot 旁加 `⚙ Settings` 入口。
- **CodeBuddy 双目标 UI 补正**（2026-05-06）：保持三条 Agent 连接器行；CodeBuddy 行展示 `AGENTS.md` 与 `Rules.mdc` 两个目标路径，不再把 Rules 伪装成独立 Agent 工具。
- **构建通过**：`npm run build --prefix blackboard/web` → vue-tsc + vite 全绿。
- 端到端 UI 验证（人工）待用户在浏览器侧手动跑过。HTTP 路由已经在 `000002` 实施中端到端验证通过。
- 2026-05-06：Settings 路由修复，Settings 合并为 Dashboard shell 内 `/projects/:project/settings` 工作区；修复 connector API 错误分类（网络不可达 vs HTTP 400/500）；修正 codebuddy-internal catalog 目标路径
- 2026-05-06：修复 bb-server `~` 展开跨平台兼容（macOS HOME / Linux HOME / Windows USERPROFILE / HOMEDRIVE+HOMEPATH）；修复 Windows 路径混合斜杠问题；修复 verbatim prefix `\\?\C:\` 展示还原
- 2026-05-07：Settings 连接器容错补齐，Codex 配置 TOML 损坏时只把对应 MCP target 标成 drift/error，不再让 Settings 整页失败；同时修复 Windows `\\?\` registry 路径展示泄露。

# 记录

## 1. 范围

在 web 看板新增 **Settings 视图**，并在其中提供"Agent 连接器"区块，让用户对每个 Agent 一键执行 Connect / Disconnect，并以**状态灯**实时展示 sync 一致性。

不在本 ticket 范围：

- 后端实现（`000002` 负责）。
- catalog 配置外置 UI / 多事实源选择（首版只读后端返回的 catalog）。

## 2. 入口

- 顶部导航 / 项目元数据面板新增 `Settings` 入口（具体放置位置在实现期决定，遵循现有 `BoardView.vue` 风格）。
- 离线降级：bb-server 不可达时整段灰显，顶部提示"启动 bb-server 才能管理 Agent 连接器"。

## 3. 信息架构

### 3.1 顶部摘要条

- 事实源路径（只读，可复制）：`<bb-root>/agents/AGENTS.md`。
- 事实源状态：`source_state`（如果 `source_missing` 用红色横幅替换整段）。
- `Sync All` 按钮：仅当存在非 `synced` 连接器时高亮，点击后顺序对每个非 `synced` 连接器调一次 `POST /api/agents/connectors/{id}/sync`。

### 3.2 连接器列表

每个连接器一行：

- 状态灯（🟢 synced / 🟡 drift / 🔴 missing|unreachable / ⚪ source_missing 时灰显）。
- display_name + Agent id（小字）。
- 目标绝对路径（展开后的）：只读，单击复制；CodeBuddy 行显示两条目标：`AGENTS.md` 与 `Rules.mdc`。
- hover tooltip：source/target sha256 前 8 位、目标 mtime、是否软链；多目标 connector 按 target 分组展示。
- 右侧按钮组：
  - `Connect`（执行 sync）。`drift` / `missing` / `unreachable` 时高亮；`synced` 时降级为"重新同步"次级按钮。CodeBuddy sync 一次写两份文件。
  - `Disconnect`（红色，二次确认弹窗"确认删除目标文件？此操作不会动事实源"）。`missing` / `unreachable` / `source_missing` 时禁用。CodeBuddy disconnect 一次删两份目标文件。

### 3.3 状态颜色映射

| 状态 | 颜色 | 文案 |
|---|---|---|
| `synced` | `#16a34a`（绿） | 已同步 |
| `drift` | `#eab308`（黄） | 有差异 |
| `missing` | `#dc2626`（红） | 未连接 |
| `unreachable` | `#9ca3af`（灰红） | 工具未安装 |
| `source_missing` | `#9ca3af`（灰） | 事实源缺失 |

`is_symlink` 在卡片右上角加一个小角标 `⤴`，hover 显示"目标当前是软链，Connect 会替换为普通文件"。

## 4. 数据层

新增 `web/src/data/agentConnectors.ts`：

- `loadAgentConnectors(): Promise<AgentConnectorList>`
- `syncAgentConnector(id): Promise<AgentConnector>`
- `disconnectAgentConnector(id): Promise<AgentConnector>`

类型定义与 `000002` 后端 JSON 契约一一对应，TypeScript interface 写在同一文件。

后端响应中 connector 保留 `target_path` 等主目标字段用于兼容，同时新增 `targets[]`；UI 展示优先使用 `targets[]`。

## 5. 组件

- `web/src/views/SettingsView.vue`（新建，可能复用 ProjectMeta 面板的卡片样式）。
- `web/src/components/AgentConnectorPanel.vue`（新建，负责数据加载 + 列表渲染 + 整体状态机）。
- `web/src/components/AgentConnectorRow.vue`（新建，单行 UI + 按钮交互）。

不引入新依赖，复用现有 vue 3 + vite 工具链。

## 6. 离线降级

- 调 `loadAgentConnectors` 失败 → 整面板灰显 + 提示"未连接到 bb-server"。
- 不引入 export-tickets-json 风格的 JSON 快照（因为 connector 状态本身依赖运行时探测，离线静态展示无意义）。

## 7. 验收

- `npm run build --prefix blackboard/web` 通过（`vue-tsc -b` + `vite build`）。
- 端到端：bb-server 启动后，浏览器打开 Settings 页，三条连接器各自能完成 sync → 绿灯、disconnect → 红灯、Sync All 一键全绿；CodeBuddy 行能展示并同步 `AGENTS.md` + `Rules.mdc` 两个目标。
- bb-server 关掉刷新后，整面板进入灰显降级，按钮全部禁用。
- 配置损坏降级：`~/.codex/config.toml` 中存在坏 TOML 时，连接器列表仍可加载，目标行展示错误信息并允许用户继续处理其他连接器。

# 下一步

- 串完 `000002`，跑端到端验收。
- 后续可单独建 ticket：
  - 把 Settings 视图扩展为更全面的 Project / Agent / 工具集中管理面板。
  - 连接器分发动作日志在 web 上的可视化（依赖 `000002` 后续 audit log 子 ticket）。

# 参考

- `000001` §4（能力清单）/ §8（风险）/ §下一步。
- `000002`：后端契约来源。
- `blackboard/web/src/views/BoardView.vue`：现有视图风格参照。
- `blackboard/web/src/components/LaneManager.vue`：现有"管理类"模态/面板的风格参照。
