# 088 飞书推送 — 实现 Draft（正式方案）

**日期**: 2026-05-27  
**状态**: Draft — 待实现  
**目标**: TaskGraph run 终态时通过飞书自定义机器人 Webhook 推送通知

---

## 技术决策

| 决策 | 结论 | 理由 |
|------|------|------|
| 接入方式 | **自定义机器人 Webhook** | 无需 token 管理、无 Node.js 依赖、一次 HTTP POST 搞定 |
| 排除方案 | ~~自建应用 Bot~~、~~lark-cli~~ | Bot 需创建应用+审核+token 续签；CLI 依赖 Node.js runtime |
| HTTP 客户端 | `reqwest`（rustls-tls） | bb_daemon 无现有 HTTP 客户端，reqwest 是 Rust 生态标准 |
| 插入位置 | `bb_daemon/src/daemon.rs::run_graph` 末尾 | `execute_run` 返回 `RunOutcome` 后立即通知 |
| 配置位置 | `config/task_graph_runner.toml` → `[feishu]` section | 复用现有三层覆盖机制 |
| 错误策略 | fire-and-forget，日志记录，不阻塞 run | S3 要求 |

---

## 方案概述

```
TaskGraph run 完成
    │
    ▼
daemon.rs::run_graph()
    │  execute_run() → RunOutcome
    │
    ▼  match outcome → Succeeded / Paused / Failed
    │
    ├─ [feishu] enabled = true ?
    │     │
    │     ▼
    │  HTTP POST → https://open.feishu.cn/open-apis/bot/v2/hook/{robot_id}
    │     body: { msg_type: "interactive", card: <Interactive Card JSON> }
    │
    └─ 无论推送成功/失败，返回原 outcome（不改变 run 结果）
```

---

## 配置

```toml
# config/task_graph_runner.toml

[feishu]
enabled = true
webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/YOUR_ROBOT_ID"
# 可选：bb_web 地址，用于构造"查看详情"链接
web_base_url = "http://localhost:5173"
```

环境变量覆盖（优先级更高）：
- `BB_FEISHU_WEBHOOK_URL`
- `BB_FEISHU_WEB_BASE_URL`

---

## 消息卡片模板

```json
{
  "msg_type": "interactive",
  "card": {
    "config": { "wide_screen_mode": true },
    "header": {
      "title": { "tag": "plain_text", "content": "✅ TaskGraph Run 完成" },
      "template": "green"
    },
    "elements": [
      {
        "tag": "div",
        "text": {
          "tag": "lark_md",
          "content": "**项目**: blackboard\n**任务**: some-graph-name\n**状态**: Succeeded\n**耗时**: 2m 30s"
        }
      },
      {
        "tag": "action",
        "actions": [
          {
            "tag": "button",
            "text": { "tag": "plain_text", "content": "查看详情" },
            "type": "primary",
            "url": "http://localhost:5173/project/blackboard/runs/run-xxx"
          }
        ]
      }
    ]
  }
}
```

Header 颜色语义：
- `green` → Succeeded
- `orange` → Paused (ready_for_review)
- `red` → Failed

---

## 要改的文件

| 文件 | 改动 | 优先级 |
|------|------|--------|
| `bb_daemon/Cargo.toml` | 新增 `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }` | P0 |
| `bb_daemon/src/feishu.rs` | 新建：FeishuConfig 解析 + send_card 函数 + 卡片 JSON 构造 | P0 |
| `bb_daemon/src/daemon.rs` | `run_graph` 末尾插入飞书通知调用 | P0 |
| `bb_daemon/src/http/task_graph/runner_config.rs` | `[feishu]` TOML section 解析 + 环境变量覆盖 | P0 |
| `.bb_template/config/task_graph_runner.toml` | 添加 `[feishu]` 配置示例（默认 disabled） | P1 |
| `bb_cli/src/commands/` | `feishu test-push` 子命令（复用 feishu.rs） | P1 |

---

## 核心实现伪代码

```rust
// bb_daemon/src/feishu.rs

pub struct FeishuConfig {
    pub enabled: bool,
    pub webhook_url: String,
    pub web_base_url: Option<String>,
}

impl FeishuConfig {
    pub fn from_toml_and_env(toml: &Table) -> Option<Self> { ... }
}

pub async fn send_notification(
    config: &FeishuConfig,
    project: &str,
    run_id: &str,
    graph_name: &str,
    outcome: &RunOutcome,
    duration: Duration,
) {
    let card = build_card(project, run_id, graph_name, outcome, duration, &config.web_base_url);
    let payload = json!({ "msg_type": "interactive", "card": card });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    match client.post(&config.webhook_url).json(&payload).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if body["code"] != 0 {
                tracing::warn!("feishu push error: {}", body);
            }
        }
        Err(e) => tracing::warn!("feishu push failed: {e}"),
    }
}
```

```rust
// daemon.rs::run_graph 末尾插入
let outcome = task_graph::execute_run(&opts).await?;

// 飞书通知（非阻塞，不改变 outcome）
if let Some(ref feishu) = feishu_config {
    let _ = feishu::send_notification(feishu, project, &run.id, &graph_name, &outcome, elapsed).await;
}

Ok(outcome)
```

---

## Webhook 限制（已确认可接受）

| 限制 | 数值 | 对 Blackboard 的影响 |
|------|------|---------------------|
| 频率 | 100次/分，5次/秒 | 单用户场景远不会触达 |
| 请求体 | 最大 20KB | 卡片内容精简，不超过 2KB |
| 交互 | 仅 open_url 跳转 | 满足"查看详情"需求 |
| 安全 | 可选关键词/IP/签名 | 本地使用暂不配置 |

---

## 验收标准

1. ✅ `[feishu] enabled = true` + 配置 webhook_url 后，run 完成时飞书群收到卡片
2. ✅ 卡片包含：项目名、任务名、状态（中文 + 颜色）、耗时、查看链接
3. ✅ 网络超时或 webhook 无效时，run 正常完成，stderr 有 warn 日志
4. ✅ `bb_cli feishu test-push` 可快速验证配置是否正确

---

## 风险

| 风险 | 缓解 |
|------|------|
| Webhook URL 泄露被滥用 | 不提交到 git；未来可加签名校验 |
| reqwest 增加编译时间 | rustls-tls 比 openssl 轻；是一次性成本 |
| 卡片 JSON 拼接错误 | 单元测试覆盖 build_card 输出 |

---

*本文档是 088 四份调研报告的综合结论，取代之前的 external/internal/v2 调研文档作为实施依据。*
