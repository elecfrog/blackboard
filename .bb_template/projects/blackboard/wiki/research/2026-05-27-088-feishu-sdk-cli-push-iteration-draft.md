# 088 飞书推送 — 精简迭代 Draft

**需求**：Graph run 跑完（成功/失败）→ 给我发一条飞书消息通知  
**日期**：2026-05-27

---

## 1. 方案选型

| 方案 | 复杂度 | 结论 |
|------|--------|------|
| 飞书自定义机器人 Webhook | 极低：一个 POST 请求，无需 token 管理 | ✅ **选这个** |
| 飞书自建应用 Bot | 高：需要 app_id/secret、token 续签、应用发布 | ❌ 过度设计 |

**自定义机器人 Webhook** = 在飞书群里加一个机器人 → 拿到 webhook URL → POST JSON 即可发消息。无需认证、无需 token、无需应用发布。

---

## 2. 改动点（仅 2 个文件）

### 2.1 `bb_daemon/Cargo.toml` — 加 reqwest

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

### 2.2 `bb_daemon/src/http/task_graph/mod.rs` — spawn_task_graph_run 加通知

在 `spawn_task_graph_run` 函数中，run 完成后调用飞书 webhook：

```rust
pub fn spawn_task_graph_run(root: PathBuf, project: String, run_id: String) {
    let opts = build_runner_opts(&root, project.clone(), run_id.clone(), None);
    tokio::task::spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            task_graph::execute_run(&opts)
        }));

        // 确定最终状态
        let (status_text, is_success) = match &result {
            Ok(Ok(outcome)) => {
                eprintln!("bb task-graph run completed: {outcome:?}");
                match outcome {
                    task_graph::RunOutcome::Succeeded => ("✅ 成功", true),
                    task_graph::RunOutcome::Paused { node_id } => {
                        // paused 不发通知（等人工操作）
                        (&*format!("⏸️ 暂停于 {node_id}"), false)
                    }
                    task_graph::RunOutcome::Failed { node_id, message } => {
                        (&*format!("❌ 失败: {node_id} — {message}"), false)
                    }
                    task_graph::RunOutcome::Cancelled => ("🚫 已取消", false),
                }
            }
            Ok(Err(e)) => {
                eprintln!("bb task-graph run error: {e}");
                let _ = task_graph::update_run_status(...);
                ("❌ 运行错误", false)
            }
            Err(_) => {
                eprintln!("bb task-graph run panicked");
                let _ = task_graph::update_run_status(...);
                ("💥 Panic", false)
            }
        };

        // >>> 飞书通知 <<<
        send_feishu_notification(&project, &opts.run_id, status_text, is_success);

        let _ = dispatch_queued_task_graph_runs(&opts.workspace_root, &opts.project);
    });
}
```

### 2.3 飞书通知函数（新增，同文件底部）

```rust
/// 发送飞书自定义机器人通知
fn send_feishu_notification(project: &str, run_id: &str, status: &str, _is_success: bool) {
    // 从环境变量读取 webhook URL，未配置则跳过
    let webhook_url = match std::env::var("BB_FEISHU_WEBHOOK_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => return, // 未配置，静默跳过
    };

    let body = serde_json::json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": format!("TaskGraph Run {status}") },
                "template": if _is_success { "green" } else { "red" }
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": format!(
                            "**项目**: {project}\n**Run ID**: `{run_id}`\n**状态**: {status}"
                        )
                    }
                }
            ]
        }
    });

    // 同步发送（已在 spawn_blocking 线程中，不阻塞 tokio runtime）
    let _ = reqwest::blocking::Client::new()
        .post(&webhook_url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| eprintln!("feishu notify failed: {e}"));
}
```

---

## 3. 配置方式

只需设置一个环境变量：

```bash
export BB_FEISHU_WEBHOOK_URL="https://open.feishu.cn/open-apis/bot/v2/hook/xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
```

或者写在 `.env` / systemd service / Windows 环境变量中。

**获取 webhook URL 的步骤**：
1. 飞书群 → 设置 → 群机器人 → 添加机器人 → 自定义机器人
2. 复制 webhook 地址
3. （可选）开启签名校验增加安全性

---

## 4. 行为定义

| 场景 | 行为 |
|------|------|
| Run 成功 | 发绿色卡片：项目名 + run_id + ✅ 成功 |
| Run 失败 | 发红色卡片：项目名 + run_id + ❌ 失败原因 |
| Run Panic | 发红色卡片：项目名 + run_id + 💥 Panic |
| Run 暂停 | 不发通知（等人工操作） |
| Run 取消 | 不发通知 |
| 未配置 webhook | 静默跳过，不影响任何功能 |
| 网络超时 | 10s 超时，失败只 eprintln，不影响 run |

---

## 5. 验收标准

- [ ] `BB_FEISHU_WEBHOOK_URL` 配置后，graph run 成功/失败时飞书群收到卡片
- [ ] 未配置时无任何副作用
- [ ] 通知失败不影响 graph run 本身的状态和后续调度
- [ ] 卡片包含项目名、run_id、状态

---

## 6. 后续可选扩展（不在本次范围）

| 扩展 | 触发条件 |
|------|---------|
| 卡片加操作链接（打开 web UI） | 前端部署后 |
| 支持多个 webhook（不同群） | 多项目需要时 |
| 支持签名校验 | 安全需求时 |
| 升级为自建应用 Bot（支持 @回复） | 需要交互式回调时 |
| 配置文件化（toml/json） | 环境变量不够用时 |

---

**总结**：改动量 = 1 个依赖 + 1 个函数 + 几行胶水代码。零配置开销，环境变量控制开关。