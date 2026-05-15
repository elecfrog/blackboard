# 066 TaskGraph E2E smoke 测试实现交接

时间: 2026-05-15T05:06:48Z
来源: codex
项目: blackboard

## 做了什么

- 新增 bb_core 级 E2E smoke：构造 Start -> Explorer -> Implementer -> Verifier -> Reviewer -> Handoff -> HumanGate -> End 的 SWE 流程，显式走 upgrade_graph/validate_graph/create_run/execute_run/resume_run。
- 核心 smoke 断言业务 role registry：Explorer/Implementer/Verifier/Reviewer/Handoff 均能映射 NodeSpec、runtime binding、artifact outputs。
- 核心 smoke 断言 superstep checkpoint/event-log：dry-run 执行到 HumanGate paused，resume 后 succeeded，并出现 run_paused/run_completed 事件。
- 核心 smoke 断言 061 channel/artifact contract：findings、diff、test_result、review_comments、handoff_summary、acceptance_gate barrier 的类型和版本语义。
- 新增 bb_cli HTTP E2E smoke：通过 REST 创建 project graph/run，测试中用 dry-run interpreter 执行，再通过 REST 读取 run detail/checkpoints/event-log，并通过 REST resume gate 到 succeeded。
- 前端浏览器冒烟确认 Task Graph Editor palette 显示 Explorer / Implementer / Verifier / Reviewer / Handoff / Local Shell；为避免污染当前图，没有保存新增节点。

## 验证了什么

- cargo test -p bb_core e2e_smoke_business_roles_supersteps_channels_and_resume --lib：通过。
- cargo test -p bb_cli tg_e2e_smoke_exposes_supersteps_event_log_and_resume：通过。
- cargo test -p bb_core task_graph --lib：通过，129 passed。
- cargo test -p bb_cli：通过，80 passed。
- cargo check -p bb_cli：通过。
- cargo fmt --all --check：通过。
- npm run build --prefix bb_web：通过，仅有既有 chunk size warning。
- in-app browser smoke：通过，palette role 节点可见。

## 下一步

- 后续可以把 HTTP dry_run 语义改成可选同步 dry-run execution，减少测试里直接调用 interpreter 的桥接步骤。
- 补真正的前端自动化测试框架后，再把 palette 创建/保存/刷新/Run UI 查看从浏览器手测升级为可重复 E2E。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs
- bb_backend/crates/bb_cli/src/http/tests/mod.rs
