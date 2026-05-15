+++
id = "000059"
lane = "bbt"
title = "TaskGraph Trigger：定时任务与 Schedule UI"
created_at = "2026-05-14"
updated_at = "2026-05-15"
status = "review"
area = "TaskGraph"
assignee = "codex"
depends_on = "000056"
kind = "trigger-subtrack"
parent = "000056"
requested_by = "user"
scope = "scheduled-trigger"
+++

# 当前进展

- 2026-05-14：从 Trigger System 拆出定时触发与 Schedule UI 子单。

- 2026-05-15：收窄范围：059 只做 Stateless Schedule 定时触发实现，不承担 TaskGraph 引擎重构。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-stateless-schedule-implementation.md`。

- 2026-05-15：新增 TaskSchedule/TaskScheduleState runtime 存储，支持 interval/daily/weekly/cron、Asia/Shanghai 时区、skip 并发、run_once misfire、幂等 claim
- 2026-05-15：新增 TaskGraph schedule HTTP API（list/create/patch/delete/run-now），抽取 shared run creation helper
- 2026-05-15：HTTP sidecar 启动后台 schedule dispatcher；bb daemon 增加 --schedules headless dispatcher 模式
- 2026-05-15：TaskGraph 页面新增 Schedule 区，支持 presets 创建、启用/暂停、删除、run now、next/last/status/last run 跳转

# 记录

- 目标：支持 interval、daily、weekly 和自定义 cron 形式的周期触发，最终统一创建 TaskIntent/RunRequest。
- 第一阶段建议：以持久化 schedule 配置 + dispatcher 轮询 due jobs 为主，UI 提供常用 presets，底层预留 cron parser。
- 关键策略：enabled、timezone、next_run_at、last_run_at、concurrency_policy、misfire_policy、failure/retry 状态必须可见。
- 边界：Schedule 只决定何时触发和带什么上下文，不直接执行 TaskGraph；实际运行交给 Compile/Execution。

- 产品决策：当前定时任务 scope 只覆盖 Stateless Schedule。每次到点都创建一次新的 TriggerEvent/TaskIntent/TaskRun，不复用长期 thread/context。
- 边界：059 专注 schedule 配置、到期扫描、幂等触发、运行记录和 UI；Task engine 的 durable/stateful/resume 语义重构派发到其他节点。
- 实现目标：定时触发只调用现有 TaskGraph 创建/执行入口，不改变节点执行、run state、checkpoint 或 runtime session 机制。

- 验证：cargo test -p bb_core -p bb_cli：通过。
- 验证：cargo check -p bb_cli：通过。
- 验证：npm run build --prefix bb_web：通过。
- 验证：Browser smoke on http://localhost:8060/#/projects/devkit/task-graphs：Schedule 区加载正常；创建并显示 Codex smoke schedule 后已删除清理。

- 来源：2026-05-15-codex-stateless-schedule-implementation.md
- 代码位置：bb_backend/crates/bb_core/src/task_graph/schedules.rs, bb_cli/src/http/task_graph/schedules.rs, bb_web/src/components/task-graph/TaskGraphSchedulePanel.vue

# 下一步

- 定义 ScheduleTrigger schema 与存储位置。
- 确定第一阶段 presets：每 15 分钟、每小时、每天 09:00、每周一 09:00、自定义 cron。
- 讨论 dispatcher 是复用现有 daemon loop，还是引入轻量 scheduler/parser。

- 按 Stateless Schedule 重新定义 MVP schema：schedule_id、project、graph_ref/template、input、enabled、timezone、next_run_at、last_run_at、last_run_id、last_status、idempotency_key。
- 优先设计 dispatcher：扫描 due schedules、用 schedule_id + planned_fire_at 去重、创建一次新 run、更新 last/next 状态。
- UI 先提供 presets 与状态查看：启用/暂停、周期、下次运行、上次运行、最近 run 链接。

- 后续可在 058/通用 Trigger System 里抽象 Hook/outbox；059 当前只落 Stateless Schedule，不改 TaskGraph engine/checkpoint/resume。
