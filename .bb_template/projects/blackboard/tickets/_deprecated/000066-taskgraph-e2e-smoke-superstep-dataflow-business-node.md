+++
id = "000066"
lane = "bbt"
title = "TaskGraph E2E Smoke：Superstep + Dataflow + Business Node 全链路验收"
created_at = "2026-05-15"
updated_at = "2026-05-16"
status = "review"
area = "TaskGraph"
assignee = "codex"
attachments = "[{\"kind\":\"wiki\",\"target\":\"proposal/taskgraph-superstep-agent-orchestration.md\",\"label\":\"Superstep TaskGraph Proposal\"},{\"kind\":\"ticket\",\"target\":\"000060\",\"label\":\"Superstep Execution Kernel\"},{\"kind\":\"ticket\",\"target\":\"000061\",\"label\":\"Dataflow Channels & Artifact Contract\"},{\"kind\":\"ticket\",\"target\":\"000062\",\"label\":\"Business Node Taxonomy\"}]"
depends_on = "000063"
kind = "e2e-validation"
parent = "000055"
proposal = "wiki/proposal/taskgraph-superstep-agent-orchestration.md"
related = "000064"
requested_by = "user"
scope = "taskgraph-e2e-smoke"
+++

# 当前进展

- 2026-05-15：从 060/061/062 review 后拆出独立 E2E 验收票。目标是验证 TaskGraph 不是只有单测和 UI 冒烟，而是能从创建图、运行、checkpoint/event-log、artifact/channel、业务节点 role 到用户验收形成完整闭环。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-e2e-smoke-ticket-creation.md`。

- codex 开始执行：开始实现 E2E smoke 测试：优先落 stub/dry-run runtime fixture、superstep checkpoint/event-log 断言、channel/artifact contract 断言和前端 palette/run UI 验证。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-066-taskgraph-e2e-smoke-tests.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-e2e-smoke-tests.md`。

# 记录

- 背景：060 已实现 superstep checkpoint/event-log，061 已实现 channel/artifact contract，062 已实现 business node taxonomy 与 palette role。当前缺口是缺少端到端 fixture 与自动化验收，无法证明三层组合后能支撑用户最初设想的“说几句话然后 Agent 协作，回来验收”。

- 验证：bb_create_ticket：通过，生成 tickets/000066-taskgraph-e2e-smoke-superstep-dataflow-business-node.md。
- 验证：Blackboard maintenance consistency：passed；export：completed；embedding：external_embedding_pending/not_implemented。

- 验证：`cargo test -p bb_core e2e_smoke_business_roles_supersteps_channels_and_resume --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::interpreter_tests --manifest-path bb_backend/Cargo.toml`：通过，30 passed。

- 验证：cargo test -p bb_core e2e_smoke_business_roles_supersteps_channels_and_resume --lib：通过。
- 验证：cargo test -p bb_cli tg_e2e_smoke_exposes_supersteps_event_log_and_resume：通过。
- 验证：cargo test -p bb_core task_graph --lib：通过，129 passed。
- 验证：cargo test -p bb_cli：通过，80 passed。
- 验证：cargo check -p bb_cli：通过。
- 验证：cargo fmt --all --check：通过。
- 验证：npm run build --prefix bb_web：通过，仅有既有 chunk size warning。
- 验证：in-app browser smoke：通过，palette role 节点可见。

- 来源：inbox/2026-05-15-codex-taskgraph-e2e-smoke-tests.md

- 来源：inbox/2026-05-15-codex-066-taskgraph-e2e-smoke-tests.md

# 下一步

- 设计一个固定 SWE smoke graph fixture：Start -> Explorer -> Implementer -> Verifier -> Reviewer -> Handoff -> HumanGate -> End。
- 先用 stub/dry-run runtime 跑通，不依赖真实模型和外部命令。
- 补 HTTP/API 集成测试：create run、read run detail、read checkpoints、read event-log、pause/resume。
- 补前端 E2E：从 palette 创建业务角色节点、保存 graph、刷新后 role 不丢、打开 run UI 能看到 superstep/checkpoint/node status。
- 补 artifact/channel 验收：findings、diff、test_result、review_comments、handoff_summary 至少在 fixture 中有稳定断言。
- 补一次真实 runtime smoke 的可选手动脚本：受控小任务接 OpenCode/Codex session，输出验收摘要。

- 开始 #000066 时优先落 stub fixture 和 HTTP 集成测试，再补前端 E2E，最后做真实 runtime smoke 的手动脚本。

- 继续补 HTTP/API 集成测试：create run、read run detail、read checkpoints、read event-log、pause/resume。
- 继续补前端 E2E：palette 创建业务角色节点、保存/刷新 role 不丢、run UI 展示 superstep/checkpoint/node status。

- 后续可以把 HTTP dry_run 语义改成可选同步 dry-run execution，减少测试里直接调用 interpreter 的桥接步骤。
- 补真正的前端自动化测试框架后，再把 palette 创建/保存/刷新/Run UI 查看从浏览器手测升级为可重复 E2E。
