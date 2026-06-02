# 088 飞书 Webhook 终态通知 — Implementation Plan（执行标准）

**状态**: 执行标准 — 由 Spec Arena v22 多模型（claude/glm/kimi/hy3 reviewer + gpt-5.5 attacker）审计后，由 CodeBuddy 拍定全部 P0 决策并固化为可执行 plan。本文件取代 draft 成为唯一实现依据。
**关联**: ticket 000088（BDD 行为边界）、000093（arena dogfood）、000091（arena 协议）
**日期**: 2026-05-31

---

## 0. 已拍定的关键决策（解决 arena P0）

| P0 | 决策 |
|----|------|
| atk-002 crate 边界 | feishu 纯逻辑（config 解析 / 卡片构造 / blocking 发送 / outcome 映射）放入 **`bb_core::feishu`**；bb_daemon、bb_cli 均已依赖 bb_core，无需 bb_cli→bb_daemon 重依赖。HTTP 依赖 `reqwest`(blocking+rustls-tls) 加到 bb_core。 |
| atk-004 payload 数据外发白名单 | 卡片**仅外发** `{project, graph_id, run_id, 终态标签, elapsed}`，可选 detail 链接（仅当 `web_base_url` 配置时）。**不外发** `RunOutcome::Failed.message` 原文、node_id 等内部细节（失败仅发"失败"终态标签）。 |
| atk-001 web_base_url ≠ webhook_url | 两者校验**完全独立**：`webhook_url` 必须 https；`web_base_url` 必须 http/https 且 path∈{"","/"}、无 query/fragment，否则降级为不展示按钮。绝不共用同一 allowlist。 |
| atk-003 join 语义 | direct-run 用 **channel `recv_timeout`** 看门狗实现有界等待（不是 `JoinHandle::join` 超时，标准库无此 API），daemon 常驻 detach。 |
| 线程生命周期 | direct-run/single-run 模式 `join_on_exit=true`（recv_timeout 等待后台发送）；watch/daemon loop `join_on_exit=false`（detach）。 |
| atk-008 patch_runner_config | 本轮非目标：`patch_runner_config`(连接器 UI 写回) 暂不处理 `[feishu]` section；`[feishu]` 由人手工编辑 toml。已在非目标声明，避免 render 丢段（render 不动 feishu，读取时 `[feishu]` 仍被 `RunnerConfig` 解析）。 |

---

## 1. 行为边界（来自 000088 BDD，不重定义）

- S1：enabled + 有效 webhook，run 终态 Succeeded/Paused/Failed → 非阻塞发一张脱敏卡片，不改 RunOutcome。
- S2：webhook 无效/不可达/超时/非成功 code → 仅脱敏 warn，不 panic、不阻塞、不改 run 状态。
- S3：`RunOutcome::Cancelled` → 显式 match 跳过，不发送。
- S4：`bb feishu test-push --project <p>` → 成功 exit 0 / 失败 exit 1，输出不含完整 URL。
- S5：缺配置或 enabled=false → 不发送；test-push 给明确未启用提示。

## 2. 改动清单

| 文件 | 改动 | 优先级 |
|------|------|--------|
| `bb_core/Cargo.toml` | 加 `reqwest = { version="0.12", default-features=false, features=["blocking","json","rustls-tls"] }` | P0 |
| `bb_core/src/feishu.rs` | 新建：FeishuTomlConfig / FeishuConfig / FeishuConfigError / FeishuSendError / FeishuTerminal / FeishuNotification / from_toml_and_env / build_card / send_notification | P0 |
| `bb_core/src/lib.rs` | `pub mod feishu;` | P0 |
| `bb_daemon/src/http/task_graph/runner_config.rs` | `RunnerConfig` 加 `feishu: Option<bb_core::feishu::FeishuTomlConfig>`（deny_unknown_fields 必须显式加）；导出 `read_runner_feishu(root)` | P0 |
| `bb_daemon/src/daemon.rs` | `run_graph` 改返回 `GraphRunResult{run_id,outcome,elapsed}`；2 个调用点更新；新增 `spawn_feishu_notification`（按 run_mode 传 join_on_exit） | P0 |
| `bb_cli/src/main.rs` | 新增 `feishu test-push --project [--json]` 子命令，复用 `bb_core::feishu` | P0 |
| `.bb_template/config/task_graph_runner.toml` | 追加 `[feishu]`（默认 enabled=false） | P1 |

## 3. bb_core::feishu 公共 API（契约）

```rust
pub struct FeishuTomlConfig { enabled, webhook_url, web_base_url, connect_timeout_secs, read_timeout_secs }  // 全 Option，Deserialize
pub struct FeishuConfig { enabled: bool, webhook_url: String, web_base_url: Option<String>, connect_timeout: Duration, read_timeout: Duration }
pub enum FeishuConfigError { EmptyWebhookUrl, InvalidWebhookUrl }            // 不含原始 URL
pub enum FeishuSendError { Network, ReadBody, HttpStatus(u16), ParseBody, Business{code,msg}, BuildClient, BodyTooLarge }
pub enum FeishuTerminal { Succeeded, Paused, Failed }                        // 仅终态标签，无 message/node_id
pub struct FeishuNotification { project, graph_id, run_id, terminal: FeishuTerminal, elapsed: Duration, detail_path: Option<String> }  // 白名单字段

impl FeishuConfig {
    pub fn from_toml_and_env(cfg: Option<&FeishuTomlConfig>) -> Result<Option<Self>, FeishuConfigError>;  // enabled=false→Ok(None)
}
pub fn build_card(cfg:&FeishuConfig, n:&FeishuNotification) -> serde_json::Value;   // 只用白名单字段
pub fn send_notification(cfg:&FeishuConfig, n:&FeishuNotification) -> Result<(), FeishuSendError>;
```

- env 覆盖：`BB_FEISHU_ENABLED` / `BB_FEISHU_WEBHOOK_URL` / `BB_FEISHU_WEB_BASE_URL`，空串视为未设置回退 toml。
- 卡片 header 颜色：Succeeded=green / Paused=orange / Failed=red。
- 发送失败定义：非 2xx、空/非 JSON body、`code!=0`、body>64KB 都返回 `Err`，调用方只脱敏 warn。

## 4. daemon 调度（bb_daemon）

```rust
struct GraphRunResult { run_id: String, outcome: RunOutcome, elapsed: Duration }

fn run_graph(...) -> Result<GraphRunResult> {
    // ...existing validate + create_run...
    let started = Instant::now();
    let outcome = task_graph::execute_run(&opts)?;
    Ok(GraphRunResult { run_id: run.id, outcome, elapsed: started.elapsed() })
}

fn spawn_feishu_notification(root, project, graph_id, result: &GraphRunResult, join_on_exit: bool) {
    let terminal = match &result.outcome {
        RunOutcome::Succeeded => FeishuTerminal::Succeeded,
        RunOutcome::Paused{..} => FeishuTerminal::Paused,
        RunOutcome::Failed{..} => FeishuTerminal::Failed,   // 不取 message
        RunOutcome::Cancelled => return,                    // S3 显式跳过
    };
    let Some(cfg) = bb_core::feishu::FeishuConfig::from_toml_and_env(
        read_runner_feishu(root).as_ref()).unwrap_or_else(|e|{warn;None}) else { return };
    let n = FeishuNotification{ project, graph_id, run_id: result.run_id, terminal,
        elapsed: result.elapsed, detail_path: cfg.web_base_url.map(|b| format!("{b}/project/{project}/runs/{run_id}")) };
    match thread::Builder::new().name("bb-feishu-notify").spawn(move || {
        if let Err(e)=bb_core::feishu::send_notification(&cfg,&n){ warn!("feishu push failed: {e}"); }
    }) {
        Ok(handle) if join_on_exit => {
            let (tx,rx)=mpsc::channel();
            thread::spawn(move ||{ let _=handle.join(); let _=tx.send(()); });
            if rx.recv_timeout(cfg_join_timeout).is_err(){ warn!("feishu push not confirmed within join timeout"); }
        }
        Ok(_) => {}                       // daemon 模式 detach
        Err(e)=> warn!("skip feishu push: spawn failed: {e}"),
    }
}
```

- 调用点：非 watch single-run（run() 直接迭代 project）→ `join_on_exit=true`；scan_inbox（watch loop）→ `false`。
- `join_timeout = connect_timeout + read_timeout + 1s`。

## 5. 测试矩阵

| story | 层级 | 覆盖 |
|-------|------|------|
| S1/payload | bb_core unit | `build_card` 只含 project/graph_id/run_id/终态/elapsed(+可选链接)；断言不含 message/node_id |
| S2 | bb_core unit (fake) | 非2xx/空body/非JSON/code!=0/body>64KB → Err |
| S3 | bb_daemon unit | Cancelled → spawn_feishu_notification 不建线程（terminal map 返回 return 路径） |
| S5/config | bb_core unit | enabled=false→Ok(None)；enabled=true+空webhook→Err(EmptyWebhookUrl)；空 env 回退 toml |
| web_base_url | bb_core unit | http(s)+path∈{"","/"} 规范化；带 path/query/fragment/非http → None；与 webhook 校验互不影响 |
| join | bb_daemon unit | join_on_exit=true 时 recv_timeout 有界返回；不改变流程；RunOutcome 不变 |

## 6. 验收 / 验证命令

- `cargo fmt --manifest-path bb_backend/Cargo.toml --all`
- `cargo build --manifest-path bb_backend/Cargo.toml`
- `cargo test --manifest-path bb_backend/Cargo.toml -p bb_core feishu`
- `bb feishu test-push --project blackboard`（手工，脱敏 test webhook）

## 7. 非目标

- 不 retry、不去重、不通知 Cancelled、不暴露可配置 timeout 之外的项；`patch_runner_config` 暂不写回 `[feishu]`（手工编辑）。
