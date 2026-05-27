以下是最终合成的调研报告：

---

# Blackboard TaskGraph 飞书消息推送接入 — 内部代码调研与迭代 Draft

**报告编号**: 088-feishu-push-draft-internal  
**任务**: 调研 bb_backend 中 taskgraph integrations、tool_layer、run_state 事件钩子、feishu/webhook 节点定义、config 层凭证管理  
**来源**: scout 任务 4 个子域（taskgraph-integration-feishu-nodes / tool-layer-registration / run-state-events-hooks / config-credentials-management）+ 外部调研 wiki/research/088-feishu-push-draft-external-research.md  
**日期**: 2026-05-27  

---

## 1. 核心结论

### 1.1 已有能力（可复用）

| 能力 | 路径 | 说明 |
|------|------|------|
| **NodeRole::FeishuNotify** | `bb_core/src/task_graph/nodes/registry.rs:41` | 枚举值已定义，`from_config_value()` 支持字符串 `"feishu_notify"`，属于预留槽位 |
| **RuntimeBindingKind::Webhook** | `bb_core/src/task_graph/nodes/registry.rs:53` | 枚举值已定义，暗示系统已考虑 webhook 绑定场景 |
| **PermissionKind::ExternalNotify** | `bb_core/src/task_graph/nodes/registry.rs:88` | 通知权限已定义但未在任何节点中声明 |
| **ToolLifecycleEvent 结构化事件** | `bb_core/src/task_graph/run_state/tool_lifecycle.rs:1-160` | 定义了 Start/Update/End 事件和 ToolLifecycleStatus，payload 序列化已实现 |
| **append_run_event() API** | `bb_core/src/task_graph/run_state/superstep.rs:257` | 追加事件到 `run_dir/events.jsonl`，支持 kind/node_id/message/payload，seq 自增保证顺序 |
| **GraphCoordinator 事件分发** | `bb_core/src/task_graph/pregel/coordinator.rs` | 完整事件流：run_started → superstep_started → node_started → node_finished → writes_committed → checkpoint_saved → superstep_completed → run_completed/run_failed/run_paused |
| **HTTP SSE 实时事件流** | `bb_daemon/src/http/task_graph/events.rs` | notify crate 监听文件系统变更，SSE 推送至 `/api/projects/{project}/task-graph-runs/{run_id}/events`，终态时关闭流 |
| **NodeSpec 角色化节点定义架构** | `bb_core/src/task_graph/nodes/registry.rs:120-360` | NodeSpec 包含 category/node_type/role/runtime/permissions/artifact_outputs，`builtin_node_specs()` 返回所有内置节点，`node_spec_for()` 按 role 查找 |
| **McpServerConfig.env 凭证注入** | `bb_core/src/agents_registry/model.rs:58` | `BTreeMap<String,String>` 支持环境变量注入（但针对 MCP server 进程，不适合 HTTP API 凭证） |
| **resolve_tool_injection_plan()** | `bb_core/src/task_graph/tool_layer.rs:27` | 统一工具注入，KNOWN_LLM_TOOLKITS 定义了 blackboard_mcp 和 browser_use |

### 1.2 关键缺口

| 缺口 | 严重度 | 说明 |
|------|--------|------|
| FeishuNotify 节点无 NodeSpec/Executor | 🔴 高 | `NodeRole::FeishuNotify` 存在但不在 `builtin_node_specs()` 中，缺少节点定义、pins、runtime binding |
| Webhook RuntimeBinding 无执行器 | 🔴 高 | `RuntimeBindingKind::Webhook` 仅有枚举，无 `runtime_for_node_type()` 实现 |
| bb_core 无 HTTP 客户端 | 🔴 高 | `reqwest` 仅在 bb_server，bb_core 无法直接发起 HTTP 调用 |
| 无 Run-level 事件订阅机制 | 🟡 中 | 现有 `append_run_event()` 仅支持追加，无 publish-subscribe 分发给外部消费者 |
| 无专用飞书凭证存储 | 🟡 中 | `McpServerConfig.env` 不适合 HTTP API token，需要独立的凭证管理模块 |
| NodeType 无 Webhook/Notification 变体 | 🟡 中 | `bb_core/src/task_graph/definition/types.rs:272` 的 NodeType 枚举缺少 Webhook 变体 |
| 无事件 schema 注册表 | 🟡 中 | 第三方难以可靠解析 events.jsonl 的 payload 结构 |

### 1.3 父级迭代目标映射

| 父级目标 | 对应内部实现 | 当前状态 |
|----------|-------------|---------|
| 模块结构 | NodeSpec 架构（role 化节点） | FeishuNotify 槽位已预留 |
| API 封装 | HTTP API 直调 + 自定义 token 管理 | 无 HTTP 客户端，需引入 reqwest |
| token 管理 | tenant_access_token + 30min 刷新策略 | 无实现 |
| 消息卡片模板 | Interactive Card JSON 结构 | 无实现 |
| 与 TaskGraph run 状态集成 | GraphCoordinator 事件分发 + append_run_event | 有基础，缺订阅钩子 |
| 分阶段落地计划 | — | 见第 8 节 |
| 风险和验收标准 | — | 见第 9-10 节 |

---

## 2. 架构分层

```
┌─────────────────────────────────────────────────────┐
│                 User / AI Agent                      │
└────────────────────┬────────────────────────────────┘
                     │ triggers run
┌────────────────────▼────────────────────────────────┐
│         GraphCoordinator (pregel/coordinator.rs)     │
│  事件流: run_started → ... → run_completed/failed    │
└────────────────────┬────────────────────────────────┘
                     │ append_run_event()
┌────────────────────▼────────────────────────────────┐
│     RunState  (run_state/model.rs / superstep.rs)     │
│  RunStatus: Queued/Pending/Running/Paused/           │
│             Succeeded/Failed/Cancelled                │
│  events.jsonl (append only, seq ordered)              │
└──────┬──────────────────────────┬────────────────────┘
       │ 读                        │ 监听
┌──────▼──────────┐    ┌───────────▼────────────────────┐
│  HTTP SSE 流     │    │   EventSubscriber 钩子（新增）  │
│  events.rs       │    │   run_started/run_completed/   │
│  (daemon 层)     │    │   run_failed 触发通知          │
└──────────────────┘    └───────────┬────────────────────┘
                                    │ call
                    ┌───────────────▼────────────────────┐
                    │  FeishuNotifyNode (新增 NodeSpec)   │
                    │  RuntimeBindingKind::Webhook        │
                    │  FeishuWebhookExecutor (新增)      │
                    └───────────────┬────────────────────┘
                                    │ HTTP POST
                    ┌───────────────▼────────────────────┐
                    │  FeishuClient (HTTP API 直调)       │
                    │  ├── FeishuTokenManager            │
                    │  ├── MessageBuilder (Card JSON)    │
                    │  └── RetryWithBackoff              │
                    └───────────────┬────────────────────┘
                                    │ HTTPS
                    ┌───────────────▼────────────────────┐
                    │     飞书开放平台 API                 │
                    │  im/v1/messages + auth/v3/         │
                    │  tenant_access_token                │
                    └────────────────────────────────────┘
```

**分层职责**：
- **GraphCoordinator**：产生 run-level 事件，不感知飞书
- **RunState**：追加事件到 events.jsonl，不分发
- **EventSubscriber**：新增层，监听 RunState 状态变更，调用 FeishuNotifyNode
- **FeishuNotifyNode**：TaskGraph 节点定义，描述"向谁发什么"
- **FeishuWebhookExecutor**：节点执行器，持有 FeishuClient 实例
- **FeishuClient**：HTTP 客户端封装，含 token 管理和重试逻辑

---

## 3. 关键代码路径

### 3.1 节点注册与扩展点

| 文件 | 行号 | 关键内容 | 用途 |
|------|------|---------|------|
| `bb_core/src/task_graph/nodes/registry.rs` | 41 | `NodeRole::FeishuNotify` | 枚举值（已定义） |
| `bb_core/src/task_graph/nodes/registry.rs` | 53 | `RuntimeBindingKind::Webhook` | 枚举值（已定义） |
| `bb_core/src/task_graph/nodes/registry.rs` | 88 | `PermissionKind::ExternalNotify` | 权限（已定义） |
| `bb_core/src/task_graph/nodes/registry.rs` | 389 | `'feishu_notify' in from_config_value()` | 字符串解析支持 |
| `bb_core/src/task_graph/nodes/registry.rs` | 120-360 | `builtin_node_specs()` | 新增 FeishuNotify NodeSpec 位置 |
| `bb_core/src/task_graph/nodes/registry.rs` | 527-600 | `runtime_for_node_type()` | 新增 Webhook runtime binding 位置 |
| `bb_core/src/task_graph/definition/types.rs` | 272-309 | `NodeType` 枚举 | 无 Webhook 变体（需扩展） |

### 3.2 事件与状态系统

| 文件 | 行号 | 关键内容 | 用途 |
|------|------|---------|------|
| `bb_core/src/task_graph/run_state/model.rs` | 24-35 | `RunStatus` 枚举 | 状态定义 |
| `bb_core/src/task_graph/run_state/model.rs` | 146 | `RunEvent` 结构体 | 事件结构（kind/node_id/message/payload） |
| `bb_core/src/task_graph/run_state/superstep.rs` | 257 | `append_run_event()` | 追加事件到 events.jsonl |
| `bb_core/src/task_graph/run_state/lifecycle.rs` | 391-420 | `update_run_status()` | 状态机验证与持久化 |
| `bb_core/src/task_graph/run_state/tool_lifecycle.rs` | 1-160 | `ToolLifecycleEventKind` / `append_tool_lifecycle_event()` | LLM tool 生命周期（非 run-level） |
| `bb_core/src/task_graph/pregel/coordinator.rs` | — | `GraphCoordinator` | 事件分发核心 |
| `bb_daemon/src/http/task_graph/events.rs` | — | SSE 事件流 | 已有 HTTP 推送，可参考模式 |

### 3.3 工具注入与凭证

| 文件 | 行号 | 关键内容 | 用途 |
|------|------|---------|------|
| `bb_core/src/task_graph/tool_layer.rs` | 27 | `KNOWN_LLM_TOOLKITS` | 工具包定义 |
| `bb_core/src/task_layer.rs` | 14-17 | `DEFAULT_BLACKBOARD_MCP_DIRECT_TOOLS` | 默认 MCP 工具集 |
| `bb_core/src/agents_config/mcp_connector/mod.rs` | — | upsert/inspect/remove 配置 | 凭证管理模式参考 |
| `bb_core/src/agents_registry/model.rs` | 58 | `McpServerConfig.env` | 环境变量注入（不直接适用） |

---

## 4. 执行模型

### 4.1 当前 Node 执行模型

TaskGraph 节点通过 `runtime_for_node_type()` 选择执行器：

```
NodeSpec {
    category: Agent,
    node_type: Llm,
    role: FeishuNotify,        // ← 新增角色
    runtime: Webhook,          // ← 新增 runtime 类型
    permissions: [ExternalNotify],
    artifact_outputs: [...]
}
```

执行链路：
1. Coordinator 在对应 superstep 调用节点执行
2. `runtime_for_node_type()` 返回 `RuntimeBinding`
3. 已有：LLM Runtime → AgentSession，Shell Runtime → shell_node.rs
4. **缺失**：Webhook Runtime → 无执行器

### 4.2 飞书节点执行链路（设计）

```
FeishuNotifyNode 执行流程：

1. Coordinator dispatch(node=FeishuNotify, phase=node_started)
   ↓
2. runtime_for_node_type(role=FeishuNotify) → RuntimeBindingKind::Webhook
   ↓
3. FeishuWebhookExecutor::execute(params: FeishuNodeParams)
   ├── FeishuTokenManager::get_token()        // 获取/刷新 token
   ├── resolve_message_template(run_state)    // 从 run context 填充卡片变量
   ├── FeishuClient::send_message(chat_id, card_json)
   │   ├── POST /open-apis/im/v1/messages?receive_id_type=chat_id
   │   ├── Authorization: Bearer <token>
   │   └── Retry on 429 / token invalid
   └── append_tool_lifecycle_event(End, Succeeded)
```

**替代轻量方案（Phase 1）**：不新建节点，通过 `ExternalNotify` 权限的 Shell 节点调用 Python/curl 脚本：
```bash
curl -X POST "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id" \
  -H "Authorization: Bearer $FEISHU_TOKEN" \
  -H "Content-Type: application/json" \
  -d @/tmp/feishu_card.json
```
**优点**：无需引入 reqwest 到 bb_core  
**缺点**：token 管理复杂，无结构化错误处理

---

## 5. 数据/状态模型

### 5.1 飞书凭证存储

```
FeishuCredentials (新增模型)
├── app_id: String           // cli_xxx 格式
├── app_secret: String       // 加密存储
├── default_chat_id: String  // 默认推送群组
└── token_cache: Option<(token: String, expire_at: DateTime)>

存储位置: bb_backend/config/ 或项目 .bb_template/.bb_config/feishu.toml
注入方式: 通过 bb_config 层读取，不硬编码
```

### 5.2 飞书节点参数（Node Pins）

```json
{
  "role": "feishu_notify",
  "params": {
    "chat_id": "",
    "user_id": "",
    "template": "run_completed",        // run_completed | run_failed | run_blocked
    "trigger_on": ["run_completed", "run_failed"],
    "custom_card": false
  },
  "pins": {
    "run_context": "upstream_node.output.run_context",
    "target_chat_id": "config.default_chat_id"
  }
}
```

### 5.3 Run 状态与事件流转

```
RunStatus 状态机:
  Queued → Pending → Running → Succeeded
                           → Failed
                           → Cancelled
                           → Paused

RunEvent 事件序列 (append_run_event → events.jsonl):
  run_started(run_id, project, intent)
  superstep_started(seq, nodes)
  node_started(node_id)
  node_finished(node_id, status, outputs)
  writes_committed(node_id, artifact_keys)
  checkpoint_saved(run_id, checkpoint_path)
  superstep_completed(seq)
  run_completed(run_id, final_status, duration)
  run_failed(run_id, error_reason)
  run_paused(run_id)

ToolLifecycleEvent (LLM tool 级别，非 run 级别):
  tool_start(tool_name, params)
  tool_update(tool_name, progress)
  tool_end(tool_name, result)
```

### 5.4 消息卡片数据结构

```
FeishuCard (TaskGraph Run 通知)
├── config.enable_forward: true
├── header.template: green|red|orange|grey  (按状态)
├── header.title.content: "TaskGraph Run: {run_id}"
├── elements:
│   ├── div: **Run**: {run_id} | **Status**: {status} | **Duration**: {duration}
│   ├── div: **Project**: {project_name} | **Trigger**: {trigger_type}
│   ├── div: **Summary**: {success_count} succeeded / {fail_count} failed
│   ├── action: [View Run] [View Logs]
│   └── note: "Blackboard 自动通知 · 勿直接回复"
└── card_link.url: "{bb_web_url}/projects/{project}/runs/{run_id}"
```

---

## 6. Coordinator / Subgraph / Mutation 关系

### 6.1 GraphCoordinator 事件分发

`bb_core/src/task_graph/pregel/coordinator.rs` 中的 `GraphCoordinator` 是事件分发的核心：

```
GraphCoordinator 关键方法:
  append_event() → append_run_event()
  
  事件分发序列:
  run_started        → Run 进入 Running 状态时
  superstep_started  → 每个 superstep 开始
  node_started       → 节点开始执行
  node_finished      → 节点执行完成
  writes_committed   → artifact 写入确认
  checkpoint_saved   → 快照保存完成
  superstep_completed→ superstep 完成
  run_completed      → 最终状态 = Succeeded
  run_failed         → 最终状态 = Failed
  run_paused         → 最终状态 = Paused
  
  tool_start/update/end → LLM tool 调用生命周期
```

### 6.2 Subgraph 与 FeishuNotify 的关系

FeishuNotify 节点在 TaskGraph 定义中有两种使用形态：

**形态 A：显式节点**（推荐）
```
TaskGraph JSON 定义:
{
  "nodes": [
    { "id": "feishu_notify_on_complete", "role": "feishu_notify", "params": {...} }
  ],
  "edges": [
    { "from": "llm_final", "to": "feishu_notify_on_complete" }
  ]
}
```
→ 节点在 `builtin_node_specs()` 中注册，Coordinator 正常调度

**形态 B：Subgraph 封装**
```
{
  "nodes": [
    { "id": "notify_subgraph", "role": "subgraph", "subgraph_id": "feishu_notify_tmpl" }
  ]
}
```
→ Subgraph 内包含 feishu_notify 节点，可跨项目复用模板

### 6.3 Mutation 与 FeishuNotify

Mutation（写边）不影响 FeishuNotify 节点调度，因为通知节点通常无上游数据依赖（读取 `run_context` 从 Coordinator 全局状态，而非通过写边传递）。通知节点可配置 `allow_no_inputs: true`。

---

## 7. 限制与风险

### 7.1 技术风险

| 风险 | 级别 | 缓解措施 |
|------|------|---------|
| bb_core 无 HTTP 客户端 | 🔴 高 | Phase 1 用 Shell 节点 + curl；Phase 2 将 reqwest 引入 bb_core 的 feishu 子模块 |
| FeishuNotify 无执行路径 | 🔴 高 | 完成 NodeSpec + RuntimeBinding + Executor 三层实现 |
| token 凭证泄露 | 🔴 高 | 环境变量注入或加密存储，禁止硬编码 |
| Run 状态变更无外部订阅 | 🟡 中 | 新增 EventSubscriber trait + 实现，在 Coordinator lifecycle 中注册 |
| 应用未发布/机器人未开启 | 🟡 中 | Phase 0 用自定义机器人（Webhook 模式，无需发布） |
| 飞书 API Rate Limit（50 QPS） | 🟡 中 | 推送层加 rate limiter，429 时按 `x-ogw-ratelimit-reset` 等待 |
| 无事件 schema 注册表 | 🟡 中 | 定义 FeishuEvent payload schema，文档化已知字段 |
| NodeType 枚举需扩展 | 🟢 低 | Webhook 节点可通过 role 扩展（无需改 NodeType） |

### 7.2 产品风险

| 风险 | 级别 | 缓解措施 |
|------|------|---------|
| 用户停止接收机器人消息 | 🟡 中 | 业务侧处理（飞书 230053 错误码） |
| 消息超长（>30KB） | 🟢 低 | 内容截断和精简 |
| DLP 审查拦截（电话/邮箱） | 🟡 中 | 消息内容脱敏 |
| 飞书 SDK 版本滞后 | 🟢 低 | HTTP API 直调为主，SDK 备选 |
| 批量推送超过 5 QPS/用户 | 🟡 中 | 应用层发送队列 + 速率控制 |

---

## 8. 分阶段落地计划

### Phase 0：环境准备（不涉及代码）
- 创建飞书自建应用，获取 `app_id` / `app_secret`
- 开启机器人能力
- 创建测试群组，获取 `chat_id`
- 本地测试用自定义机器人（Webhook，无需发布）

### Phase 1：基础推送（独立 Python/Shell 模块）
**目标**：验证飞书 API 连通性  
**交付物**：
```
bb_backend/integrations/feishu/
├── feishu_client.py          # HTTP API 直调 + token 管理
│   ├── FeishuTokenManager     # get_token() with 30min refresh
│   └── send_card()           # POST im/v1/messages
├── message_builder.py         # 卡片 JSON 模板
└── __init__.py
```

**技术决策**：
- 不引入 reqwest 到 bb_core，用 Python 脚本 + Shell 节点调用
- token 存储在 `~/.bb_config/feishu.toml`（环境变量注入）
- 用 `curl` 或 `python -m urllib.request` 发送 HTTP 请求

**验收标准**：
- [ ] `python -m bb_backend.integrations.feishu.feishu_client` 向指定 chat_id 发送卡片消息
- [ ] token 续签生效（2h 内不重复获取）
- [ ] HTTP 429 触发正确等待并重试

### Phase 2：FeishuNotify NodeSpec（bb_core Rust 实现）
**目标**：TaskGraph 支持 feishu_notify 节点类型  
**交付物**：
```
bb_core/src/task_graph/nodes/
├── feishu_notify/
│   ├── mod.rs                 # NodeSpec 定义
│   ├── params.rs              # FeishuNodeParams
│   └── executor.rs             # FeishuWebhookExecutor
└── registry.rs               # NodeRole::FeishuNotify 注册
```

**改动点**：
1. `registry.rs`: 在 `builtin_node_specs()` 添加 FeishuNotify NodeSpec
2. `registry.rs`: 在 `runtime_for_node_type()` 添加 `RuntimeBindingKind::Webhook` 路径
3. 新增 `RuntimeBindingKind::Webhook` 的 `WebhookRuntime`
4. 新增 `FeishuWebhookExecutor::execute()`
5. 在 `bb_core/Cargo.toml` 将 `reqwest` 从 bb_server 引入到 feishu_notify 子模块

**验收标准**：
- [ ] TaskGraph JSON 支持 `"role": "feishu_notify"`
- [ ] 节点执行后飞书收到消息
- [ ] 无效 chat_id 返回明确错误，不 panic

### Phase 3：Run 状态事件集成
**目标**：Run 完成/失败时自动触发飞书通知  
**交付物**：
```
bb_core/src/task_graph/
├── run_state/event_subscriber.rs   # EventSubscriber trait（新增）
│   └── trait EventSubscriber {
│         fn on_run_started(&self, event: &RunEvent)
│         fn on_run_completed(&self, event: &RunEvent)
│         fn on_run_failed(&self, event: &RunEvent)
│     }
├── pregel/coordinator.rs           # 注册 EventSubscriber 到生命周期
└── integrations/feishu/
    └── feishu_subscriber.rs        # 实现 EventSubscriber
```

**改动点**：
1. 新增 `EventSubscriber` trait，定义 run-level 事件钩子
2. Coordinator 在 `run_started` 时注册 subscriber，在终态时调用 `on_run_*`
3. `FeishuSubscriber` 消费 run_context，构建卡片，调用 `FeishuClient`
4. 配置文件支持 `notify_on: [run_completed, run_failed, run_paused]`

**验收标准**：
- [ ] TaskGraph run 进入 Succeeded/Failed 时，自动推送飞书卡片
- [ ] 用户可通过节点配置覆盖全局通知策略

### Phase 4：CLI 本地测试命令
**目标**：开发者快速验证推送是否正常  
**交付物**：
```bash
# bb_cli feishu test-push --chat-id oc_xxx
bb_cli feishu send \
  --chat-id <chat_id> \
  --template run_completed \
  --run-id <run_id>
```

### Phase 5：生产完善
**目标**：生产级健壮性  
**交付物**：
- Rate limit 队列（应用层 5 QPS 控制）
- 幂等发送（message_id 去重）
- 监控告警（推送失败 > 3 次触发警告）
- 消息内容 DLP 脱敏

---

## 9. 验收标准

### 9.1 功能验收

| # | 验收条件 | 验证方式 |
|---|---------|---------|
| V1 | 发送文本/卡片消息到指定 chat_id 和 user_id，返回 message_id | curl 或 `bb_cli feishu send` |
| V2 | token 在 2h 有效期内不重复获取；< 30min 时自动刷新 | 日志验证 token 请求次数 |
| V3 | HTTP 429 触发等待 `x-ogw-ratelimit-reset` 后重试 | 模拟限流测试 |
| V4 | token invalid（code=4001）触发强制刷新重试 | 模拟过期 token |
| V5 | TaskGraph run 进入 Succeeded/Failed/Paused 时自动推送 | 完整 run smoke test |
| V6 | 卡片 header 颜色对应 run 状态（green/red/orange） | 目视验证 |
| V7 | `bb_cli feishu test-push` 命令可快速验证连通性 | CLI smoke test |
| V8 | 飞书推送失败不阻塞 TaskGraph run 继续执行 | 断网测试 |
| V9 | 卡片包含 Run ID、项目名、状态、时长、操作链接 | 内容断言 |
| V10 | DLP 审查通过（消息无明文电话/邮箱） | 敏感字段测试 |

### 9.2 非功能验收

| # | 验收条件 | 说明 |
|---|---------|------|
| N1 | 推送延迟 < 5s（从 run 终态到飞书收到消息） | 端到端延迟 |
| N2 | 推送错误有结构化日志（包含 run_id、chat_id、error_code） | 可调试性 |
| N3 | 凭证不硬编码，通过环境变量或配置文件注入 | 安全 |
| N4 | 无 panic 不导致 TaskGraph run 中断 | 健壮性 |

---

## 10. 附录：关键文件索引

| 文件路径 | 描述 | 角色 |
|---------|------|------|
| `bb_core/src/task_graph/nodes/registry.rs` | NodeRole / RuntimeBindingKind / NodeSpec / builtin_node_specs | **主扩展点** |
| `bb_core/src/task_graph/pregel/coordinator.rs` | GraphCoordinator 事件分发 | **集成触发点** |
| `bb_core/src/task_graph/run_state/model.rs` | RunStatus / RunEvent 定义 | 状态数据模型 |
| `bb_core/src/task_graph/run_state/superstep.rs` | append_run_event() API | 事件持久化 |
| `bb_core/src/task_graph/run_state/lifecycle.rs` | update_run_status() 状态机 | 状态变更 |
| `bb_core/src/task_graph/run_state/tool_lifecycle.rs` | ToolLifecycleEvent 结构化事件 | 参考实现 |
| `bb_core/src/task_graph/definition/types.rs` | NodeType 枚举 | 类型扩展参考 |
| `bb_core/src/task_graph/tool_layer.rs` | resolve_tool_injection_plan() | 工具注入参考 |
| `bb_core/src/agents_config/mcp_connector/mod.rs` | 第三方配置管理模式 | 凭证管理参考 |
| `bb_core/src/agents_registry/model.rs` | McpServerConfig.env | 环境变量注入参考 |
| `bb_daemon/src/http/task_graph/events.rs` | SSE 事件流实现 | HTTP 推送参考 |
| `projects/blackboard/tickets/000065-taskgraph-integrations-feishu-webhook-plugins.json` | 父级 ticket（第三方生态） | 产品定义 |
| `projects/blackboard/tickets/000088-飞书-sdk-cli-消息推送接入.json` | 当前 ticket（088） | 产品定义 |
| `wiki/research/088-feishu-push-draft-external-research.md` | 外部飞书 API 调研 | 技术参考 |

---

*综合调研报告 | 数据源: 内部代码 scout (run-20260527-060810-53f0ec8c) + 外部飞书 API 调研 | 不输出本地文件，由 system_write_output 写入 output_path*