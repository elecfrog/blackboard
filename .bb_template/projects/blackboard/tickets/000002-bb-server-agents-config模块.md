+++
id = "000002"
lane = "bbt"
title = "bb-server agents_config 模块：Agent 配置连接器后端"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
depends_on = "000001"
parent = "000001"
+++

# 当前进展

- 子 ticket 落号 2026-05-06，从产品基线 `000001` 拆出。
- **后端实现完成**（2026-05-06）：bb-core `agents_config` 模块（catalog + 状态机 + list/sync/disconnect 三组核心函数 + 11 条单测）、bb-server stdio 三个 MCP 工具、HTTP 三条路由全部就位。
- **CodeBuddy 特化补正**（2026-05-06）：保持三条 Agent 连接器（Codex / CodeBuddy / OpenCode），其中 CodeBuddy 单连接器同步两份目标文件：`~/.codebuddy/AGENTS.md` 与 `~/.codebuddy/rules/blackboard-rules.mdc`；Rules `.mdc` 写入时自动追加 `alwaysApply: true` + `type: always` frontmatter，保证 CodeBuddy 自动应用。
- **测试全绿**：`cargo test` → bb-core 52 通过 / bb-server 24 通过。
- **HTTP 端到端验证通过**（2026-05-06）：
  - `GET /api/agents/connectors` 返回三条 Agent 连接器；CodeBuddy 的返回对象包含两个 `targets`。
  - `POST .../{id}/sync` 三连击 → 三条连接器全部 `synced`，CodeBuddy 两份目标文件均与事实源一致。
  - `DELETE .../codex` → 回 `missing`，父目录保留。
  - 再 `POST .../codex/sync` → 重新 `synced`，本机三个 Agent 工具的全局规则均对齐到 `blackboard/agents/AGENTS.md`。
- 等待 `000003` 接通后做"web Settings 页一键 Connect"端到端联调。
- 2026-05-06：CodeBuddy 双 target 实现完成，保持三条连接器，codebuddy 同步 AGENTS.md + Rules.mdc 两个目标，Rules.mdc 设置 alwaysApply: true + type: always；修正 check-ticket-ids.sh 旧 family/assignee 校验规则

# 记录

## 1. 范围

为 Blackboard 提供 **Agent 配置连接器**的后端能力，让 web Settings 页（`000003`）可以通过 HTTP 一键把 `<bb-root>/agents/AGENTS.md` 同步到本机各 Agent 工具的全局规则路径，并实时显示状态。

不在本 ticket 范围：

- 前端 UI 实现（`000003` 负责）。
- catalog 配置外置（首版只允许 `bb-core` 常量；`<bb-root>/agents/connectors.toml` 增量覆盖留给后续 ticket）。
- 分发审计日志写入 inbox / audit log（留给后续 ticket）。

## 2. 事实源

- 路径：`<bb-root>/agents/AGENTS.md`，其中 `<bb-root>` = bb-server `--root` 指向的 `blackboard/` 目录。
- 这是**唯一被人编辑的版本**；分发出去的副本视作只读。
- 副本顶部已带"由 Blackboard 同步、不要直接编辑"提示头，避免回流污染。

## 3. 受支持的连接器（首版）

| Agent id | display_name | 目标路径 |
|---|---|---|
| `codex` | Codex CLI | `~/.codex/AGENTS.md` |
| `codebuddy` | CodeBuddy | `~/.codebuddy/AGENTS.md` + `~/.codebuddy/rules/blackboard-rules.mdc` |
| `opencode` | OpenCode | `~/.config/opencode/AGENTS.md` |

约束：
- 路径模板必须支持 `~` 展开；Windows 使用 `USERPROFILE` 展开，macOS / Linux 使用 `HOME` 展开。
- catalog 用 `bb-core` 常量声明（`AGENT_CONNECTORS`），不硬编码到 `bb-server`。
- `codebuddy` 是一个连接器、两个 target：`AGENTS.md` 按普通 Markdown 写入，`Rules.mdc` 按 CodeBuddy Rules 格式写入 frontmatter。

## 4. 状态机

| 状态 | 含义 | 触发条件 |
|---|---|---|
| `synced` | 目标存在且 `sha256(target) == sha256(source)` | 与事实源一致 |
| `drift` | 目标存在但内容不一致 | 副本被外部修改、或事实源已更新未推 |
| `missing` | 目标父目录可达但文件不存在 | 该 Agent 尚未连接 |
| `unreachable` | 父目录都不存在 | 该 Agent 没装在本机 |
| `source_missing` | `<bb-root>/agents/AGENTS.md` 自身缺失 | 全局错误，所有连接器都会标红 |

附加信息位（不进 state 字段，作为 connector / target 详情独立返回）：
- `is_symlink`：目标当前是软链（首版 sync 仍按"覆盖"策略，会把它替换成普通文件）。
- `targets`：多目标连接器的目标明细；CodeBuddy 首版固定为 `AGENTS.md` + `Rules.mdc`。连接器顶层 `state` 是 targets 的聚合状态。

## 5. stdio MCP 工具

- `list_agent_connectors`（无参数）→ `{ source_state, source_sha256, source_path, connectors: [...] }`。
- `sync_agent_connector { id }` → 把事实源覆盖到该连接器的所有目标（必要时 `mkdir -p` 父目录）；返回**该连接器的新状态对象**。
- `disconnect_agent_connector { id }` → 删除该连接器的所有目标文件（不删父目录）；返回新状态对象。

约束：
- 任何写操作前先校验 `id` 合法、目标路径展开后**不在事实源同源路径**（防止把源覆盖回去）。
- `source_missing` 时 `sync_*` 直接返回错误。

## 6. HTTP 路由

破例开放写入面（与 lane 写入同等位）：

- `GET /api/agents/connectors` → 同 `list_agent_connectors`。
- `POST /api/agents/connectors/{id}/sync` → 同 `sync_agent_connector`。
- `DELETE /api/agents/connectors/{id}` → 同 `disconnect_agent_connector`。

不暴露 `sync_all_agent_connectors`——前端按需多次 POST，避免后端事务模糊。

## 7. 验收

- `cargo test -p bb-core` 全绿，包含至少 6 条新测试：`synced` / `drift` / `missing` / `unreachable` / `source_missing` / `disconnect 后回到 missing`。
- `cargo test -p bb-server` 全绿。
- 启动 `bb-server http`，curl 三条路由，对 codex / codebuddy / opencode 三个连接器顺序：list → sync → list（绿）→ disconnect → list（红 missing）→ sync → list（绿），全部可观察到状态切换；CodeBuddy sync 后需同时存在 `AGENTS.md` 与带 `alwaysApply: true`、`type: always` 的 `blackboard-rules.mdc`。

# 下一步

- 等 `000003` 接通后做端到端：从 web Settings 页点 Connect → 后端 sync → 状态灯切绿。
- 后续可单独建 ticket：
  - catalog 外置到 `<bb-root>/agents/connectors.toml`。
  - 分发动作 audit log。
  - Claude / Cursor / Rider 等连接器扩展。

# 参考

- `000001` §4（能力清单）/ §8（风险）。
- `blackboard/agents/AGENTS.md`：事实源。
- `blackboard/bb-server/README.md`：现有 stdio + HTTP 接口风格。
