# Spec Arena Report — 088 飞书推送 Implementation Draft

**Draft**: `wiki/research/088-feishu-push-implementation-draft.md`
**Ticket**: `tickets/000088-飞书-sdk-cli-消息推送接入.json`
**Arena Ticket**: `tickets/000093-对-088-飞书推送-draft-执行-spec-arena-4-1-审计.json`
**Final Spec 目标路径**: `wiki/specs/088-feishu-push/implementation_spec.md`
**Arena Report 输出路径**: `wiki/specs/088-feishu-push/arena_report.md`
**Human Gate**: ❌ **Blocked** — `attacker.verdict=needs_rework`，4 个 P0 blockers 未解决
**Spec Arena 协议版本**: v10

---

## 1. 输入 Provenance

| 输入 | 内容摘要 |
|------|---------|
| **ResourceBundle** | draft_to_implementation_spec_arena 任务；model=minimax-cn/MiniMax-M2.7-highspeed；output_dir=wiki/specs/088-feishu-push |
| **code_facts** | `bb_cli` 无 `commands/` 目录（`exists:false`）；`daemon.rs` run_graph 返回 `Result<RunOutcome>`（line 82/279/397）；`runner_config.rs` 有 `deny_unknown_fields`（line 22/58），无 `feishu` 字段 |
| **Draft** | 伪代码含 `spawn_feishu_notification`、`FeishuConfig::from_toml_and_env`、三层配置覆盖、`run_graph` 改为 `Result<GraphRunResult>`、`join_on_exit` 语义 |
| **Ticket 000088** | S1: daemon 非阻塞飞书推送；S2: 脱敏 warning；S4: `bb_cli feishu test-push`；S5: 配置缺失明确提示 |
| **Arena 协议 000091** | 4 reviewer + 互评 + attacker + human gate 流程 |

---

## 2. 四个 Reviewer 视角与 Verdict

### R1 — daemon / TaskGraph Integration

| 字段 | 值 |
|------|---|
| **Perspective** | daemon / TaskGraph integration |
| **Verdict** | `pass_with_changes` |
| **Stars** | 4（self），3（aggregate 排名第四） |
| **Rank** | 4（aggregate） |
| **P0 Findings** | r1-001: `run_graph` 签名从 `Result<RunOutcome>` 改为 `Result<GraphRunResult>`，两处调用点（daemon.rs ~279 和 ~397）必须同步重构 |
| | r1-002: `RunnerConfig deny_unknown_fields` 导致含 `[feishu]` section 的 TOML 解析失败回退 default，静默失效 |

**贡献亮点**: 精确定位 `run_graph` 两处调用点的代码行号；明确 `join_on_exit` 通过调用位置区分（daemon.rs else 分支 vs scan_inbox）而非引入新枚举。

---

### R2 — config / CLI / Operator UX

| 字段 | 值 |
|------|---|
| **Perspective** | config / CLI / operator UX |
| **Verdict** | `pass_with_changes` |
| **Stars** | 4（self），5（aggregate 并列第一） |
| **Rank** | 2（aggregate） |
| **P0 Findings** | r2-001 + r1-002: `RunnerConfig` 需新增 `feishu: Option<FeishuTomlConfig>` 字段，绕过 `deny_unknown_fields` 阻断 |
| | **r2-002（独有 P0）**: `render_runner_config` 重写整个 TOML 文件，`patch_runner_config` 会**静默删除** `[feishu]` section |
| | r2-003: `bb_cli/src/commands/` 目录不存在，`enum Commands` 无 Feishu 变体；给出精确的 Clap subcommand 枚举扩展方案 |

**贡献亮点**: 唯一识别 `patch_runner_config` 静默删除 `[feishu]` section 的破坏性操作风险；提供最具体的 CLI 骨架实现步骤。

---

### R3 — Reliability / Security / Observability

| 字段 | 值 |
|------|---|
| **Perspective** | reliability / security / observability |
| **Verdict** | `pass_with_changes` |
| **Stars** | 4（self），5（aggregate 并列第一） |
| **Rank** | 1（aggregate，按 novelty 排序） |
| **P0 Findings** | **r3-001（独有 P0）**: `reqwest::Error::Display` 隐式包含完整 webhook URL（含 token），`tracing::warn!("feishu push failed: {err}")` 会泄露敏感信息 |
| | **r3-002（独有 P0）**: `FeishuConfig` 若使用 `#[derive(Debug)]`，日志/panic 回溯会打印完整 `webhook_url` |
| | **r3-003: `let _ = handle.join()` 忽略 panic 返回值，daemon 模式（fire-and-forget）下通知线程 panic 被完全静默吞掉 |
| | r3-005: webhook_url host 未校验，可指向任意 https 域名，存在轻量 SSRF 风险 |

**贡献亮点**: 发现两条独立的 webhook URL 泄露路径，均未被其他 reviewer 识别；`reqwest::Error::Display` 隐式泄露是绕过所有显式字段级 mitigation 的 bypass 路径。

---

### R4 — BDD Alignment / Test Matrix / Spec Completeness

| 字段 | 值 |
|------|---|
| **Perspective** | BDD alignment / test matrix / spec completeness |
| **Verdict** | `pass_with_changes` |
| **Stars** | 3（self & aggregate） |
| **Rank** | 4（aggregate） |
| **P1 Findings** | r4-001: 测试矩阵 S4 缺少 CLI `--json` 输出 shape 的具体断言 |
| | r4-002: `bb_cli` commands 目录不存在 vs `enum Commands` 并存的歧义需在 spec 中明确二选一 |
| | r4-004（被采纳 P1）: 实现顺序将 CLI test-push 置于 daemon 集成之后，但 S4/S5 验收要求 CLI 可独立测试 |

**贡献亮点**: 唯一识别实现顺序问题（CLI test-push 应与 feishu.rs 同阶段实现）；指出测试矩阵缺失 CLI --json 输出格式的具体断言。

---

## 3. 互评排序与星级

### 互评汇总星级

| Reviewer | R1 给分 | R2 给分 | R3 给分 | R4 给分 | Aggregate Stars | Aggregate Rank |
|----------|---------|---------|---------|---------|-----------------|----------------|
| **R3** | ★★★★★ | ★★★★★ | — | ★★★★★ | 5 | 1 |
| **R2** | ★★★★★ | — | ★★★★★ | ★★★★★ | 5 | 2 |
| **R1** | ★★★★☆ | ★★★★☆ | ★★★★☆ | ★★★★☆ | 4 | 3 |
| **R4** | ★★★★☆ | ★★★☆☆ | ★★★☆☆ | — | 3 | 4 |

### 各 Reviewer 的 Sorted Reviews（Top Pick）

**R3 排序 R2 为 #1**：理由是 r2-002（patch 破坏 [feishu] section）是被所有其他 review 遗漏的独特 P0；r2-003 提供最具体的 CLI 枚举扩展实现指导。R3 自评 rank 2，理由是 security 发现 novelty 最高但 actionability 略低于 R2。

**R2 排序 R2 为 #1**：self-assessment，理由是所有 finding 均有 code_facts 支撑，无 false positive。排序 R3 为 #2，理由是 security 发现有代码行为依据但部分 severity 校准有偏差（r3-011 应为 P1 而非 P2）。

**R1 排序 R1 为 #3**：self-assessment，承认 novelty 较低（与 R2/R3 重叠多），但 daemon 集成视角代码行级精度最高。

**R4 排序 R4 为 #4**：self-assessment，承认 severity 校准偏弱，部分 finding 与 R3 冲突（r4-003 vs r3-003）。

---

## 4. 互评汇总 Accept/Reject 决策

### 共识 P0（must_accept=true）

| Finding | Reviewer(s) | 描述 |
|---------|-------------|------|
| r3-001 + r3-002 | R3 | `reqwest::Error::Display` 隐式泄露 webhook URL；`FeishuConfig #[derive(Debug)]` 泄露 `webhook_url` |
| r2-002 | R2（独有） | `patch_runner_config` → `render_runner_config` 重写整个 TOML，静默删除 `[feishu]` section |
| r1-001 + r2-001 + r1-002 | R1, R2 | `run_graph` 返回类型改为 `Result<GraphRunResult>`；`RunnerConfig` 需 `feishu: Option<FeishuTomlConfig>` |
| r2-003 | R2 | `bb_cli` 缺少 Feishu subcommand 入口，需明确 enum 扩展方案 |
| r3-003 | R3 | 通知线程 panic 后 `join()` 结果被忽略，需 `catch_unwind` |

### 共识 P1（must_accept=true）

| Finding | Reviewer(s) | 描述 |
|---------|-------------|------|
| r4-004 | R4 | 实现顺序调整：CLI test-push 应与 feishu.rs 同阶段 |
| r2-004 | R2 | `connect_timeout_secs` / `read_timeout_secs` 无环境变量覆盖，需补充或显式声明为 non-goal |
| r2-005 | R2 | `InvalidWebhookUrl`（非 https）错误处理策略未定义 |
| r3-004 | R3 | `read_timeout_secs` 命名误导，`reqwest::timeout()` 是总请求超时 |

### 冲突裁决

| Conflict | 裁决 |
|----------|------|
| r3-003 vs r4-003（thread panic severity） | **采纳 R3 P0**：fire-and-forget 设计不证明静默 panic loss 合理；必须 `catch_unwind` + `tracing::error!` |
| r3-009（per-notification Client 重建） | **降级为 P2 / known limitation**：MVP 可接受，post-MVP 优化项 |
| r1-005 vs r4（spawn placement） | **降级为 P2 / documentation clarity**：draft 逻辑内部一致，无需结构性修改 |
| r3-011 vs r2（web_base_url host 白名单） | **升级为 P1**：钓鱼风险真实，需与 webhook_url 区分处理 |

### Deferred / Rejected

| Decision | Item | 理由 |
|----------|------|------|
| **open** | r3-005（webhook_url SSRF host 白名单） | 需产品确认私有化域名支持 |
| **open** | r2-006（project inference 算法） | 与 atk-006 矛盾，见下方 |
| **reject** | r3-009 重回 P1 | 性能问题，MVP non-goal |
| **defer** | r3-012（User-Agent）、r3-008（Retry-After header） | post-MVP observability |

---

## 5. Attacker 质疑（10 项）

### P0 Attack Items（必须解决）

| ID | Claim | Verdict | Recommended Resolution |
|----|-------|---------|------------------------|
| **atk-001** | `web_base_url` 被错误地与 `webhook_url` 共用 `open.feishu.cn` allowlist。`webhook_url` 是飞书接收地址，`web_base_url` 是 Blackboard 详情页入口，域名完全不同。 | **reject**（attacker 正确） | 拆分两个校验：`webhook_url` 只允许 `open.feishu.cn`；`web_base_url` 应校验为合法 Blackboard Web origin，MVP 只做规范化与脱敏日志 |
| **atk-002** | `bb_cli` 与 `bb_daemon` 的 feishu 模块 crate 归属未解决，`accepted advice` 要求 CLI 直接调用 `FeishuConfig::load + send_notification`，但未验证 Cargo 依赖图和 `lib.rs` public API | **open** | final spec 需先明确 crate 归属（共享 crate / bb_daemon public API），验证前将"direct CLI execution"标为待定 |
| **atk-003** | `join_on_exit` 超时实现被描述为 `JoinHandle::join` 语义，但 Rust `JoinHandle::join` 不支持超时；`Ok(Err(_))` 语义是对 `join()` 返回类型的误解 | **accept**（attacker 正确） | 必须改用 `mpsc channel + recv_timeout`：通知线程 `catch_unwind` 后通过 channel 返回结果；single-run 调用方 `recv_timeout(connect + request + margin)`；daemon 模式不等待 |
| **atk-004** | 飞书卡片 payload 数据外发边界未定义：run_id、project、graph_name、失败 message、详情 URL 是否允许外发，000088 BDD 未明确授权 | **open** | final spec 需补全"允许外发字段白名单"，并与 000088 ticket owner 确认授权范围 |

### P1 Attack Items

| ID | Claim | Verdict |
|----|-------|---------|
| **atk-005** | 测试矩阵未闭环"通知失败/panic/timeout/invalid config 不得改变原始 RunOutcome/exit code" | **accept** |
| **atk-006** | r2-006 project inference 被同时标记为 `must_accept:false`、`minority high-impact`、`accepted advice` 中完整算法，内部矛盾 | **accept**（需消除矛盾） |
| **atk-007** | run_id 展示格式（完整字符串 vs 截断）未被聚合解决，未列入 must-accept | **accept** |
| **atk-008** | `patch_runner_config` 保留 `[feishu]` 列出三个互斥方案但未选定 | **open**（需选择具体实现路线） |
| **atk-009** | 安全增强项（host allowlist、Retry-After、User-Agent、连接池）部分为 post-MVP，但已被推入 spec | **defer**（分层处理） |
| **atk-010** | reviewer ranking 过度奖励 novelty，可能将 post-MVP hardening 与 MVP 阻断项混在一起 | **defer**（参考，非决策因素） |

---

## 6. Human Gate 结果

```
verdict: needs_rework
```

**结论**：Human gate 虽然标记为 `accepted`，但 `attacker.verdict=needs_rework` 且存在 **4 个 P0 attack items** 和 **7 项未解决的 `must_accept_before_merge`**。

依据 Merge Safety Contract v10，**不得输出可执行 implementation spec**。

---

## 7. 最终 Spec 路径

| 文件 | 路径 | 状态 |
|------|------|------|
| **Final Implementation Spec** | `wiki/specs/088-feishu-push/implementation_spec.md` | ❌ **未输出**（被 block） |
| **Arena Report** | `wiki/specs/088-feishu-push/arena_report.md` | ✅ 本文档 |

---

## 8. 剩余 Open Questions（Merge 前必须解决）

| ID | 问题 | 责任方 |
|----|------|--------|
| **oq1** | `run_mode.is_single_run()` 在 `daemon.rs` 中不存在。如何区分 direct-run 和 daemon 模式：`!options.watch && !options.schedules` 硬编码调用位置是否足够？ | 实现者确认 |
| **oq2** | `bb_cli feishu test-push` 是 CLI 进程直接执行 HTTP POST，还是透传给 daemon HTTP endpoint？ | 架构决策 |
| **oq3** | `feishu` 模块 crate 归属：`bb_daemon` 暴露 public API、`bb_core` 子模块、还是独立 `bb_feishu` crate？ | 架构 owner |
| **oq4** | daemon 启动时 `FeishuConfig` 加载失败：应 warn + 继续运行还是 error + 退出？ | 行为决策 |
| **oq5** | `webhook_url` host allowlist 是否支持私有化飞书域名？默认行为？ | 产品确认 |
| **oq6** | `web_base_url` 允许的 origin 范围（与 `webhook_url` 不同！）。MVP 是否只做规范化不做 host 校验？ | 产品确认 |
| **oq7** | `direct-run` join timeout 的具体同步机制是否采用 `mpsc recv_timeout`？ | 实现者确认 |
| **oq8** | `run_id` 展示格式：完整字符串还是短 ID？与卡片详情 URL 使用同一规范？ | 行为决策 |
| **oq9** | 飞书 payload 允许外发字段白名单：project、graph_name、run_id、elapsed、outcome 是否允许外发？详情 URL 是否包含 run_id？ | 000088 ticket owner |
| **oq10** | `patch_runner_config` 保留 `[feishu]` 具体实现路线：三选一（read-modify-write / diff-reject / section-aware） | 实现决策 |

---

## 9. Merge Gate Summary

| Gate | 结果 |
|------|------|
| **4 Reviewer Verdict** | `pass_with_changes`（全员） |
| **Peer Rating Aggregate** | R3=R2(★5) > R1(★4) > R4(★3) |
| **Attacker Verdict** | `needs_rework` |
| **Human Gate** | ❌ **Blocked** |
| **Spec 可合并** | ❌ 否 |

### 必须解决的 7 项 `must_accept_before_merge`

1. ❌ **atk-001（P0）**: 拒绝 `web_base_url` 复用 `open.feishu.cn` allowlist，拆分 URL 校验语义
2. ❌ **atk-002（P0）**: 明确 feishu 模块 crate/API 边界，验证 Cargo 依赖图
3. ❌ **atk-003（P0）**: 将 `join_on_exit` 改写为 Rust 可执行的 `mpsc channel + recv_timeout` 方案
4. ❌ **atk-005（P1）**: 补齐负路径验收（通知失败/panic/timeout/invalid config 不得改变 RunOutcome/exit code）
5. ❌ **atk-004 + atk-007（P0/P1）**: 明确 run_id 展示格式和允许外发字段白名单，需 000088 ticket owner 确认
6. ❌ **atk-006（P1）**: 消除 r2-006 project inference 的聚合矛盾（二选一：实现 or 明确延期）
7. ❌ **atk-008（P1）**: 为 `patch_runner_config` 保留 `[feishu]` 选择一个具体实现方案

---

**Reporter**: Arena-Reporter（Spec Arena 节点）
**Generated**: 2026-05-31
**Protocol**: 000091 Spec Arena Single Draft Multi-Agent Audit