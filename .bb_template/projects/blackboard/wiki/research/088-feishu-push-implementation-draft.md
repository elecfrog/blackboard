# 088 飞书推送 — 实现 Draft（正式方案）

**日期**: 2026-05-27  
**状态**: Draft — 待实现，2026-05-31 已按 Spec Arena v22 多模型（gpt-5.5 attacker）的 P0「direct-run 通知线程生命周期」修订：direct-run/CLI 模式 join 带超时、daemon 常驻模式 fire-and-forget，两种语义分开定义，待重跑验证  
**目标**: TaskGraph run 终态时通过飞书自定义机器人 Webhook 推送通知

**MVP 非目标**:
- 不对飞书发送失败做自动 retry；失败只记录脱敏日志，不改变 TaskGraph outcome。
- 不做重复通知去重；同一 run_id 被重复执行/重试导致的重复通知留作后续增强。
- 不通知 `RunOutcome::Cancelled`；本轮只覆盖 Succeeded / Paused / Failed，但 Cancelled 必须显式 match。

---

## 技术决策

| 决策 | 结论 | 理由 |
|------|------|------|
| 接入方式 | **自定义机器人 Webhook** | 无需 token 管理、无 Node.js 依赖、一次 HTTP POST 搞定 |
| 排除方案 | ~~自建应用 Bot~~、~~lark-cli~~ | Bot 需创建应用+审核+token 续签；CLI 依赖 Node.js runtime |
| HTTP 客户端 | `reqwest`（`blocking` + `rustls-tls`） | 当前 `daemon::run` / `run_graph` 都是同步函数；MVP 不重构 daemon 为 async |
| 插入位置 | `bb_daemon/src/daemon.rs::run` 的 `match run_graph(...)` outcome 分支 | `run_graph` 只创建并执行 run，并返回 run_id/outcome/elapsed；通知调度放在 daemon run loop，避免在同步函数中 `.await` |
| 调度方式 | daemon 常驻模式：`std::thread::Builder::spawn` 后台线程 + `reqwest::blocking::Client`，fire-and-forget；direct-run/CLI 单次模式：spawn 后 **`join` 一个有界超时**（`connect_timeout + read_timeout + 小余量`） | 不依赖当前线程存在 Tokio runtime；不改变原始 `RunOutcome`；线程 spawn 失败只记录脱敏 warn。**关键（v22 gpt-5.5 P0）**：direct-run 模式 run 结束后进程立即退出，分离线程会被杀导致通知静默丢失，因此必须 join 带超时；daemon 常驻进程不会立即退出，保持 fire-and-forget |
| 通知线程生命周期 | daemon 模式 detach（不 join）；direct-run 模式 join(超时) | run_mode 由 `daemon::run` 调用点区分（single-run vs watch/daemon loop），传入 `FeishuNotificationContext.join_on_exit: bool` |
| 配置位置 | `config/task_graph_runner.toml` → `[feishu]` section | 现有 runner config 需要新增 `feishu` 字段和 env 覆盖 helper |
| 错误策略 | fire-and-forget，日志记录，不阻塞 run | S3 要求 |

---

## 方案概述

```
TaskGraph run 完成
    │
    ▼
daemon.rs::run_graph()
    │  execute_run() → RunOutcome
    │  return GraphRunResult { run_id, outcome, elapsed }
    │
    ▼
daemon.rs::run()
    │  match GraphRunResult.outcome → Succeeded / Paused / Failed / Cancelled
    │
    ├─ Cancelled ?
    │     └─ 本轮不通知，直接记录 cancelled 并返回原 outcome
    │
    ├─ [feishu] enabled = true ?
    │     │
    │     ▼
    │  std::thread::spawn
    │     HTTP POST → https://open.feishu.cn/open-apis/bot/v2/hook/{robot_id}
    │     body: { msg_type: "interactive", card: <Interactive Card JSON> }
    │
    └─ 无论推送成功/失败，返回原 outcome（不改变 run 结果）
```

---

## 配置

```toml
# config/task_graph_runner.toml

[feishu]
enabled = false
# webhook_url 建议通过 BB_FEISHU_WEBHOOK_URL 注入；不要提交真实 URL。
webhook_url = ""
# 可选：bb_web 地址，用于构造"查看详情"链接
web_base_url = "http://localhost:5173"
# 可选：连接和读取超时，单位秒；省略时使用 3 / 10
connect_timeout_secs = 3
read_timeout_secs = 10
```

启用示例：

```toml
[feishu]
enabled = true
webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/YOUR_ROBOT_ID"
web_base_url = "http://localhost:5173"
```

环境变量覆盖（优先级更高）：
- `BB_FEISHU_ENABLED`
- `BB_FEISHU_WEBHOOK_URL`
- `BB_FEISHU_WEB_BASE_URL`

配置语义：
- `.bb_template/config/task_graph_runner.toml` 必须默认 `enabled = false`。
- `enabled = false` 时不校验也不发送飞书通知。
- `enabled = true` 但 `webhook_url` 缺失或 trim 后为空时，daemon 必须记录脱敏 warn 并跳过飞书调度，不发送空 URL 请求，不改变 TaskGraph outcome。
- `web_base_url` 只用于"查看详情"按钮；非空时必须是 `http` 或 `https` origin（允许 `/`，不允许额外 path/query/fragment）。无效或带路径时记录脱敏 warn 并降级为不展示按钮，不阻塞发送主体。
- 完整 `webhook_url` 视为敏感值；日志、CLI 输出、arena report 和 ticket 里都不得打印真实 URL。
- `BB_FEISHU_*` 环境变量 trim 后为空时视为未设置，回退 TOML 值；只有非空环境变量才覆盖 TOML。

配置解析链路：
- 现状：`bb_daemon/src/http/task_graph/runner_config.rs::RunnerConfig` 使用 `#[serde(deny_unknown_fields)]`，当前没有 `feishu` 字段，也没有 `read_env_bool_or_toml` / `read_env_or_toml` helper。
- 必须新增 `FeishuTomlConfig`，并在 `RunnerConfig` 上增加 `feishu: Option<FeishuTomlConfig>`，否则 `[feishu]` section 会导致现有 runner config 解析失败并回退默认值。
- 必须新增可复用 helper：`read_env_or_toml(env_name, toml_value) -> Option<String>` 和 `read_env_bool_or_toml(env_name, toml_value) -> Option<bool>`；helper 只接受非空 env override，空字符串按未设置处理。
- `FeishuConfig::from_toml_and_env(feishu: Option<&FeishuTomlConfig>)` 只消费 `RunnerConfig` 已解析出的 `feishu` section，不独立打开或重复解析 TOML 文件。
- `FeishuConfig::load(root)` 如需保留，只能是薄封装：调用 `runner_config::read_runner_config(root)`，再把 `config.feishu.as_ref()` 传给 `from_toml_and_env`。daemon direct run、watch/inbox scan 和 CLI test-push 必须复用同一解析路径。
- `FeishuTomlConfig` 支持可选 `connect_timeout_secs` / `read_timeout_secs`；默认值分别为 3 / 10。若决定不暴露超时配置，spec 必须显式把可配置 timeout 写入非目标。

脱敏 test webhook 语义：
- “脱敏 test webhook” 指专用测试机器人 URL，其中 robot id/token 被替换为明显假的占位符，例如 `https://open.feishu.cn/open-apis/bot/v2/hook/<redacted-test-robot>`。
- 手工 e2e 验证可在本机环境变量中使用真实测试 webhook，但任何日志、ticket、wiki、arena output、git diff 和错误信息都只能展示 `<redacted>` 或明显假的 robot id。
- test-push 的成功/失败输出不得包含完整 webhook URL；最多输出 host、是否已配置、脱敏后的尾部状态。

CLI 验证：
- 命令：`bb_cli feishu test-push --project <project>`；默认 project 可从当前 workspace 推断时允许省略，否则必须明确报错。
- 默认读取 `config/task_graph_runner.toml` 与 `BB_FEISHU_*` 环境变量；不提供 `--webhook-url` 参数，避免 URL 作为命令行参数进入 shell history 与进程 `argv`。
- **Shell history 风险澄清（v21 attacker P0-1）**：仅"用 env var 替代 `--webhook-url`"并不能保证 URL 不进 history——`export BB_FEISHU_WEBHOOK_URL=...` 或 `BB_FEISHU_WEBHOOK_URL=... bb_cli ...` 这类内联命令本身会被记录到 `~/.bash_history` / `~/.zsh_history`。正确做法是：把 webhook_url 放入**不提交的配置文件**（`config/task_graph_runner.toml` 本地副本或 `.env`），由进程读取，而不是在交互式 shell 内联 export；如确需临时覆盖，应使用 `HISTCONTROL=ignorespace` + 前置空格、或从受控 secret 文件 `set -a; source .env; set +a` 注入，避免明文进入 history。该 mitigation 措辞需同步收紧到 000088 ticket 的 `webhook-url-leak` 风险。
- 成功：exit code 0，默认输出人类可读摘要；`--json` 输出 `{"status":"ok","code":0,"msg":"ok"}`。
- 失败：exit code 1，输出脱敏错误摘要；`--json` 输出 `{"status":"error","code":<feishu_code_or_http_status>,"msg":"<sanitized_message>"}`。

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
- `Cancelled` → 本轮不发送飞书通知；必须显式 match 处理，不能遗漏分支

---

## 要改的文件

| 文件 | 改动 | 优先级 |
|------|------|--------|
| `bb_daemon/Cargo.toml` | 新增 `reqwest = { version = "0.12", features = ["json", "blocking", "rustls-tls"] }` | P0 |
| `bb_daemon/src/feishu.rs` | 新建：FeishuConfig 解析 + send_card 函数 + 卡片 JSON 构造 | P0 |
| `bb_daemon/src/daemon.rs` | 将 `run_graph(...) -> Result<RunOutcome>` 改为 `Result<GraphRunResult>`，并更新 `daemon::run` 的 direct run 与 inbox scan 两个调用点；通知只在 `daemon::run` outcome 分支调度；不得 `.await` | P0 |
| `bb_daemon/src/http/task_graph/runner_config.rs` | 增加 `feishu: Option<FeishuTomlConfig>`，避免 `deny_unknown_fields` 拒绝 `[feishu]`；新增 env 覆盖 helper 或导出给 `feishu.rs` 复用 | P0 |
| `.bb_template/config/task_graph_runner.toml` | 添加 `[feishu]` 配置示例（默认 disabled） | P1 |
| `bb_cli/src/commands/` | `feishu test-push` 子命令（复用 feishu.rs）；000088 S4/S5 依赖它完成 operator 验证，因此属于 MVP 必需项 | P0 |

---

## 核心实现伪代码

```rust
// bb_daemon/src/http/task_graph/runner_config.rs

pub struct FeishuTomlConfig {
    pub enabled: Option<bool>,
    pub webhook_url: Option<String>,
    pub web_base_url: Option<String>,
    pub connect_timeout_secs: Option<u64>,
    pub read_timeout_secs: Option<u64>,
}

// bb_daemon/src/feishu.rs

pub struct FeishuConfig {
    pub enabled: bool,
    pub webhook_url: String,
    pub web_base_url: Option<String>,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
}

pub enum FeishuConfigError {
    EmptyWebhookUrl,
    InvalidWebhookUrl(String),
}

impl FeishuConfig {
    pub fn from_toml_and_env(
        feishu: Option<&FeishuTomlConfig>,
    ) -> Result<Option<Self>, FeishuConfigError> {
        let enabled = read_env_bool_or_toml(
            "BB_FEISHU_ENABLED",
            feishu.and_then(|config| config.enabled),
        )
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }

        let webhook_url = read_env_or_toml(
            "BB_FEISHU_WEBHOOK_URL",
            feishu.and_then(|config| config.webhook_url.as_deref()),
        )
        .unwrap_or_default();
        if webhook_url.trim().is_empty() {
            return Err(FeishuConfigError::EmptyWebhookUrl);
        }

        let parsed = reqwest::Url::parse(&webhook_url)
            .map_err(|_| FeishuConfigError::InvalidWebhookUrl("<redacted>".to_string()))?;
        if parsed.scheme() != "https" {
            return Err(FeishuConfigError::InvalidWebhookUrl("<redacted>".to_string()));
        }

        let web_base_url = read_env_or_toml(
            "BB_FEISHU_WEB_BASE_URL",
            feishu.and_then(|config| config.web_base_url.as_deref()),
        )
            .unwrap_or_default()
            .trim()
            .to_string();
        let web_base_url = if web_base_url.is_empty() {
            None
        } else {
            match reqwest::Url::parse(&web_base_url) {
                Ok(mut url)
                    if matches!(url.scheme(), "http" | "https")
                        && matches!(url.path(), "" | "/")
                        && url.query().is_none()
                        && url.fragment().is_none() =>
                {
                    url.set_path("");
                    Some(url.to_string().trim_end_matches('/').to_string())
                }
                _ => {
                    tracing::warn!("ignore invalid feishu web_base_url: <redacted>");
                    None
                }
            }
        };
        let connect_timeout_secs = feishu
            .and_then(|config| config.connect_timeout_secs)
            .unwrap_or(3);
        let read_timeout_secs = feishu
            .and_then(|config| config.read_timeout_secs)
            .unwrap_or(10);

        Ok(Some(Self {
            enabled,
            webhook_url,
            web_base_url,
            connect_timeout: Duration::from_secs(connect_timeout_secs),
            read_timeout: Duration::from_secs(read_timeout_secs),
        }))
    }
}

pub enum FeishuSendError {
    Network(reqwest::Error),
    ReadBody(std::io::Error),
    HttpStatus(reqwest::StatusCode),
    ParseBody(serde_json::Error),
    Business { code: i64, msg: String },
    BuildCard(String),
    ClientBuild(String),
    InvalidResponseBody(String),
    BodyTooLarge { limit: usize },
}

pub fn send_notification(
    config: &FeishuConfig,
    project: &str,
    run_id: &str,
    graph_name: &str,
    outcome: FeishuOutcome,
    duration: Duration,
) -> Result<(), FeishuSendError> {
    let card = build_card(project, run_id, graph_name, outcome, duration, &config.web_base_url)
        .map_err(|err| FeishuSendError::BuildCard(err.to_string()))?;
    let payload = json!({ "msg_type": "interactive", "card": card });

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(config.read_timeout)
        .build()
        .map_err(|err| FeishuSendError::ClientBuild(err.to_string()))?;

    let response = client
        .post(&config.webhook_url)
        .json(&payload)
        .send()
        .map_err(FeishuSendError::Network)?;

    let status = response.status();
    let mut limited_body = response.take((64 * 1024 + 1) as u64);
    let mut body_bytes = Vec::new();
    limited_body
        .read_to_end(&mut body_bytes)
        .map_err(FeishuSendError::ReadBody)?;
    if body_bytes.len() > 64 * 1024 {
        tracing::warn!(body_len = body_bytes.len(), "feishu push response body too large");
        return Err(FeishuSendError::BodyTooLarge { limit: 64 * 1024 });
    }
    let body_text = std::str::from_utf8(&body_bytes)
        .map_err(|err| FeishuSendError::InvalidResponseBody(err.to_string()))?;

    if !status.is_success() {
        tracing::warn!(
            status = %status,
            body_len = body_text.len(),
            "feishu push http status error"
        );
        return Err(FeishuSendError::HttpStatus(status));
    }

    let body: serde_json::Value =
        serde_json::from_str(&body_text).map_err(FeishuSendError::ParseBody)?;

    let code = body.get("code").and_then(|value| value.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = body.get("msg").and_then(|value| value.as_str()).unwrap_or("unknown");
        tracing::warn!(code, msg, "feishu push business error");
        return Err(FeishuSendError::Business { code, msg: msg.to_string() });
    }

    Ok(())
}
```

日志要求：
- 不记录完整 `webhook_url`，避免 URL 泄露。
- 网络错误日志只记录错误类别、HTTP status、body 长度或飞书业务 code/msg。
- 非 2xx、非 JSON body、飞书业务 `code != 0` 都必须被视为发送失败并返回 `Err`；不得 `unwrap_or_default()` 后静默当成功。

```rust
// bb_daemon/src/feishu.rs
#[derive(Debug, Clone, Copy)]
pub enum FeishuOutcome {
    Succeeded,
    Paused,
    Failed,
}

pub fn map_outcome(outcome: &RunOutcome) -> Option<FeishuOutcome> {
    match outcome {
        RunOutcome::Succeeded => Some(FeishuOutcome::Succeeded),
        RunOutcome::Paused { .. } => Some(FeishuOutcome::Paused),
        RunOutcome::Failed { .. } => Some(FeishuOutcome::Failed),
        RunOutcome::Cancelled => None, // 000088 本轮只覆盖 Succeeded / Paused / Failed
    }
}
```

```rust
// bb_daemon/src/daemon.rs
#[derive(Debug, Clone)]
struct GraphRunResult {
    run_id: String,
    outcome: RunOutcome,
    elapsed: Duration,
}

struct FeishuNotificationContext {
    project: String,
    run_id: String,
    graph_name: String,
    outcome: RunOutcome,
    elapsed: Duration,
    // v22 gpt-5.5 P0：区分进程是否会在 run 结束后立即退出。
    // daemon 常驻 loop = false（detach）；direct-run/CLI 单次 = true（join 带超时）。
    join_on_exit: bool,
    // join 超时上界，建议 = connect_timeout + read_timeout + 1s 余量。
    join_timeout: Duration,
}

// 代码级事实（2026-05-28）：
// - bb_daemon/src/daemon.rs::run_graph 当前签名是：
//   fn run_graph(..., project: &str, input: Value) -> Result<RunOutcome>
// - run_id 来自 run_graph 内部的 task_graph::create_run(...)? 返回值：run.id。
// - outcome 来自同一函数内 task_graph::execute_run(&opts)?。
// - direct run 调用点在 daemon.rs::run 的 single-run mode；watch 调用点在 scan_inbox。
// 因此 GraphRunResult 变更是代码级可行修改，不是协议推断。
// 注意：run_graph 只创建 run、执行 run 并返回 GraphRunResult；
// run_graph 内部不得调用 spawn_feishu_notification，也不得产生飞书 side effect。
// 本 draft 只使用 bb_core::task_graph::RunOutcome；不得引入旧的内部 outcome 类型名。
fn run_graph(...) -> Result<GraphRunResult> {
    let started = Instant::now();
    let run = task_graph::create_run(...)?;
    let outcome = task_graph::execute_run(&opts)?;
    let elapsed = started.elapsed();

    Ok(GraphRunResult { run_id: run.id, outcome, elapsed })
}

fn spawn_feishu_notification(ctx: FeishuNotificationContext) {
    let Some(feishu_outcome) = feishu::map_outcome(&ctx.outcome) else {
        return; // Cancelled 不通知，但已显式处理
    };

    let join_on_exit = ctx.join_on_exit;
    let join_timeout = ctx.join_timeout;
    match std::thread::Builder::new()
        .name("bb-feishu-notify".to_string())
        .spawn(move || {
            if let Err(err) = feishu::send_notification(..., feishu_outcome, ...) {
                tracing::warn!("feishu push failed: {err}");
            }
        })
    {
        Ok(handle) => {
            // v22 gpt-5.5 P0：direct-run/CLI 模式进程会在 run 结束后立即退出，
            // 分离线程会被杀导致通知静默丢失。因此 direct-run 必须 join 带超时；
            // 用一个看门狗线程实现 join 上界，避免主线程被慢/挂的 HTTP 永久阻塞。
            if join_on_exit {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = handle.join();
                    let _ = tx.send(());
                });
                if rx.recv_timeout(join_timeout).is_err() {
                    tracing::warn!("feishu push not confirmed within join timeout on direct-run exit");
                }
            }
            // daemon 常驻模式：detach（不 join），fire-and-forget。
        }
        Err(err) => {
            tracing::warn!("skip feishu push: spawn failed: {err}");
        }
    }
}

// daemon.rs::run 的 match run_graph(...) 分支：两个调用点都按同一模式处理。
// 这是唯一允许调度飞书通知的位置；通知调度不属于 run_graph 职责。
// v22 gpt-5.5 P0：两个调用点必须按 run_mode 传不同的 join_on_exit：
//   - direct-run / single-run（进程将立即退出） -> join_on_exit = true
//   - watch / daemon 常驻 loop（进程继续存活）   -> join_on_exit = false
match run_graph(...) {
    Ok(result) => {
        spawn_feishu_notification(FeishuNotificationContext {
            project: project.name.clone(),
            run_id: result.run_id.clone(),
            graph_name: graph_ref.id.clone(), // code_facts 仅确认 graph_ref.id；不用未验证的 graph.id
            outcome: result.outcome.clone(),
            elapsed: result.elapsed,
            join_on_exit: run_mode.is_single_run(), // direct-run=true，daemon loop=false
            join_timeout: feishu_cfg.connect_timeout + feishu_cfg.read_timeout
                + Duration::from_secs(1),
        });
        match result.outcome {
            RunOutcome::Succeeded => { ... }
            RunOutcome::Paused { node_id } => { ... }
            RunOutcome::Failed { node_id, message } => { ... }
            RunOutcome::Cancelled => { ... }
        }
    }
    Err(e) => { ... }
}
```

## 测试矩阵与 BDD 映射

| BDD story | 测试层级 | 覆盖点 |
|-----------|----------|--------|
| S1 终态通知 | daemon integration / unit + manual e2e | `run_graph` 返回 `GraphRunResult`；`daemon::run` outcome 分支对成功、暂停、失败调用 `spawn_feishu_notification`，同时保留原始 `RunOutcome` 分支行为；最终需用脱敏 test webhook 人工验证飞书群实际收到卡片 |
| S2 发送失败不影响 run | unit / fake HTTP | 网络错误、HTTP 非 2xx、空 body、非 JSON body、truncated JSON、`code != 0`、body > 64KB 都返回 `Err` 并只记录脱敏 warn；集成测试显式断言原始 `RunOutcome` 不变 |
| S3 Cancelled 非目标 | unit | `map_outcome(&RunOutcome::Cancelled) == None`；`spawn_feishu_notification` 不创建线程 |
| S4 CLI test-push | CLI integration | `bb_cli feishu test-push --project blackboard` 成功 exit 0，失败 exit 1，输出不含完整 webhook URL；无 `--project` 且 workspace 不可推断时 exit 1 并提示 project required |
| S5 配置禁用/缺失 | unit / config | `enabled=false` 不校验 webhook；`enabled=true` + 空 webhook 记录脱敏 warn 并跳过发送；空 env var 回退 TOML |
| S6 web_base_url | unit / config | `http://host` / `http://host/` 规范化为 origin；`http://host/path`、query、fragment、相对路径和非 http(s) scheme 都降级为不展示按钮并记录脱敏 warn |
| S7 通知线程生命周期（v22 P0） | unit / integration | direct-run/single-run 模式 `join_on_exit=true`：spawn 后 join 带超时，超时记录脱敏 warn 且不阻塞超过 `join_timeout`；daemon 常驻模式 `join_on_exit=false`：detach 不 join。断言两种模式都不改变原始 `RunOutcome`，且 direct-run 进程在 join 窗口内退出 |

实现顺序：
1. 在 `bb_daemon/src/http/task_graph/runner_config.rs` 增加 `[feishu]` 可解析字段，避免现有 `deny_unknown_fields` 拒绝新 section。
2. 在 `bb_daemon/src/feishu.rs` 实现配置解析 helper、outcome 映射、卡片构造和 blocking 发送函数。
3. 在 `bb_daemon/src/daemon.rs` 中把 `run_graph(...)` 改为返回 `GraphRunResult`，并同步更新 direct run 与 inbox scan 两个调用点。
4. 添加 `.bb_template/config/task_graph_runner.toml` 默认 disabled 示例。
5. 添加 `bb_cli feishu test-push --project <project>`，复用同一套配置解析和发送函数。该命令是 000088 S4/S5 的 MVP 验收路径，不得延后到 post-MVP。
6. 补充单测/集成测试，再手动用脱敏测试 webhook 验证。

---

## Webhook 限制（待查证，不作为实现事实）

Spec Arena v13 attacker 指出：rate limit、空 body、错误 body 等飞书协议细节没有官方文档引用前，不能写成已确认事实。

MVP 实现只做保守处理：
- HTTP 非 2xx：记录脱敏 warn，返回发送失败。
- 2xx 但 body 为空或非 JSON：记录脱敏 warn，返回发送失败。
- JSON body 中 `code != 0`：记录 `code/msg`，返回发送失败。
- 429/rate limit：不做自动 retry，作为发送失败记录；具体频率限制等到引用官方文档后再进入正式 spec。

---

## 验收标准

1. ✅ `[feishu] enabled = true` + 配置 webhook_url 后，run 完成时飞书群收到卡片
2. ✅ 卡片包含：项目名、任务名、状态（中文 + 颜色）、耗时、查看链接
3. ✅ 网络超时或 webhook 无效时，run 正常完成，stderr 有 warn 日志
4. ✅ `bb_cli feishu test-push` 可快速验证配置是否正确
5. ✅ `RunOutcome::Cancelled` 被显式 match，不发送飞书通知，不 panic，不影响 daemon 后续处理

---

## 风险

| 风险 | 缓解 |
|------|------|
| Webhook URL 泄露被滥用 | 不提交到 git；未来可加签名校验 |
| reqwest 增加编译时间 | rustls-tls 比 openssl 轻；是一次性成本 |
| 卡片 JSON 拼接错误 | 单元测试覆盖 build_card 输出 |
| 飞书协议细节被误写成事实 | 未引用官方文档前只作为防御性假设或待查证风险 |

---

## Spec Arena v21 attacker 复审：未采纳 / 澄清项

本轮 arena（run-20260530-171557-e6e1cac4）attacker verdict=`needs_rework`，3×P0。除上文已收紧的 shell history 措辞（P0-1）外，另两项处理如下，本 draft 据此 hold-for-revision 后修订：

- **不采纳 metrics counter / Error variant 建议（P0-2）**：reviewer R3 建议新增 metrics counter、增强 Error variant，依赖 `bb_core::metrics` 基础设施。但 code_facts 中 `bb_core` 仅出现在 `task_graph::create_run`（daemon.rs:104）与 `task_graph::execute_run`（daemon.rs:157），**没有 `bb_core::metrics` 的任何实证**。按 000091 协议（claim 类型/函数存在前必须有 grep/read 实证），该建议为 inference/hypothesis，**本轮 MVP 不采纳，标记为 deferred**：飞书通知失败只走 `tracing::warn!` 脱敏日志（已是 draft 既定行为），不引入 metrics 依赖；若未来确需 metrics，另立 ticket 并先验证 `bb_core::metrics` 是否存在。
- **TLS 兼容性按假设处理，不写成事实（P0-3）**：R3-001 的 TLS P0 是外部依赖假设。draft 已固定 `reqwest` 使用 `rustls-tls`（见"技术决策"表），不额外引入未经验证的 TLS 兼容性 claim；任何 TLS 行为细节在引用官方/实证前只作为风险，不作为实现事实。
- **`GraphRunResult` 字段类型明确（非阻塞 P0 澄清）**：`run_id: String`、`outcome: bb_core::task_graph::RunOutcome`、`elapsed: std::time::Duration`（见"核心实现伪代码"中 `struct GraphRunResult`）。卡片渲染耗时时由 `build_card` 内部格式化 `Duration`，不在结构体层面改成 ms 整数。

---

*本文档是 088 四份调研报告的综合结论，取代之前的 external/internal/v2 调研文档作为实施依据。*
