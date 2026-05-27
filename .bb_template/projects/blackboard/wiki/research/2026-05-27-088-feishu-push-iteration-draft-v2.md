两份报告已读取完毕，现在根据它们作为主要事实源合成迭代 draft。

---

# Ticket 088 飞书推送迭代 Draft

**版本**：v2  
**状态**：初稿，供 Agent 评审后迭代  
**依赖事实源**：
- 内部调研：`088-feishu-push-draft-v2-internal-research.md`（以下标注「内部事实」）
- 外部调研：`088-feishu-push-draft-v2-external-research.md`（以下标注「外部事实」）

---

## 一、目标与非目标

### 目标（Stories 覆盖）

| Story | 内容 | 验收对应 |
|-------|------|---------|
| **S1** | 配置飞书凭证后，调用推送模块成功投递消息 | `send_card` 返回 `Ok` |
| **S2** | Run 进入终态（`ready_for_review`/`failed`/`blocked`）时，自动推送结构化卡片（含项目名、任务摘要、状态、操作链接） | 真实 run 完成后群内收到卡片 |
| **S3** | 凭证失效或网络不可达时返回明确错误，不 panic，不阻塞 run | 模拟故障后 run 正常完成 |
| **S4** | `bb_cli feishu test-push` 本地验证可用 | CLI 命令成功发送测试卡片 |

### 非目标

- 不实现 @ 指定用户（推断：需要 open_id，当前阶段不引入）
- 不实现多群动态路由（推断：多群场景属 Phase 2+ 需求）
- 不在 `bb_daemon` 之外（如 `bb_web`）触发推送
- 不使用自建应用 Bot（推断：Webhook 方案完全覆盖全部 4 个 Story，无 token 管理负担）

---

## 二、内部现状事实（来源：内部调研报告）

### 2.1 HTTP 客户端现状

> 内部事实：`bb_daemon` 当前无任何 HTTP 客户端依赖。`reqwest` 在 `bb_server` 中存在（`bb_server/Cargo.toml` 第 16 行），但 `bb_daemon` 独立编译，不共享依赖树。需在 `bb_daemon/Cargo.toml` 显式新增 `reqwest`。

### 2.2 插入点确认

> 内部事实：`run_graph` 函数（`daemon.rs` ~line 85-165）位于 `execute_run` 返回 `RunOutcome` 之后，是通知的最优插入点。Watch 模式（`scan_inbox` ~line 320-345）在 `record_result` 之后、`write_state` 之前追加通知。

### 2.3 RunOutcome → 终态映射

| ticket 088 终态 | 代码中的枚举 | 需通知 |
|----------------|------------|-------|
| `ready_for_review` | `RunOutcome::Paused { node_id }` | ✅ |
| `failed` | `RunOutcome::Failed { node_id, message }` | ✅ |
| `blocked` | **不存在于 `RunOutcome`**（是 ticket 系统状态，非 TaskGraph run 状态） | ⚠️ 见下方说明 |

**关于 `blocked` 的推断**：内部报告指出 `blocked` 不属于 `RunStatus`/`RunOutcome` 枚举，而是 ticket 系统状态（来自 Blackboard ticket 生命周期，而非 TaskGraph run 生命周期）。推断：088 应在 ticket 状态变为 `blocked` 时单独触发飞书通知，这需要额外的 ticket 状态监听，而非在 `run_graph` 中处理。**建议开子票确认 blocked 触发机制。**

### 2.4 配置机制可用

> 内部事实：`runner_config.rs:70-90` 已有三层覆盖机制（CLI overrides > Agent Profile > TOML），`RunnerOverrides` 结构可扩展添加 feishu 字段。无需新建配置读取框架。

---

## 三、外部启发（来源：外部调研报告）

### 3.1 Webhook vs Bot 选型

> 外部事实：自定义机器人 Webhook 方案完全满足 ticket 088 全部 4 个 Story。对比：Webhook 无需 token，Bot 需要处理 tenant_access_token 2h 过期续签（外部事实：「当剩余有效期 < 30 分钟时获取接口返回新 token」）。**选型结论：采用 Webhook 方案，最短路径，无需 token 管理。**

### 3.2 Webhook 技术参数

> 外部事实：Webhook 限频 100次/分钟、5次/秒，请求体最大 20KB，消息类型支持 `interactive`（卡片）。成功响应 `{"code":0,"msg":"success"}`。

### 3.3 卡片结构

> 外部事实：卡片 JSON 有 1.0/2.0 两套 Schema，推荐使用 2.0（`"schema":"2.0"`）。四要素：`header`（含 `title`、`template` 颜色）、`body`（含 `elements`）、`config`（宽屏模式）、可选 `card_link`。自定义机器人卡片最大 20KB（外部事实）。

### 3.4 错误处理策略

> 外部事实：限频返回 `99991400`，应读取 `x-ogw-ratelimit-reset` 延迟重试。参数错误 `230001`、权限错误 `230035` 不重试。内部错误 `10500/10101` 可指数退避重试最多 3 次。S3 容错要点：HTTP 超时 10-15s，捕获 `net.DialError`/`net.Timeout`，推送失败 fire-and-forget 不阻断主流程。

---

## 四、拟改架构

### 4.1 模块位置

```
bb_daemon/src/
├── feishu/               # 新建目录
│   ├── mod.rs            # 模块入口，导出 send_card / FeishuConfig
│   ├── config.rs         # FeishuConfig 定义（webhook_url, keyword, enabled, web_base_url）
│   ├── card.rs           # 卡片 JSON 组装（Interactive Card 2.0）
│   └── client.rs         # HTTP POST 到 Webhook URL，含超时和错误处理
├── http/
│   └── task_graph/
│       └── runner_config.rs  # 修改：RunnerOverrides 增加 feishu 字段
└── daemon.rs             # 修改：run_graph 末尾 + scan_inbox 中插入通知调用
```

### 4.2 数据流

```
run_graph(opts)
    ├─ create_run()           → run_id, project, graph_ref
    ├─ execute_run(&opts)     → RunOutcome
    └─ notify_feishu(&feishu_config, &run, project, &outcome)
          ├─ FeishuConfig::resolve()   # 读取 TOML / env / CLI override
          ├─ build_card(run, project, outcome)  # 组装 Interactive Card 2.0 JSON
          └─ client::post(&config.webhook_url, card_json, timeout=10s)
                ├─ HTTP 200 + code==0  → Ok
                ├─ HTTP 429 / 99991400 → FeishuRateLimited（记录日志，不阻塞）
                ├─ 网络超时/不可达      → FeishuNetworkError（记录日志，不阻塞）
                └─ 异常                 → 打印 stderr，Ok (fire-and-forget)
```

### 4.3 卡片内容设计（对应 S2）

```json
{
  "msg_type": "interactive",
  "card": {
    "schema": "2.0",
    "config": { "wide_screen_mode": true },
    "header": {
      "title": { "tag": "plain_text", "content": "🎉 TaskGraph Run 完成" },
      "template": "blue"   // green=成功, red=失败, yellow=ready_for_review
    },
    "body": {
      "elements": [
        { "tag": "div", "text": { "tag": "lark_md",
          "content": "**项目**: {project_name}\n**状态**: {outcome_label}\n**摘要**: {run_context_input_truncated}" }},
        { "tag": "hr" },
        { "tag": "actions", "actions": [
          { "tag": "button", "text": { "tag": "plain_text", "content": "查看详情 →" },
            "type": "primary", "behaviors": { "type": "open_url",
              "default_url": "{web_base_url}/project/{project}/runs/{run_id}" }}
        ]}
      ]
    }
  }
}
```

| 字段 | 来源 | 备注 |
|------|------|------|
| `project_name` | `project` 函数参数 | 内部事实 |
| `outcome_label` | `RunOutcome` match 映射为中文 | Paused→待审核, Failed→失败, Succeeded→成功 |
| `run_context_input` | `run.context.input` | 内部事实，可截断至 200 字符 |
| `run_id` | `run.id` | 内部事实 |
| `web_base_url` | `FeishuConfig.web_base_url` | 需在配置中指定 bb_web 地址 |

---

## 五、Graph / Node / Runtime / Data Contract 改动

### 5.1 Graph 改动

| 改动点 | 说明 |
|--------|------|
| 新增 `bb_daemon/src/feishu/` 模块 | 推送功能封装 |
| 新增 `bb_daemon/Cargo.toml` 依赖 | `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`（内部事实） |

### 5.2 Node 改动

| 改动点 | 说明 |
|--------|------|
| `daemon.rs::run_graph` | 末尾 match outcome 后插入 `notify_feishu`，`let _ =` 接收结果不改变 `outcome` |
| `daemon.rs::scan_inbox` | `record_result` 后、`write_state` 前插入通知，携带 `Dispatch` 上下文 |

### 5.3 Runtime 改动

| 改动点 | 说明 |
|--------|------|
| `FeishuConfig` 解析 | 读取 TOML `[feishu]` section，支持 `web_base_url` 指定 bb_web 地址 |
| `RunnerOverrides` 扩展 | 新增可选 feishu override 字段（CLI `--feishu-webhook-url` 等） |

### 5.4 Data Contract 改动

| 改动点 | 说明 |
|--------|------|
| `FeishuConfig` 结构 | `webhook_url`（必填）、`keyword`（可选，Webhook 安全配置）、`web_base_url`（必填，构造链接）、`enabled`（默认 true） |
| Run record 无需改动 | 推送不写入持久化状态，fire-and-forget |

---

## 六、阶段计划

### Phase 1：最小可运行（覆盖 S1 + S4）

**目标**：`bb_cli feishu test-push` 可发送文本消息到飞书群

1. `bb_daemon/Cargo.toml` 新增 `reqwest` 依赖（内部事实）
2. 新建 `bb_daemon/src/feishu/config.rs`：`FeishuConfig` 结构含 `webhook_url`、`keyword`、`web_base_url`、`enabled`
3. 新建 `bb_daemon/src/feishu/client.rs`：`post` 函数，HTTP POST，10s 超时，返回 `Result<(), FeishuError>`
4. 新建 `bb_daemon/src/feishu/mod.rs`：导出 `send_card`
5. `bb_cli` 新增 `feishu test-push` 子命令（S4）
6. `.bb_template/config/task_graph_runner.toml` 添加 `[feishu]` 配置示例

**验收**：S1 ✅（模块可成功发送），S4 ✅（CLI 命令可用）

### Phase 2：卡片消息（覆盖 S2）

**目标**：真实 run 完成后群内收到结构化卡片

1. 新建 `bb_daemon/src/feishu/card.rs`：卡片 JSON 组装（Interactive Card 2.0，`header`+`body`，状态颜色映射：`green`/`red`/`yellow`）
2. 修改 `daemon.rs::run_graph`：插入 `notify_feishu`，从 `run` 提取 `id`/`context.input`，从 `outcome` 映射状态中文名
3. 修改 `runner_config.rs`：扩展 `RunnerOverrides` 支持 feishu 字段，解析 `[feishu]` TOML section
4. `FeishuConfig.web_base_url` 配置 bb_web 地址，构造操作链接

**验收**：S2 ✅（卡片含项目名、任务摘要、状态、操作链接）

### Phase 3：容错增强（覆盖 S3）

**目标**：故障场景下 run 正常完成，通知失败有明确日志

1. `client.rs` 定义 `FeishuError` 枚举：`NetworkError`/`Timeout`/`RateLimited(code)`/`ApiError(code, msg)`
2. HTTP 客户端超时设为 10s，捕获 `net::Error`
3. `daemon.rs` 中 `notify_feishu` 用 `let _ =` 调用，结果只打印 stderr，不改变 `outcome`
4. 429 限频时记录日志并返回 `FeishuRateLimited`，不重试（推断：Phase 1 不引入重试复杂性）
5. token 错误（推断：Webhook 无 token，可忽略；但错误处理结构预留以便未来迁移 Bot）

**验收**：S3 ✅（模拟网络断开，run 正常完成，stderr 有 FeishuNetworkError 记录）

### Phase 4：Watch 模式（补充 S2）

**目标**：`scan_inbox` 路径也能收到通知

1. 在 `daemon.rs::scan_inbox` 的 `record_result` 后、`write_state` 前插入通知调用
2. 携带 `Dispatch { project, note, content_hash }` 上下文，增强卡片 note 内容

**验收**：Watch 模式下 run 完成能正常收到卡片

---

## 七、验收标准（对齐 Ticket 088 四个 Story）

| Story | 验收条件 | 验证方式 |
|-------|---------|---------|
| **S1** | 配置 `webhook_url` 后，`feishu::send_card` 返回 `Ok`，飞书群收到消息 | 本地 curl + `bb_cli feishu test-push` |
| **S2** | Run 终态时飞书卡片包含：项目名（✅）、任务摘要（✅）、状态中文名（✅）、操作链接（✅，可点击） | 触发真实 run，检查群内卡片 |
| **S3** | 凭证/网络故障时 stderr 有明确错误日志，run 正常返回，Succeeded/Failed exit code 不变 | 断网测试 + token 错误注入 |
| **S4** | `bb_cli feishu test-push` 可指定 `--webhook-url` 或从配置文件读取，成功发送测试卡片 | CLI 命令手动执行 |

---

## 八、风险与回滚策略

### 风险

| # | 风险 | 级别 | 缓解 |
|---|------|------|------|
| R1 | `blocked` 状态不在 `RunOutcome`/`RunStatus` 中，需 ticket 系统侧触发，当前方案不覆盖 | ⚠️ 中 | 推断：建议开子票确认 blocked 触发时机，不在主票迭代范围内 |
| R2 | Webhook URL 泄露后被滥用发送垃圾消息 | ⚠️ 中 | 推断：生产环境使用 IP 白名单或签名校验，webhook URL 不提交到公开仓库 |
| R3 | 卡片 JSON 二次序列化易出错（WebSocket 需要 string，而非对象） | ⚠️ 中 | 内部事实：`send_card` 封装为独立函数，Phase 1 写好单元测试 |
| R4 | `web_base_url` 配置缺失时操作链接为空 | ℹ️ 低 | `FeishuConfig` 中 `web_base_url` 设为必填字段，解析失败则不发送卡片 |
| R5 | Phase 1 不实现重试，Phase 3 才加入容错 | ℹ️ 低（设计决策） | 推断：最小可运行优先，重试在 Phase 3 统一处理 |
| R6 | reqwest 引入 rustls-tls 增加编译时间 | ℹ️ 低 | 内部事实：当前 `bb_daemon` 无 HTTP 客户端，引入是必要的 |
| R7 | 多 run 并发时（如批量完成）可能触发 Webhook 5 QPS 限制 | ℹ️ 低 | 推断：当前用户量下单群场景罕见，如遇限频在 Phase 3 处理退避 |
| R8 | 99991663/99991664 错误码未找到官方文档出处 | ℹ️ 低 | 外部事实：可能属于旧版或特定接口错误码，通过实际调用 `ext` 字段定位 |

### 回滚策略

- **配置回滚**：在 TOML 中将 `enabled = false` 或注释 `[feishu]` section，即时生效，无需代码回滚
- **代码回滚**：删除 `bb_daemon/src/feishu/` 目录，移除 `Cargo.toml` 中的 reqwest 依赖，恢复 `daemon.rs` 中两处插入点
- **数据回滚**：推送不写入持久化存储，无数据层面回滚需求

---

## 九、待确认事项（建议在实现前与用户确认）

1. **`blocked` 触发方式**：`blocked` 是否在 ticket 状态变更时触发（需 ticket 系统侧回调），还是在 TaskGraph run 中出现（需确认代码路径）？推断：建议在 ticket 088 主票下开子票专项处理。
2. **`web_base_url` 默认值**：bb_web 是否有固定的默认地址（如 `http://localhost:5173`），还是需要用户显式配置？推断：建议在配置中显式指定，避免硬编码。
3. **卡片 @ 所有人**：是否需要在卡片中 @ 所有人？推断：Phase 1 不支持，未来如需可升级 Bot 方案。

---

*本 draft 事实来源标注：「内部事实」= 内部调研报告，「外部事实」= 外部调研报告，「推断」= 基于上下文的技术判断，应在实现前验证。*