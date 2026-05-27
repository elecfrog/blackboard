# TaskGraph Run 终态通知 — 飞书推送调研报告

**研究目标**：为 Ticket 088（TaskGraph 接入飞书消息推送）提供技术事实基础  
**调研范围**：RunOutcome 处理链、daemon 回调点、配置机制、HTTP 客户端依赖、RunStatus 终态  
**产出用途**：迭代 draft（最短路径实现方案、要改的文件、配置方式、分阶段落地、风险和验收标准）

---

## 1. 核心结论（可直接用于迭代决策）

| 决策点 | 结论 | Source |
|--------|------|--------|
| **最佳插入位置** | `daemon.rs::run_graph` 函数末尾，`execute_run` 返回 `RunOutcome` 后立即通知 | `bb_daemon/src/daemon.rs` (run_graph ~line 85-165) |
| **通知范围** | `Succeeded` + `Failed` + `Paused`（对应 ticket 的 ready_for_review） | `bb_core/src/task_graph/pregel/runner.rs:107-113` |
| **HTTP 客户端** | bb_daemon 无 reqwest，需新增依赖（推荐 `reqwest + rustls-tls`） | `bb_daemon/Cargo.toml` |
| **配置方式** | 复用现有三层覆盖机制：TOML section → RunnerOverrides → Agent Profile | `runner_config.rs:70-90, 458-490` |
| **Watch 模式** | 走 `scan_inbox`，在 `record_result` 后、`write_state` 前插入，额外携带 `Dispatch` 上下文 | `daemon.rs:320-345` |
| **blocked 状态** | **RunStatus 枚举中不存在**，是 ticket 系统状态，需跨领域协调 | `run_state/model.rs:23-31` |

---

## 2. 架构分层

```
┌─────────────────────────────────────────────────────────────┐
│                    bb_cli / bb_web                          │
│  (用户触发 TaskGraph run，bb_cli 提供 test-push 子命令)      │
└────────────────────────┬────────────────────────────────────┘
                         │ spawn/run
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    bb_daemon                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ run_graph(&opts) → Result<RunOutcome>                │  │ ← 最佳插入点
│  │   ├─ create_run()          # 创建 run 记录           │  │
│  │   ├─ task_graph::execute_run(&opts)  # 执行 run     │  │
│  │   └─ match outcome { Succeeded/Paused/Failed/Cancelled } │
│  │       └─ [这里插入飞书通知，非阻塞，不改变 exit code]  │  │
│  └──────────────────────────────────────────────────────┘  │
│  scan_inbox (watch 模式)                                    │
│  └─ run_graph → record_result → [通知] → write_state       │
└────────────────────────┬────────────────────────────────────┘
                         │ execute_run
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    bb_core (task_graph)                     │
│  pregel/runner.rs: RunOutcome (Succeeded/Paused/Failed/    │
│                     Cancelled)                              │
│  pregel/coordinator.rs: Phase 5 ReduceAction → update_     │
│                          run_status → 返回 RunOutcome      │
│  run_state/model.rs:   RunStatus (Queued/Pending/Running/   │
│                         Paused/Succeeded/Failed/Cancelled) │
│  run_state/lifecycle.rs: 终态处理 completed_at/active_nodes  │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 关键代码路径

### 3.1 RunOutcome 枚举定义

**文件**：`bb_core/src/task_graph/pregel/runner.rs:107-113`

```rust
pub enum RunOutcome {
    Succeeded,
    Paused { node_id: String },      // ← 对应 ready_for_review
    Failed { node_id: String, message: String },
    Cancelled,
}
```

**终态映射**（ticket 088 语义 vs 代码语义）：

| Ticket 088 状态 | 代码 RunOutcome | 需通知？ |
|----------------|-----------------|---------|
| ready_for_review | `Paused` (human gate) | ✅ |
| failed | `Failed` | ✅ |
| blocked | **不存在**（ticket 系统状态） | ⚠️ 需确认 |
| (隐含) 正常完成 | `Succeeded` | ✅（超出 ticket 范围） |

### 3.2 RunStatus 枚举（持久化层）

**文件**：`bb_core/src/task_graph/run_state/model.rs:23-31`

```rust
pub enum RunStatus {
    Queued,
    Pending,
    Running,
    Paused,       // ← 对应 ready_for_review
    Succeeded,   // 终态
    Failed,      // 终态
    Cancelled,   // 终态
}
```

**终态转移验证**（`lifecycle.rs:562-576`）：
- 终态 `Succeeded`/`Failed`/`Cancelled` 之后**无合法转移路径**
- 进入终态时自动设置 `run.completed_at = Some(now)`，清空 `active_nodes`

### 3.3 最佳插入位置：`run_graph` 函数

**文件**：`bb_daemon/src/daemon.rs` (run_graph ~line 85-165)

```rust
pub async fn run_graph(opts: RunGraphOpts, ...) -> Result<RunOutcome> {
    let run = task_graph::create_run(...);   // run.id/project/graph_ref 已知
    let outcome = task_graph::execute_run(&opts).await?;  // 执行

    // ━━━━━ 插入点（推荐） ━━━━━
    if let Some(feishu) = &feishu_config {
        let _ = send_feishu_card(feishu, &run.id, project, &outcome).await;
        // 注意：不 panic，不改变 outcome，不阻塞主流程（S3 对齐）
    }
    // ━━━━━━━━━━━━━━━━━━━━━━━━━

    Ok(outcome)
}
```

**Watch 模式补充**（`scan_inbox` ~line 320-345）：
- 在 `record_result(state, &dispatch, status, ...)` 之后
- 在 `write_state(state_path, state)` 之前
- 可额外携带 `Dispatch { project, note, content_hash }` 上下文

### 3.4 数据来源（飞书卡片内容）

| 卡片字段 | 数据来源 | 代码位置 |
|---------|---------|---------|
| 项目名 | `project` 函数参数 | `daemon.rs` |
| 任务摘要 | `run.context.input` 或 `graph_snapshot.name` | `run_state/model.rs:269-315` |
| 状态文字 | `RunOutcome` 变体映射 | `daemon.rs` match 分支 |
| 操作链接 | 构造 `{bb_web_url}/project/{project}/runs/{run_id}` | 需在配置中指定 bb_web 地址 |

---

## 4. 配置机制

**文件**：`bb_daemon/src/http/task_graph/runner_config.rs`

### 4.1 三层覆盖优先级

```
CLI overrides  (--feishu-webhook, --feishu-chat-id, etc.)
     > Agent Profile  (bb registry 中 agents/{name}/profile.toml)
     > workspace TOML  (config/task_graph_runner.toml → [feishu] section)
     > 空 (默认不推送)
```

**实现代码**：`runner_config.rs:70-90` (choose_*) + `runner_config.rs:458-490` (resolve_overrides_from_profile)

### 4.2 飞书配置结构（建议）

```toml
# config/task_graph_runner.toml

[feishu]
enabled = true
webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/xxx"
app_id = "${FEISHU_APP_ID}"
app_secret = "${FEISHU_APP_SECRET}"   # 或 vault 引用
web_base_url = "http://localhost:5173"  # bb_web 地址，构造操作链接

# 可选：per-project webhook（多项目不同群）
# [[feishu.projects]]
# project = "blackboard"
# chat_id = "oc_xxx"
```

**注意**：当前项目无通用环境变量覆盖机制。需在新增 `FeishuConfig` 中显式添加 `env::var(...)` 读取。

---

## 5. HTTP 客户端依赖

**文件**：`bb_daemon/Cargo.toml`

| Crate | reqwest | 备注 |
|-------|---------|------|
| bb_daemon | ❌ 无 | 需要新增 |
| bb_server | ✅ 有 | `Cargo.toml line 16: reqwest = { version = "0.12", default-features = false, features = ["stream"] }` |

**推荐依赖声明**：

```toml
# bb_daemon/Cargo.toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
# rustls-tls 避免 OpenSSL 平台差异，Windows 构建友好
```

**注意**：axum 在 bb_daemon 中存在，但属于 MCP server transport 层，不适合直接用于对外发送 HTTP 请求到飞书 API。

---

## 6. Coordinator / Subgraph / Mutation 关系

```
TaskGraph Run
    │
    ├── Coordinator (llm_coordinator.rs)
    │   └── 管理 LLM 节点执行，调用 execute_run
    │   └── Phase 5: ReduceAction::Completed/Paused/Failed → update_run_status → RunOutcome
    │
    ├── Subgraph (subgraph.rs)
    │   └── 子图递归调用 execute_run
    │   └── 子图 run 也有自己的 RunOutcome（需确认是否也要通知）
    │   └── 当前建议：仅在 bb_daemon 顶级调用处统一通知，子图由父级汇总
    │
    └── Mutation 节点
        └── 不影响 run 终态判断
```

**`spawn_task_graph_run` 的实际调用链**（`coordinator.rs:~280-360`）：
- Coordinator 主循环 Phase 5 处理 `ReduceAction::Completed/Paused/Failed/Cancelled`
- 调用 `update_run_status` 更新持久化状态
- 返回 `RunOutcome`

---

## 7. 限制与风险

| 风险 | 描述 | 缓解方案 |
|------|------|---------|
| **token 2h 过期** | 飞书 tenant_access_token 有效期 2h，过期需自动续签 | 实现 token 缓存（内存存储 `expires_at`，请求前检查 <30min 则重新获取） |
| **API 50QPS 限频** | 飞书 API 限频 50QPS | bb_daemon 通常单实例，风险低；如多实例需集中管理 |
| **通知失败不阻塞** | S3 要求：凭证过期或网络不可达时返回明确错误，不 panic 不阻塞 run | `run_graph` 中通知调用结果只打印 stderr，不改变 `outcome` |
| **Watch 模式重复触发** | `scan_inbox` 可能短时间内多次扫描同一 run | 通知前检查 `run.status` 是否已处于终态，或依赖幂等去重 |
| **blocked 状态不存在** | `RunStatus` 枚举中无 `blocked`，这是 ticket 系统状态 | 需确认 blocked 的触发时机，是否需要在 ticket 状态变更时单独发通知 |
| **per-project 配置** | 多 project 共用一个 webhook 可能导致通知混乱 | 支持 `FeishuConfig` 中 per-project `chat_id` 映射 |
| **依赖引入成本** | reqwest 引入额外编译时间和二进制体积 | 评估后如已有其他 HTTP 需求可集中管理，否则接受增量 |
| **bb_cli 本地验证（S4）** | `bb_cli feishu test-push` 需独立实现 | 飞书推送模块设计为可独立调用，bb_cli 子命令复用同一服务 |

---

## 8. 迭代 Draft（最短路径）

### Phase 1：基础设施（最小可验证）

1. **新增依赖**：`bb_daemon/Cargo.toml` 添加 `reqwest` + `rustls-tls`
2. **新增 FeishuConfig**：`bb_daemon/src/config/feishu.rs`
   - 字段：`webhook_url`, `app_id`, `app_secret`, `web_base_url`, `enabled`
   - 实现 token 缓存（2h 过期前自动续签）
3. **配置接入**：修改 `runner_config.rs`，支持 `RunnerOverrides` 中的 feishu 字段和 TOML `[feishu]` section
4. **基础推送函数**：`feishu.rs::send_card`（HTTP POST 到 webhook）
5. **插入通知**：在 `daemon.rs::run_graph` 末尾、match outcome 后插入调用

**验收**：`bb_cli feishu test-push` 能成功发送测试卡片

### Phase 2：卡片内容完善

1. 从 `run.context.input` / `graph_snapshot` 读取任务摘要
2. 构造操作链接 `{web_base_url}/project/{project}/runs/{run_id}`
3. 支持 per-project webhook 配置

**验收**：真实 run 完成后飞书卡片显示项目名、任务名、状态、链接

### Phase 3：韧性增强（S3 对齐）

1. 通知失败时打印 stderr，不 panic，不改变 exit code
2. token 过期自动续签（缓存机制）
3. 网络不可达时返回明确错误（不 panic）

**验收**：模拟网络故障或 token 过期，run 正常完成，通知失败但无崩溃

### Phase 4：Watch 模式适配

1. 在 `scan_inbox` 的 `record_result` 后插入通知
2. 携带 `Dispatch` 上下文（note 信息）增强卡片内容

**验收**：Watch 模式下 run 完成能正常收到通知

---

## 9. 要改的文件清单

| 文件 | 改动类型 | 优先级 |
|------|---------|--------|
| `bb_daemon/Cargo.toml` | 新增 `reqwest` 依赖 | P0 |
| `bb_daemon/src/config/feishu.rs` | 新建，飞书配置 + token 缓存 | P0 |
| `bb_daemon/src/config/mod.rs` | 导出 `FeishuConfig` | P0 |
| `bb_daemon/src/http/task_graph/runner_config.rs` | 增加 feishu 字段到 `RunnerOverrides`，支持 `[feishu]` TOML section | P0 |
| `bb_daemon/src/daemon.rs` | `run_graph` 末尾插入通知调用（`run_graph` + `scan_inbox` 两处） | P0 |
| `.bb_template/config/task_graph_runner.toml` | 添加 `[feishu]` 配置示例 | P1 |
| `bb_cli/src/main.rs`（或 `commands/feishu.rs`） | 新增 `feishu test-push` 子命令（S4） | P1 |

---

## 10. 验收标准（对齐 Ticket 088 四个 Story）

| Story | 验收条件 |
|-------|---------|
| **S1** | 配置飞书凭证后，调用推送模块能成功投递消息（`send_card` 返回 `Ok`） |
| **S2** | Run 进入 `Succeeded`/`Paused`/`Failed` 时，飞书卡片包含：项目名、任务摘要、状态（中文）、操作链接 |
| **S3** | 凭证过期或网络不可达时返回明确错误（日志打印），不 panic，不阻塞 run 主流程 |
| **S4** | `bb_cli feishu test-push` 本地验证可用，测试消息成功发送到飞书群 |

---

**报告完毕。** 所有技术事实均来自 scout 输出的 source anchors，可直接用于迭代 draft 编写。