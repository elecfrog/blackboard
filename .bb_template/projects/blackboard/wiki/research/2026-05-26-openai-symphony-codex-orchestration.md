# OpenAI Symphony Codex 编排框架调研报告

**数据日期**：2026-05-26  
**调研时间**：2026-05-26  
**输出路径**：`wiki/research/2026-05-26-openai-symphony-codex-orchestration.md`

---

## TL;DR

Symphony 是 OpenAI 开源的一个**高度垂直的 Codex 任务编排服务**，而非通用工作流框架。它以 Linear ticket 为工作单元，通过单权威 Orchestrator 的轮询驱动（tick-based poll loop），协调多个隔离的 Codex Agent Workspace 并发执行。其核心设计亮点包括：六层分层架构、`WORKFLOW.md` Policy-as-Code 契约、`linear_graphql` 客户端工具扩展、以及 stall 检测 + 指数退避重试机制。

**与 Blackboard TaskGraph 的本质差异**：Symphony 是外部事件驱动的单权威 Daemon 服务（Tracker 状态 → Orchestrator 决策 → Agent 执行），非 DAG 图执行；TaskGraph 更接近 LangGraph 的 Pregel/Superstep 模型，支持多节点分布式 Supervisor 树和更丰富的迭代语义。**两者可直接互补而非替代**：Symphony 的 workspace 隔离模型、WORKFLOW.md 契约格式、tracker 驱动调度可作为 TaskGraph 的上层 Dispatcher 参考。

> ⚠️ Symphony 当前为 **prototype/preview 阶段**（SPEC.md 标注 Draft v1），不建议直接用于生产。

---

## 调研范围与数据来源

| 维度 | 内容 |
|---|---|
| **主要来源** | `github.com/openai/symphony`（SPEC.md、elixir/lib/、elixir/README.md、elixir/WORKFLOW.md） |
| **官方博客** | `openai.com/index/open-source-codex-orchestration-symphony/`（JS 渲染页面未完整获取） |
| **框架对比** | `github.com/langchain-ai/langgraph`（README、checkpoint/src/base.ts） |
| **时间范围** | 2026-05（发布即调研，无历史版本对比） |

**Scout 覆盖情况**：✅ 覆盖 SPEC.md 全 17 节 + Elixir 参考实现完整源码；✅ 覆盖 agent-orchestration-and-communication；✅ 覆盖 task-decomposition-and-coordination（含 Elixir GenServer 源码行号引用）；✅ 覆盖 tool-integration-patterns（含 dynamic_tool.ex、app_server.ex 源码）；✅ 覆盖 comparison-with-langgraph-and-pregel。**未覆盖**：Codex app-server 协议具体 JSON-RPC schema（需运行 `codex app-server generate-json-schema` 动态生成）。

---

## 1. Symphony 概述与定位

### 1.1 核心定位

Symphony 的定位从其 [Problem Statement](https://github.com/openai/symphony/blob/main/SPEC.md#1-problem-statement) 可直接引用：

> "The next evolution of harness engineering — from managing coding agents to managing the work that needs to be done."

Symphony 试图将项目工作转变为**隔离的自主执行运行**，使团队能够管理"需要完成的工作"本身，而非监督单个编码 Agent。它是 OpenAI "harness engineering" 理念的产物，官方定位为 **"low-key engineering preview"**，仅限可信环境测试。

### 1.2 语言无关规范 + Elixir 参考实现

Symphony 采用独特的双轨设计：

- **[SPEC.md](https://github.com/openai/symphony/blob/main/SPEC.md)**：语言中立的服务契约文档，定义完整的行为规范和算法伪代码（Section 16），任何编程语言均可据此实现。
- **Elixir/BEAM 参考实现**：基于 OTP supervisor 模型，擅长管理长期运行进程，支持热代码重载而不停止活跃的 Agent 子进程。

README 明确建议开发者让喜欢的 coding agent 依据 SPEC.md 用任意语言实现自己的 Symphony。

---

## 2. 架构设计：六层分离

Symphony 规范定义了**六个抽象层次** [[SPEC.md §3.1](https://github.com/openai/symphony/blob/main/SPEC.md#3-1-abstraction-levels)]：

| 层次 | 职责 | 关键组件 |
|---|---|---|
| **Policy Layer** | repo 定义的 `WORKFLOW.md` prompt body | Liquid 模板渲染 |
| **Configuration Layer** | YAML front matter 类型化读取 | 环境变量插值 `$VAR`、路径展开 `~` |
| **Coordination Layer** | 轮询、eligibility、并发、重试、对账 | `Orchestrator`（GenServer） |
| **Execution Layer** | workspace + agent 子进程 | `WorkspaceManager`、`AgentRunner` |
| **Integration Layer** | Linear tracker 适配 | `LinearAdapter`（GraphQL） |
| **Observability Layer** | 结构化日志 + 可选 API/dashboard | JSON REST API、Phoenix LiveView |

```
Policy Layer (WORKFLOW.md)
       ↓
Configuration Layer (YAML front matter)
       ↓
Coordination Layer ←→ Integration Layer (Linear)
       ↓
Execution Layer (Workspace + AgentRunner)
       ↓
Observability Layer
```

核心组件交互：
- **Orchestrator**：唯一的调度状态权威，拥有内存中的 `running`、`claimed`、`blocked`、`retry_attempts`、`completed` 状态映射 [[elixir/orchestrator.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/orchestrator.ex#L30-L43)]。
- **Workspace Manager**：为每个 issue 创建隔离的文件系统目录，管理生命周期钩子（after_create/before_run/after_run/before_remove） [[SPEC.md §9](https://github.com/openai/symphony/blob/main/SPEC.md#9-workspace-management-and-safety)]。
- **Agent Runner**：创建/reuse workspace、按 WORKFLOW.md template 构建 prompt、通过 app-server subprocess 启动 Codex session。

---

## 3. Agent 编排模型与通信机制

### 3.1 单权威轮询调度模型

Symphony 的 Orchestrator 是一个 **GenServer tick loop**，以固定周期（默认 30s）对 Linear 发起 `fetch_candidate_issues()` 拉取候选工作 [[elixir/orchestrator.ex tick handle_info](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/orchestrator.ex#L94-L108)]：

```elixir
# Orchestrator.tick/1 的核心循环
def handle_info(:tick, state) do
  state = state |> run_poll_cycle() |> run_reconciliation() |> schedule_tick()
  {:noreply, state}
end
```

**关键约束**：所有调度决策串行化于单一 Orchestrator，无多协调者选举。Agent 间通信完全不存在——每个 issue 独立运行 [[agent-orchestration-and-communication finding](https://github.com/openai/symphony/blob/main/SPEC.md#7-orchestration-state-machine)]。

### 3.2 Issue 编排状态机

每个 Linear issue 在服务内部有独立的编排状态机 [[SPEC.md §7.1](https://github.com/openai/symphony/blob/main/SPEC.md#7-1-issue-orchestration-states)]：

```
Unclaimed → Claimed → Running
                     ↓ ↓
            RetryQueued → Released
```

状态转换全部由 Orchestrator 显式驱动，所有 worker 结果通过 Elixir `send/2` 消息流上报。

### 3.3 通信机制

Symphony 实现了**三层通信** [[comparison-with-langgraph-and-pregel finding](https://github.com/openai/symphony/blob/main/SPEC.md#8-polling-scheduling-and-reconciliation)]：

1. **Tracker 轮询**：定时拉取 Linear API 获取候选 issue，排序后分发（priority 升序 → created_at 最旧 → identifier 字典序）。
2. **事件流上报**：Agent Runner 通过 stdio 与 Codex app-server 协议通信，将 `session_started`/`turn_completed`/`approval_request` 等事件流上报给 Orchestrator [[elixir/agent_runner.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/agent_runner.ex#L52-59)]。
3. **客户端工具调用**：`linear_graphql` 客户端工具让 Agent 在 session 内直接执行 Linear GraphQL mutation，orchestrator 保持"scheduler/runner/tracker reader"的边界清晰。

### 3.4 Turn 延续机制（类 Pregel Superstep）

单个 issue 的 Agent 生命周期内，Symphony 支持最多 `max_turns`（默认 20）个连续 Codex turn，同一 `thread_id` 会话保持活跃 [[SPEC.md §10.3](https://github.com/openai/symphony/blob/main/SPEC.md#10-3-continuation-processing)]：

- 每次 turn 正常完成后，`do_run_codex_turns` 重新查询 Linear issue 状态；若仍处于 active 状态且未达 max_turns，则开始下一轮 turn（携带继续引导 prompt 而非重新渲染原始 prompt） [[elixir/agent_runner.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/agent_runner.ex#L116-162)]。
- 这与 Pregel 顶点活跃则进入下一 Superstep 的语义**相近**，但 Symphony 是 poll-based 而非 BSP 同步迭代。

---

## 4. 任务分解与协调策略

### 4.1 任务粒度：Issue 为原子单位

Symphony **不做自动子任务分解**，任务粒度由 Linear ticket 决定 [[SPEC.md §2](https://github.com/openai/symphony/blob/main/SPEC.md#2-goals-and-non-goals)]。Agent 行为通过两个机制控制：

1. **WORKFLOW.md Markdown body** 的 prompt template，渲染时注入 issue 对象（含 identifier/title/description/state/labels/blockers）和 `attempt` 变量 [[SPEC.md §5.4](https://github.com/openai/symphony/blob/main/SPEC.md#5-4-prompt-template-contract)]。workflow 可以区分首次运行和续续运行的指令。
2. **agent.max_turns**（默认 20）限制单次 worker session 中的最大 turns 数。

### 4.2 隐式依赖图

Symphony 不构建显式的任务图 DAG，依赖管理通过 Linear 原生的 blocking 关系实现 [[elixir/orchestrator.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/orchestrator.ex#L824-840)]：

- `fetch_candidate_issues()` 结果经过 `todo_issue_blocked_by_non_terminal?` 过滤——若 Todo 状态的 issue 有非终态的 blocker，则**不具备派发资格** [[SPEC.md §4.1.1](https://github.com/openai/symphony/blob/main/SPEC.md#4-1-1-issue-entity)]。
- `blocked_by` 字段从 Linear GraphQL 的**反向 blocks 关系**归一化而来 [[elixir/linear/adapter.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/linear/adapter.ex)]。

### 4.3 三层并发控制

Symphony 通过三层并发控制协调多 Agent [[SPEC.md §8.3](https://github.com/openai/symphony/blob/main/SPEC.md#8-3-concurrency-control)]：

| 层级 | 机制 | 默认值 |
|---|---|---|
| 全局限制 | `max_concurrent_agents` | 10 |
| 按状态槽位 | `max_concurrent_agents_by_state` | 按 Linear state key 细分 |
| SSH Host Pool | `select_worker_host` least_loaded | 亲和性 + 每主机槽位 |

可用槽位 = `max(max_concurrent_agents - running_count, 0)`。

### 4.4 重试策略

Symphony 对不同退出原因采用**差异化重试策略** [[SPEC.md §8.4](https://github.com/openai/symphony/blob/main/SPEC.md#8-4-retry-and-backoff)]：

- **正常退出**（clean worker exit）：1s 固定短延迟继续重试（attempt=1）。
- **异常退出**：指数退避 `delay = min(10000 * 2^(attempt-1), max_retry_backoff_ms)`，最大 5 分钟。
- **input_required blocker**：进入内存 blocked 状态，需人工介入或重启服务解锁。

### 4.5 主动对账（Reconciliation）

每个 tick 的 reconciliation 包含两部分 [[SPEC.md §8.5](https://github.com/openai/symphony/blob/main/SPEC.md#8-5-active-run-reconciliation)]：

1. **Stall 检测**：计算 elapsed_ms 自 `last_codex_timestamp`，超过 `stall_timeout_ms`（默认 5 分钟）则终止 worker 并重试。
2. **Tracker 状态刷新**：批量获取所有 running issue 的最新状态——终态则终止 worker 并清理 workspace；仍活跃则更新内存 snapshot；非活跃非终态则终止 worker 但不清理。

### 4.6 三层超时处理

| 超时类型 | 默认值 | 行为 |
|---|---|---|
| `turn_timeout_ms` | 1h | 单个 Codex turn 最大耗时 |
| `stall_timeout_ms` | 5min | 无活动检测 → 退避重试 |
| `retry_backoff` | 指数退避 | 最大 5min，上限 10 次幂 |

---

## 5. 工具集成方式

### 5.1 Codex App-Server Protocol

Symphony 通过 **Codex app-server 的 JSON-RPC 2.0 over stdio 协议**与 Codex 通信 [[SPEC.md §10](https://github.com/openai/symphony/blob/main/SPEC.md#10-agent-runner-protocol-coding-agent-integration)]。工具定义采用 JSON Schema 格式：

```json
// DynamicTool.tool_specs() 输出示例
[
  {
    "name": "linear_graphql",
    "description": "Execute Linear GraphQL queries",
    "inputSchema": {
      "type": "object",
      "properties": {
        "query": { "type": "string" },
        "variables": { "type": "object" }
      },
      "required": ["query"]
    }
  }
]
```

在 `thread/start` 时通过 `dynamicTools` 字段向 Codex 会话宣告客户端工具（advertise tools） [[elixir/dynamic_tool.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/dynamic_tool.ex#L13-L22)] [[SPEC.md §10.2](https://github.com/openai/symphony/blob/main/SPEC.md#10-2-session-startup-responsibilities)]。

### 5.2 工具调用协议

当收到 `{"method":"item/tool/call", "id":<id>, "params":{"tool":<name>, "arguments":<args>}}` 时，app_server.ex 从 params 提取 tool name 和 arguments，委托 `DynamicTool.execute/2` 执行 [[elixir/app_server.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/app_server.ex#L430-L460)]：

```json
// 响应格式（必须包含三字段）
{
  "id": <id>,
  "result": {
    "success": true,
    "output": "...",
    "contentItems": [{ "type": "inputText", "text": "..." }]
  }
}
```

SPEC 规定：若 agent 请求不支持的工具调用，返回结构化 failure 并继续 session，不应让 session 卡死。

### 5.3 linear_graphql 客户端工具

Symphony 当前唯一的内置客户端工具 `linear_graphql` [[elixir/dynamic_tool.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/dynamic_tool.ex#L38-L58)]：

- **执行时复用 Symphony 配置的 Linear auth**（`tracker.api_key` / `LINEAR_API_KEY`），不要求 agent 读取原始 token。
- 执行结果区分 GraphQL 层面 success（无 top-level errors）与 transport 层面 success。
- **核心设计原则**：ticket 状态转换和评论通过此工具由 agent 驱动，而非 orchestrator 业务逻辑——orchestrator 保持调度边界清晰 [[SPEC.md §10.5](https://github.com/openai/symphony/blob/main/SPEC.md#105-approval-tool-calls-and-user-input-policy)]。

### 5.4 Approval Policy

Symphony 在 WORKFLOW.md 的 `codex.approval_policy` 配置审批策略 [[app_server.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/app_server.ex#L255-L273)]：

| Policy | 行为 |
|---|---|
| `on-failure` | 失败时批准 |
| `on-request` | 按需批准 |
| `never` | 全部 auto_approve（高风险） |
| `untrusted` | 拒绝所有（安全模式） |

默认值为 `{"reject":{"sandbox_approval":true,"rules":true,"mcp_elicitations":true}}`。工具调用默认 auto_approve（high-trust 模式），在不受信环境中存在安全风险 [[SPEC.md §15.5](https://github.com/openai/symphony/blob/main/SPEC.md)]。

### 5.5 工具扩展机制

新增工具只需在 `DynamicTool.execute/2` 中添加分支并通过 `tool_specs/0` 导出，无需修改协议层代码——这是**语言中立的实现契约** [[SPEC.md §18.2](https://github.com/openai/symphony/blob/main/SPEC.md#182-recommended-extensions)]。

---

## 6. 与 LangGraph / Pregel / TaskGraph 的对比

### 6.1 三种执行范式对比

| 维度 | Symphony | LangGraph | Pregel/Superstep |
|---|---|---|---|
| **计算模型** | 外部事件驱动 Daemon（Tracker 状态→Orchestrator→Agent） | Pregel 启发的 DAG 状态机 | BSP superstep 同步迭代 |
| **协调拓扑** | 单权威轮询（无图结构） | 有向图节点 + channel 消息传递 | vertex-centric + 全局 barrier |
| **任务粒度** | Linear ticket 级别（issue-level） | 可细粒度定义子任务图 | 顶点级别 |
| **状态持久化** | Orchestrator in-memory，tracker/filesystem 恢复 | checkpointer 快照（channel_values/channel_versions） | 全局持久 coordinator 状态 |
| **通信机制** | poll-based tracker + stdio 事件流 | 图节点间消息传递 | superstep 内 vertex-to-vertex 消息 |
| **Agent 间通信** | 无（每 issue 独立） | 图边显式定义 | superstep 消息传递 |
| **实现语言** | 语言无关（SPEC）+ Elixir 参考 | Python-first | Google C++（原生） |
| **Tracker 集成** | Linear 原生（核心依赖） | 无（通用框架） | 无 |

### 6.2 与 LangGraph 的具体差异

LangGraph 明确说明灵感来源于 **Pregel 和 Apache Beam** [[LangGraph README Acknowledgements](https://github.com/langchain-ai/langgraph/blob/main/README.md#acknowledgements)]。其 `checkpoint/base.ts` 中的 Checkpoint 接口（channel_values/channel_versions/versions_seen）与 Pregel 的 superstep 语义高度对齐。

核心差异 [[comparison-with-langgraph-and-pregel finding](https://github.com/openai/symphony/blob/main/SPEC.md#2-goals-and-non-goals)]：

- **关注点不同**：LangGraph 等是**图定义框架**，描述 Agent 间的数据流和有向图；Symphony 是**工作级编排服务**，管理多 ticket 并发执行和长期运行。
- **任务粒度**：LangGraph 可细粒度定义子任务图，Symphony 粒度为 ticket 级别，不做自动子任务分解。
- **通信机制**：LangGraph 用图节点间消息传递，Symphony 用 poll-based tracker 驱动 + Codex app-server 协议。

### 6.3 与 Pregel/Superstep 的具体差异

| 维度 | Symphony | Pregel/Superstep |
|---|---|---|
| **同步机制** | tick-based poll loop（异步，无全局 barrier） | barrier-based superstep（BSP 同步） |
| **拓扑依赖** | 各 issue workspace 完全独立（blocker 仅作为派发 eligibility） | 图顶点间消息传递依赖 |
| **状态管理** | Orchestrator 状态完全 in-memory | 全局持久 coordinator 状态 |
| **执行单元** | per-issue workspace（文件系统隔离） | per-vertex computation（内存分区） |
| **迭代终止** | issue 达非 active 状态 | 顶点进入 inactive 状态（所有消息处理完） |

**相似点**：都强调隔离执行单元（Symphony: per-issue workspace；Pregel: per-vertex computation）和某种形式的同步/协调机制。

### 6.4 Symphony vs Blackboard TaskGraph

| 维度 | Symphony | TaskGraph |
|---|---|---|
| **调度权威** | 单一 GenServer 全权调度，所有决策串行化 | Supervisor 树 + MCP 分布式，调度分布多个 node |
| **Agent 间通信** | 完全不存在（每 issue 独立） | handoff/inbox 机制本身就是通信模式 |
| **任务粒度** | issue-level 单 agent | 同一 ticket 内支持多子任务 agent fan-out |
| **迭代模型** | turn 循环（非真正迭代），不跨 session 恢复状态 | Pregel-like model，支持更丰富的迭代语义 |
| **状态持久化** | in-memory orchestrator，重启靠 tracker reconciliation | checkpoint 持久化 |

---

## 7. 对 Blackboard TaskGraph 的借鉴意义

以下设计对 TaskGraph 有直接迁移/借鉴价值，按优先级排列：

### 7.1 Workspace 隔离模型（高优先级）

Symphony 为每个 issue 创建隔离的 workspace 目录 `<workspace.root>/<sanitized_issue_identifier>`，包含三个强制安全不变式 [[SPEC.md §9](https://github.com/openai/symphony/blob/main/SPEC.md#9-workspace-management-and-safety)]：

1. **cwd 必须在 per-issue workspace 路径内**：launch 前验证 `cwd == workspace_path`。
2. **Workspace 路径必须在 workspace root 前缀内**：拒绝任何越界路径。
3. **Workspace key 归一化**：仅允许 `[A-Za-z0-9._-]`。

→ **迁移方案**：TaskGraph 可借鉴此模型强化 node workspace 隔离，但需支持更细粒度的 sandbox 管理（Symphony 的 per-issue 粒度在 TaskGraph 多节点并行执行场景下可能过粗）。

### 7.2 WORKFLOW.md Policy-as-Code 契约（高优先级）

Symphony 的 `WORKFLOW.md` 采用 Markdown + YAML front matter 格式，前半部分 YAML 定义配置，后半部分 Markdown body 作为 per-issue prompt template [[SPEC.md §5.2](https://github.com/openai/symphony/blob/main/SPEC.md#5-2-file-format)]：

- 文件由 repo 团队版本控制，无需带外服务配置。
- 运行时通过文件系统 watch **自动热重载**配置，无需重启服务。
- 扩展机制允许添加额外 top-level key 而不破坏核心 schema。

→ **迁移方案**：TaskGraph 的 skill definition 和 prompt template 可采用类似 YAML+Markdown 契约格式，实现 repo-owned policy。但需注意 Symphony 的 prompt-centric 与 TaskGraph 的 structured behavior definition（JSON BDD）存在哲学差异，混用可能导致上下文二义性。

### 7.3 Tracker 驱动的外部协调（中优先级）

Symphony 用 Linear 作为事实来源，`blocked_by` 过滤作为隐式依赖图 [[SPEC.md §8.2](https://github.com/openai/symphony/blob/main/SPEC.md#8-2-candidate-selection-rules)]。这种"外部状态驱动派发"的模式可映射到 TaskGraph 用 ticket/inbox 作为协调机制。

→ **迁移方案**：Symphony 的 Orchestrator 层（调度+重试+并发）可作为 TaskGraph 的上层 Dispatcher，TaskGraph Pregel 执行引擎作为下层执行单元，通过统一的状态快照接口对接。

### 7.4 Stall 检测 + 指数退避重试（中优先级）

Symphony 的 `reconcile_stalled_running_issues` 定期检查 `last_codex_timestamp` 距当前的毫秒数，超过阈值则终止并退避重试 [[elixir/orchestrator.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/orchestrator.ex#L557-620)]。这对长时运行的 Agent 任务有直接借鉴价值。

### 7.5 Hook 生命周期机制（低优先级）

`after_create`/`before_run`/`after_run`/`before_remove` 四个钩子 [[SPEC.md §9.4](https://github.com/openai/symphony/blob/main/SPEC.md#9-4-workspace-hooks)] 可扩展的 workspace lifecycle，为 TaskGraph 的 node lifecycle hooks 提供参考。

---

## 8. 限制与风险

### 8.1 产品阶段风险

- Symphony 官方定位为 **"low-key engineering preview"**（[SPEC.md §2](https://github.com/openai/symphony/blob/main/SPEC.md#2-goals-and-non-goals)），SPEC.md 标注 **"Draft v1"**，Elixir 实现标注 **"prototype software"**，直接生产使用存在稳定性风险。
- Orchestrator **无持久化状态**（纯 in-memory），重启后通过 tracker reconciliation 恢复，与 TaskGraph 的 checkpoint 机制在语义上不兼容。

### 8.2 功能限制

- **仅支持 Linear** 作为 tracker [[SPEC.md §11](https://github.com/openai/symphony/blob/main/SPEC.md#11-issue-tracker-integration-contract-linear-compatible)]，任何其他 tracker 需自行适配。
- **强依赖 Codex app-server 协议**，若 Blackboard Agent 运行在 Codex 以外的环境（如 Claude/cline），工具集成层需要完全重写。
- **任务粒度锁定在 ticket 级别**，无法表达 DAG 内部并行/依赖 [[agent-orchestration-and-communication finding](https://github.com/openai/symphony/blob/main/SPEC.md)]。
- **`linear_graphql` 是唯一的客户端工具**，扩展其他工具需修改 `DynamicTool` 模块，无开箱即用的插件机制。
- **tick-based polling 有最小 5 秒轮询间隔**的延迟下限，不适合低延迟响应需求。

### 8.3 安全风险

- `linear_graphql` 扩展暴露了完整的 Linear GraphQL 访问能力，SPEC 明确要求部署时进行 harness hardening 并收窄权限范围。
- `approval_policy = "never"` 时所有操作 auto_approve，在不受信环境中存在严重风险。
- Symphony 不验证工具输出内容的正确性或安全性（[SPEC.md §15.5](https://github.com/openai/symphony/blob/main/SPEC.md) 指出 tracker data、repo contents、prompt inputs 不可信）。

### 8.4 与 TaskGraph 的整合风险

- Symphony 与 TaskGraph 的模型差异较大：前者是外部轮询+状态机，后者是 Pregel DAG+Superstep。两者架构整合需要设计**专门的桥接层**，不能简单替换或叠加。
- Symphony 的 per-issue workspace 隔离模型在 TaskGraph 多节点并行执行场景下**粒度过粗**。

---

## 9. 后续建议

1. **短期（调研验证）**：在 `bb_web` 或独立 POC 中实现 Symphony Orchestrator 的概念验证，用 `WORKFLOW.md` 契约格式定义 TaskGraph 的 skill prompt 模板，验证热重载机制。
2. **中期（能力增强）**：为 TaskGraph 引入 stall 检测和指数退避重试机制，增强长运行任务的可靠性；参考 Symphony 的 workspace isolation 设计，强化 TaskGraph node 的 sandbox 边界。
3. **长期（架构演进）**：设计 TaskGraph 的上层 Dispatcher 接口，允许接入 Symphony 风格的 tracker-driven 任务源（Linear/GitHub Issues 等）；或反之，让 Symphony 的 Orchestrator 层通过 MCP 调用 TaskGraph 的 Pregel 执行引擎作为下层执行单元。
4. **并行研究**：持续关注 LangGraph 的 `checkpointer` 接口演进（与 TaskGraph 的 Pregel/Superstep 语义对齐度最高）和 Symphony 的 SPEC.md 成熟度。

---

## 来源清单

| # | 来源 | 类型 | 覆盖内容 |
|---|---|---|---|
| 1 | [github.com/openai/symphony/SPEC.md](https://github.com/openai/symphony/blob/main/SPEC.md) | 规范文档 | 全 17 节核心规范 |
| 2 | [github.com/openai/symphony/elixir/lib/symphony_elixir/orchestrator.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/orchestrator.ex) | 源码 | GenServer tick loop、调度算法 |
| 3 | [github.com/openai/symphony/elixir/lib/symphony_elixir/agent_runner.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/agent_runner.ex) | 源码 | Turn 延续、多轮执行 |
| 4 | [github.com/openai/symphony/elixir/lib/symphony_elixir/workspace.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/workspace.ex) | 源码 | Workspace 隔离与安全 |
| 5 | [github.com/openai/symphony/elixir/lib/symphony_elixir/codex/dynamic_tool.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/dynamic_tool.ex) | 源码 | 工具注册与执行 |
| 6 | [github.com/openai/symphony/elixir/lib/symphony_elixir/codex/app_server.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/codex/app_server.ex) | 源码 | App-Server 协议、工具调用路由 |
| 7 | [github.com/openai/symphony/elixir/lib/symphony_elixir/linear/adapter.ex](https://github.com/openai/symphony/blob/main/elixir/lib/symphony_elixir/linear/adapter.ex) | 源码 | Linear tracker 适配、blocked_by 归一化 |
| 8 | [github.com/openai/symphony/elixir/WORKFLOW.md](https://github.com/openai/symphony/blob/main/elixir/WORKFLOW.md) | 配置文件 | WORKFLOW.md 实际格式示例 |
| 9 | [github.com/openai/symphony/elixir/README.md](https://github.com/openai/symphony/blob/main/elixir/README.md) | 文档 | Elixir 实现使用方式、配置 |
| 10 | [github.com/openai/symphony/README.md](https://github.com/openai/symphony/blob/main/README.md) | 文档 | 官方概览、demo 链接 |
| 11 | [github.com/langchain-ai/langgraph/blob/main/README.md](https://github.com/langchain-ai/langgraph/blob/main/README.md) | 文档 | LangGraph 架构、Pregel 启发声明 |
| 12 | [github.com/langchain-ai/langgraph/blob/main/libs/checkpoint/src/base.ts](https://github.com/langchain-ai/langgraph/blob/main/libs/checkpoint/src/base.ts) | 源码 | Checkpoint 接口定义 |