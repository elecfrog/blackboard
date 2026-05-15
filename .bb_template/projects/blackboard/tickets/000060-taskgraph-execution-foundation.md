+++
id = "000060"
lane = "bbt"
title = "TaskGraph 060：Graph Compile 与 Superstep Execution Kernel"
created_at = "2026-05-14"
updated_at = "2026-05-15"
status = "review"
area = "TaskGraph"
assignee = "codex"
attachments = "[{\"kind\":\"wiki\",\"target\":\"proposal/taskgraph-superstep-agent-orchestration.md\",\"label\":\"Superstep TaskGraph Proposal\"}]"
depends_on = "000055"
explicitly_excludes = "business-agent-role-palette,prompt-source,agent-profile-design"
kind = "langgraph-distillation-execution-kernel"
layer = "execution-core"
parent = "000055"
proposal = "wiki/proposal/taskgraph-superstep-agent-orchestration.md"
requested_by = "user"
rewrite_version = "2026-05-15-corrected-langgraph-boundary"
scope = "compile-superstep-scheduler-checkpoint-event-log"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出 Execution 基建子单。
- 2026-05-15：基于 LangGraphJS `modules/pregel` 与 `modules/checkpoint*` 蒸馏重写边界：本 ticket 聚焦 TaskGraph 的 superstep-based 执行内核，不再泛泛描述 TaskRun/NodeRun 状态机。

- codex 开始执行：开始实现 superstep-based execution kernel；先落 checkpoint/event-log/run cursor，再继续推进 061/062。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-060-superstep-execution-kernel.md`。

- 2026-05-15：完成 superstep execution kernel 阶段工作——新增 SuperstepCheckpoint/RunEvent/PendingWrite/current_superstep/last_checkpoint_id，实现 coordinator superstep checkpoint+event-log，新增 run event-log 与 checkpoint 列表 HTTP API，RunPanel 展示 Superstep 和 last checkpoint。验证：cargo fmt/check/test 通过，npm run build 通过。

- codex 开始执行：开始实现 060：Graph Compile 与 Superstep Execution Kernel；范围限定为 compile/scheduler/barrier/checkpoint/event-log/resume，不处理业务 Agent role/prompt/palette。

- 2026-05-15：源码校准后继续实现 060。直接读取 `C:\Dev\langgraphjs\libs\langgraph-core\src\graph\graph.ts`、`graph/state.ts`、`pregel/index.ts`、`pregel/read.ts`、`pregel/write.ts`、`pregel/algo.ts`、`pregel/loop.ts`，确认 LangGraph compile 的核心是 lowering 到 Pregel-style `processes/channels/triggers/writers`，不是简单 adjacency map。
- 新增 `bb_backend/crates/bb_core/src/task_graph/compiler.rs`：将 raw TaskGraph 编译为 `CompiledGraph`，包含 `CompiledProcess`、`CompiledChannel`、`CompiledWriter`、`trigger_to_nodes`、reserved `__start__/__end__` channel、branch channel、join barrier channel、node output stream channel。保留 `nodes/exec_outgoing/exec_incoming/data_edges/join_nodes` 以兼容现有 coordinator。
- `create_run` 现在在冻结 `graph.snapshot.json` 的同时写入 `graph.compiled.json`，让每个 run 有可审计的编译产物。`compile_graph_for_execution` 会先执行 `upgrade_graph`，兼容旧 graph fixture/旧保存图。
- Coordinator 接入 `CompiledGraph` 的 entrypoint 与 join_nodes；新增 `SuperstepPlan`，每轮记录 cursor_before、ready_nodes、waiting_nodes。节点输出在 barrier 前收集为 `PendingWrite`，并写入 superstep checkpoint。
- Run event log 扩展为更可审计的事件序列：`run_started`、`superstep_started`、`node_started`、`node_finished`、`writes_committed`、`checkpoint_saved`、以及原有 run_completed/run_paused/run_failed 等终态事件。
- 新增/更新测试：compiler 单测验证 Pregel-style IR；run state 测试确认 `graph.compiled.json` 落盘；interpreter 测试验证并行 superstep 的 pending writes 会进入 checkpoint，event log 包含 superstep/node/checkpoint/write 事件。

- codex 开始执行：按用户新定义进入 060：先对照 LangGraphJS 源码和已蒸馏文档写 mapping，明确已做/未做/待补，再以 mapping 为实施清单推进 graph compile + superstep 对齐。

- codex 开始执行：用户明确 060 改为复刻 LangGraph 图底层 compile + Pregel/superstep 模型；先产出源码 mapping 文档，再按 mapping 补齐实现。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-langgraph-compile-pregel-superstep-alignment.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-pregel-push-task-model-follow-up.md`。

- codex 开始执行：用户指出 060 不应被视作完整完成；当前只完成 LangGraph compile + Pregel/superstep 第一层骨架对齐，仍需继续对齐 interrupt、checkpoint recovery、channel/reducer、PUSH/Command 等长程能力。

- codex 开始执行：继续 060 超长程对齐：本轮按阶段推进，不宣称全量完成；先更新 mapping 为验收表，再补 Compile + State/Pregel checkpoint 语义，确保当前 TaskGraph 全测试仍通过。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-incremental-langgraph-channel-state-parity.md`。

- codex 开始执行：继续 060 长程对齐：本轮小步补 LangGraph State channel parity，优先 AnyValue / BinaryOperatorAggregate 语义，保持现有 TaskGraph 全量测试通过。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-langgraph-state-channel-reducer-parity-increment.md`。

- codex 开始执行：继续长程对齐 LangGraph：本轮推进 StateGraph attachNode/write mapper 与 Command/Send IR 的下一个可验证切片；060 保持 in_progress，不按完成处理。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-langgraph-stategraph-attachnode-command-send-parity-increment.md`。

- codex 开始执行：继续 060 长程对齐 LangGraph：本轮从 checkpoint recovery 切入，优先补 pending writes replay / skip-done-tasks 的可验证模型，保持现有 TaskGraph 任务可跑。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-langgraph-checkpoint-recovery-pending-writes-replay-increment.md`。

- codex 开始执行：开始长程推进 PregelLoop：先抽 Blackboard PregelLoop 内核外壳，承接 checkpoint/pending writes/prepare/apply，再逐步把 coordinator 散装 superstep 逻辑搬进去。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-pregelloop-长程推进-prepare-commit-外壳接入-coordinator.md`。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-pregelloop-长程推进-长期-loop-state-recursion-limit.md`。

- codex 开始执行：继续按顺序长程推进：checkpoint recovery → interrupt semantics → Annotation/channel factory → PregelLoop industrial details → subgraph namespace。当前先从 checkpoint recovery 切片开始。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-pregelloop-长程推进-checkpoint-recovery-interrupt-annotation-factory-industrial-details-namespace.md`。

- codex 开始执行：开始 060 cleanup pass：不新增 LangGraph 能力，优先清理 Pregel/Compile/Coordinator 模块边界与测试结构；不以向前兼容为约束，但保住当前 TaskGraph 核心测试。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-060-pregel-cleanup-pass-拆分-monolith-并收紧-pregel-模块边界.md`。

- codex 开始执行：补 TaskGraph 模块地图 README：让读者不看代码也能理解 task_graph 下各模块职责、核心执行链路与后续扩展落点。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-readme-模块地图.md`。

- codex 开始执行：开始整理 TaskGraph 测试结构：将顶层 `interpreter_tests` 并入 `tests/`，并在 tests 文件夹内按主题拆分多文件。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-测试目录整理-合并-interpreter-tests-到-tests.md`。

- codex 开始执行：开始将 TaskGraph `interpreter` 命名收束为 `runner`：重命名模块、入口类型、测试目录和文档引用，避免旧解释器语义继续误导。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-runner-命名收束.md`。

- codex 开始执行：开始整理 TaskGraph 节点模块：将 `node_exec` 改名为 `nodes`，并清理内部 control/runtime/subgraph 命名和引用。

- codex 开始执行：开始整理 task_graph 的 node_exec -> nodes 命名，并清理 nodes 内部模块边界。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-nodes-module-rename-cleanup.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-nodes-module-rename-cleanup-tool-audit.md`。

- codex 开始执行：继续 cleanup：收敛 task_graph 根目录裸文件，按职责分组整理模块边界。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-root-module-surface-cleanup.md`。

- codex 开始执行：修正 cleanup 命名：Pregel 才是 graph engine，将误导性的 engine facade 模块改为 orchestration。

- codex 开始执行：按产品边界修正：Pregel 才是 graph engine，将 engine/runner/coordinator/executor/outcome 并入 pregel。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-pregel-owns-runner-coordinator-executor-outcome.md`。

# 背景

TaskGraph 要支持“用户说几句话后离开，多个 Runtime 自主协作，用户回来验收”。这要求执行层不能只是顺序跑节点，也不能只做一次聊天调用。它需要像 LangGraph Pregel 一样有明确的 superstep、barrier、checkpoint、interrupt 和 resume 语义。

参考 proposal：`wiki/proposal/taskgraph-superstep-agent-orchestration.md`。

# 定位

060 是 TaskGraph 的核心层：定义一张图如何开始、暂停、恢复、并发、失败、重试、取消和进入下一轮。

060 只负责执行内核，不负责定义全部数据类型和业务节点 taxonomy。061 承接 dataflow/channel/artifact，062 承接 node taxonomy/runtime binding。

# 核心模型

一轮 superstep 的执行顺序：

1. 读取最新 TaskRun checkpoint。
2. 基于 graph spec、node run 状态、channel version 和 permission gate 选择本轮 runnable nodes。
3. 并发启动本轮 NodeRun。
4. NodeRun 只产出 writes/events/artifacts/interrupt，不直接修改全局状态。
5. barrier 等本轮 NodeRun 完成、失败、超时或中断。
6. 统一 apply writes，推进 channel versions。
7. checkpoint-first 持久化，再发 run events 给 UI/通知。
8. 生成下一轮 pending NodeRun，或进入 blocked/failed/ready_for_review/done。

# 状态机

TaskRun 状态：

- `pending`：已创建，尚未开始执行。
- `running`：至少一个 superstep 正在执行。
- `blocked`：缺少权限、输入、依赖或 runtime 可用性。
- `needs_approval`：等待用户批准继续。
- `failed`：不可自动恢复的失败。
- `ready_for_review`：实现和验证已完成，等待用户验收。
- `done`：用户验收或系统确认完成。
- `cancelled`：用户或策略取消。

NodeRun 状态：

- `pending`
- `running`
- `succeeded`
- `failed`
- `skipped`
- `retrying`
- `interrupted`
- `cancelled`

# 实施计划

1. 盘点现有 `bb_core/src/task_graph` 的 interpreter/runtime/run_state 能力，标记可复用和需要替换的部分。
2. 引入核心数据结构：`TaskRun`、`SuperstepRun`、`NodeRun`、`RunEvent`、`Checkpoint`、`PendingWrite`、`RuntimeSessionBinding`。
3. 实现 minimal scheduler：根据 graph spec 和 NodeRun 状态选择 runnable nodes，先支持固定模板图。
4. 实现 superstep runner：并发执行本轮 NodeRun，收集 writes/events/artifacts。
5. 实现 barrier + apply writes 的边界，具体 channel 语义由 #000061 定义。
6. 实现 checkpoint-first 持久化：写 checkpoint 后再发 UI event。
7. 实现 interrupt/approval/resume/cancel/retry/timeout 的第一阶段语义。
8. 对接现有 AgentSession/OpenCode runtime：NodeRun 可以绑定可恢复 session id。
9. 暴露 HTTP API：create run、get run、list events、resume/cancel/approve。
10. 写最小 e2e：Explorer -> Implementer -> Verifier -> Handoff 的固定图可以跨 superstep 跑完。

# Task 列表

- [ ] 读现有 TaskGraph interpreter/runtime/run_state 模块并形成差异清单。
- [ ] 定义 TaskRun/SuperstepRun/NodeRun/RunEvent/Checkpoint Rust 数据模型。
- [ ] 定义状态迁移函数，禁止 UI/API 直接写非法状态。
- [ ] 实现 `prepare_next_superstep`：从 checkpoint 和 graph spec 得到 runnable NodeRuns。
- [ ] 实现 `run_superstep`：并发执行 NodeRun 并收集 writes。
- [ ] 实现 barrier：本轮结束后统一提交 writes。
- [ ] 实现 checkpoint 存储接口，MVP 可先落 JSON，接口预留 SQLite/Postgres。
- [ ] 实现 runtime session binding，先支持 OpenCode/AgentSession。
- [ ] 实现 interrupt before/after、permission gate、approval resume。
- [ ] 实现 cancel、timeout、retry policy。
- [ ] 实现 run event stream API，UI 能持续读取状态。
- [ ] 增加单元测试覆盖状态机、循环依赖、失败重试、恢复。
- [ ] 增加一条 SWE 固定模板 e2e 测试。

# 验收标准

- 一个 TaskRun 可以从 pending 进入 running，并按 superstep 推进。
- NodeRun 并发执行后不会直接改全局状态，所有输出都通过 writes 进入 checkpoint。
- 任意 superstep 结束后都存在可恢复 checkpoint。
- 中断/权限审批后可以从 checkpoint 恢复继续跑。
- 失败、取消、超时、重试都有明确事件和状态。
- UI/API 可以查询 run 状态、节点状态和事件流。

# 依赖关系

- 上游：#000055 TaskGraph Task Execution。
- 下游：#000061 Dataflow/Channels/Artifacts 必须使用本 ticket 的 checkpoint/superstep 语义。
- 下游：#000062 Node Taxonomy 必须运行在本 ticket 提供的 NodeRun/SuperstepRun 模型上。

# 记录

- 验证：cargo fmt --all --check：通过（bb_backend workspace）。
- 验证：cargo check -p bb_cli：通过。
- 验证：cargo test -p bb_core task_graph --lib：通过，128 passed。
- 验证：npm run build --prefix bb_web：通过，仅保留既有 chunk size warning。

- 来源：2026-05-15-codex-taskgraph-060-superstep-execution-kernel.md
- 代码位置：bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs, coordinator.rs, bb_cli/src/http/task_graph/mod.rs, bb_web/src/components/TaskGraphRunPanel.vue

- 2026-05-15 纠偏重写：本票从“业务 Agent 角色/Palette”纠正为 LangGraph 蒸馏的执行内核票。060 只关心图如何被 compile 成可执行计划、如何按 superstep 推进、如何 checkpoint-first 持久化、如何产生日志事件；不定义 Explorer/Implementer 这类业务角色，也不处理 prompt source 或 Agent Profile。
- 工程边界：Blackboard 现有 LLM 节点、Agent Profile、OpenCode/Codex/Claude connector 是上层 runtime 实践，060 只提供它们将来挂载进去的 Pregel-like execution semantics。
- 验收口径：不是 UI palette 出现几个节点，而是一个已编译图能按 superstep 执行；每轮有明确 runnable set、barrier、pending writes、checkpoint、event log；崩溃后能从 checkpoint 恢复。

- 本阶段仍保留现有 cursor-based coordinator 作为执行兼容层；真正 LangGraph 式 `channel_versions + versions_seen` PULL 调度需要在 #000061 数据平面落地后继续替换 scheduler。
- 没有改 Explorer/Implementer palette、prompt source 或 Agent Profile 设计；这些仍明确排除在 060 外。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，3 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，134 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check -- <060 touched files>`：通过；仅有 Windows LF/CRLF 提示，无 whitespace error。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，4 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，135 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，6 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，137 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check -- <本轮触达文件>`：通过；仅 Windows LF/CRLF 提示。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，10 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，141 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅提示 `mod.rs` CRLF warning。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，12 passed。
- 验证：`cargo test -p bb_core task_graph::executor --lib --manifest-path bb_backend/Cargo.toml`：通过，2 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，145 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/executor.rs bb_backend/crates/bb_core/src/task_graph/coordinator.rs bb_backend/crates/bb_core/src/task_graph/interpreter.rs bb_backend/crates/bb_core/src/task_graph/outcome.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅有 CRLF warning。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，14 passed。
- 验证：`cargo test -p bb_core task_graph::tests::run_state_tests::test_pending_pregel_writes_roundtrip_and_clear --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- 验证：`cargo test -p bb_core recovery_replays_pending_start_writes_without_rerunning_start --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，149 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/executor.rs bb_backend/crates/bb_core/src/task_graph/coordinator.rs bb_backend/crates/bb_core/src/task_graph/interpreter.rs bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs bb_backend/crates/bb_core/src/task_graph/outcome.rs bb_backend/crates/bb_core/src/task_graph/run_state/mod.rs bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs bb_backend/crates/bb_core/src/task_graph/tests/mod.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅有 CRLF warning。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：151 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- 验证：`git diff --check` 针对本轮相关文件通过；仅有 Git CRLF 工作区提示。
- 验证：本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_core task_graph::pregel_loop --lib --manifest-path bb_backend/Cargo.toml` 通过：3 passed。
- 验证：`cargo test -p bb_core recovery_replays_pending_start_writes_without_rerunning_start --lib --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_core superstep_barrier_records_parallel_pending_writes --lib --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_core e2e_smoke_business_roles_supersteps_channels_and_resume --lib --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：152 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- 验证：`git diff --check` 针对本轮相关 tracked 文件通过；`rg -n "[ \\t]+$"` 针对相关 tracked/untracked 文件无尾随空白；仅有 Git CRLF 工作区提示。
- 验证：本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：Targeted tests 通过：`task_graph::pregel_loop`、`test_pregel_checkpoint_tuple_prefers_latest_superstep_checkpoint`、`recovery_replays_pending_start_writes_without_rerunning_start`、`interrupt_before_pauses_and_resume_runs_original_task`、`e2e_smoke_business_roles_supersteps_channels_and_resume`、`compile_graph_indexes_entrypoint_edges_and_joins`、`checkpoint_namespace_helpers_match_langgraph_shape`。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：156 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- 验证：`git diff --check` 针对本轮相关 tracked 文件通过；尾随空白扫描无结果；仅有 Git CRLF 工作区提示。
- 验证：本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check` 针对本轮相关文件：通过，仅有既有 CRLF 工作区提示。

- 验证：`rg -n "[ \\t]+$" bb_backend/crates/bb_core/src/task_graph/README.md`：无尾随空白。
- 验证：`git diff --check -- bb_backend/crates/bb_core/src/task_graph/README.md`：通过。
- 验证：本次只新增文档，未改 Rust/前端源码，未跑 cargo/npm 构建。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph::tests::interpreter --lib --manifest-path bb_backend/Cargo.toml`：通过，33 passed。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`git diff --check` 针对本轮相关文件：通过，仅有 CRLF 工作区提示。
- 验证：`rg -n "interpreter_tests|�|[ \\t]+$"` 针对 `task_graph/tests` 与 README：无结果。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- 验证：`rg -n "\\binterpreter\\b|\\bInterpreter\\b|\\bStepResult\\b|interpreter_config|tests/interpreter" ...`：无结果。
- 验证：`git diff --check` 针对本轮相关文件：通过，仅有 CRLF 工作区提示。

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed after renaming the runtime test file.
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- 验证：Repository-level stale scan `rg -n "node_exec|control_nodes|runtime_nodes|sub_graph_node" . -g '!bb_backend/target/**' -g '!node_modules/**' -g '!bb_web/node_modules/**' -g '!**/.git/**'` returned no matches.
- 验证：`rg -n "super::super" bb_backend/crates/bb_core/src/task_graph/nodes` returned no matches.
- 验证：`git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.

- 验证：No additional code changes in this audit note; previous validation remains: rustfmt passed, bb_core task_graph tests passed 156/156, bb_cli check passed, bb_cli tests passed 80/80, stale naming scans returned no matches.

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed.
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- 验证：`git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.
- 验证：Final root file check passed: `bb_backend/crates/bb_core/src/task_graph` now has only `mod.rs` and `README.md` as files.

- 验证：`cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed.
- 验证：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- 验证：`cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- 验证：`cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- 验证：Stale naming scan for `task_graph::engine`, `pub mod engine`, `engine/`, and facade wording returned no matches in task_graph/bb_cli task_graph code.
- 验证：`git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.
- 验证：Filesystem root directory check showed no `task_graph/engine` directory; `pregel/` now contains `runner.rs`, `coordinator.rs`, `executor.rs`, and `outcome.rs`.

# 下一步

- 后续由 #000061 将 pending writes 推进为 typed channel writes；由 #000062 把业务节点输出绑定到 artifact/channel contract。

- 定义 Graph Compile 输入输出：raw TaskGraph spec -> compiled executable graph，包含 node ids、edges、input/output channels、static validation errors、entrypoints、interrupt points。
- 实现 superstep scheduler：根据 checkpoint、node state、channel versions 和 dependency readiness 计算本轮 runnable nodes，禁止节点直接改全局状态。
- 实现 barrier 语义：同一 superstep 的节点并发运行，输出先进入 pending writes，本轮结束后统一提交。
- 实现 checkpoint-first lifecycle：每个 superstep 结束先写 checkpoint，再发布 run event/UI event，保证 durable execution。
- 实现 event log：记录 run_started、superstep_started、node_started、node_finished、writes_committed、checkpoint_saved、interrupted、resumed、failed、cancelled。
- 实现 minimal resume：从最后 checkpoint 恢复 run cursor，重新计算 runnable set，保证重复 resume 不重复提交已提交 writes。
- 补测试：compile 静态错误、两节点并发 superstep、barrier 后推进下一轮、失败后恢复、event log 顺序。

- 把 061 的 ChannelState / VersionsSeen 纳入 run checkpoint 后，060 下一步应把 `prepare_superstep_plan` 从 cursor/dependency 改为 Pregel-style `_prepareNextTasks`：基于 `trigger_to_nodes`、channel availability、channel_versions、versions_seen 计算 PULL tasks。
- 在 062 落 interrupt/Command 后，把 `interruptBefore/interruptAfter`、RESUME/INTERRUPT reserved channel 接进当前 `CompiledGraph` 与 event/checkpoint 模型。

- 060 仍未完整复刻 LangGraph 的 PUSH/TASKS/Send 动态任务派发；当前已建 reserved channel 和 Pregel task model，后续需要把 Send packet 写入/读取接进执行路径。
- interruptBefore/interruptAfter 还需要按 LangGraph 的 channel version + triggered tasks 语义实现。
- Channel 类型目前按 Blackboard compiled channel kind 实现 MVP；后续 061 需要补 LastValue/Topic/Aggregate/ArtifactRef 的 reducer parity 与 durable channel store。

- 剩余 LangGraph parity 主要是 interruptBefore/interruptAfter 的版本判断和后续 061 reducer/channel store 深化。

- 继续保持 000060 in_progress，不视为完成。下一小步建议补 StateGraph Annotation/reducer -> channel factory 的 Blackboard 对齐入口，或先补 interruptBefore/interruptAfter 的 LangGraph 版本判断。

- 继续 060，不切到完成态：下一步对齐 LangGraph `StateGraph.attachNode` 的 state write mapper，让业务节点返回对象后能按 state key 写入对应 channel。
- 继续补 `Command/goto/Send` writer 入 IR，与现有 `__pregel_tasks` PUSH task 准备逻辑接起来。
- 随后补 interruptBefore/interruptAfter 的 versions_seen 语义，以及 checkpoint tuple / pending writes recovery。

- 继续 060：对齐 LangGraph checkpoint recovery，重点是 pending writes replay、skip-done-tasks、checkpoint tuple / parent / namespace。
- 继续补 interruptBefore/interruptAfter 的 versions_seen / __interrupt__ 精确语义。
- 继续补 StateGraph Annotation/Zod schema -> channel factory，以及 Command graph/subgraph/PARENT 的完整语义。

- 继续 060：补 checkpoint tuple / parent checkpoint / namespace，把 run-local pending writes 模型升级成更贴近 LangGraph CheckpointTuple 的结构。
- 继续 060：补 interruptBefore/interruptAfter 的 `versions_seen[__interrupt__]` 精确语义。
- 继续 060：补 retry/cache/no_writes/error channel 与 task scratchpad/read/write runtime API。

- 继续贴 LangGraph `PregelLoop.tick()`：把 stop/recursion limit、interruptBefore/interruptAfter、durability/checkpoint promise、stream output、scratchpad/read/write runtime API 逐步搬进 Blackboard loop。
- 后续可把 coordinator 的 pending writes 持久化也下沉进 `PregelLoop.put_writes` / loop-local persistence adapter，进一步减少 coordinator 的 Pregel 细节。

- 继续贴 LangGraph `PregelLoop.tick()`：interruptBefore/interruptAfter、durability/checkpoint promise、stream output、scratchpad/read/write runtime API、managed values、subgraph namespace。
- 下一段建议优先做 interrupt 语义，因为它会决定 HumanGate/resume 与 checkpoint `versions_seen` 的正确边界。

- 继续 060 时，建议下一轮按剩余差距推进：Command resume payload / GraphInterrupt envelope → checkpointer durability promises → scratchpad/read/write runtime API → 子图 checkpoint ID 与父 checkpoint 原子绑定 → stream/debug task envelope。

- 后续 cleanup 建议继续拆 `coordinator.rs` 的 persistence/event projection adapter；当前 coordinator 仍是下一个最容易变胖的文件。
- 后续再补 LangGraph 新能力时，先更新 mapping 的 owner module，再落代码和测试。

- 后续如果继续拆 `coordinator.rs`，同步更新 README 的模块职责和新能力落点。

- 后续可继续把 `tests/mod.rs` 里原 store/run_state 大模块拆成 `store.rs`、`validation.rs`、`run_state.rs`，让整个 tests 目录完全主题化。

- 后续如继续清理，可考虑将 `execute_run` / `resume_run` 保持为 public verb，但把内部文档明确为 compile-based Pregel runner facade。

- No functional follow-up required for this rename. Existing working tree still contains broader 060/runner/Pregel changes from the current long-running task; this cleanup did not stage or revert them.

- None.

- No frontend build was run because this change did not touch `bb_web/src`.
- Existing larger 060/Pregel/runner working-tree changes are still present; this cleanup did not stage or revert unrelated concurrent changes.

- No frontend build was run because no `bb_web/src` files changed.
- No staging/revert was performed; the working tree still includes broader 060 cleanup/Pregel changes from the ongoing task.
