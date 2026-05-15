# 060 PregelLoop ordered parity increment

时间: 2026-05-15T08:21:47Z
来源: Codex
项目: blackboard

## 做了什么

- 按用户指定顺序完成一轮长程推进：checkpoint recovery → interrupt semantics → Annotation/channel factory → PregelLoop industrial details → subgraph namespace。
- Checkpoint recovery：新增 Blackboard `PregelCheckpointTuple` / `PregelCheckpointConfig` / `PregelCheckpointMetadata`，tuple 包含 config、checkpoint、metadata、parentConfig、pendingWrites；`read_pregel_checkpoint_tuple` 会从最新 superstep checkpoint 或更新的 run.json Pregel checkpoint 恢复。
- Checkpoint recovery：coordinator load 改为通过 checkpoint tuple 恢复 PregelLoop，避免 run.json checkpoint 滞后于 superstep checkpoint；同时修复 resume 后 run.json checkpoint 比 checkpoint 文件更新时应优先使用 run.json 的场景。
- Interrupt semantics：`PregelLoopStatus` 增加 `InterruptBefore` / `InterruptAfter`；graph metadata 增加 `interrupt_before` / `interrupt_after`；中断判断基于 channel version 与 `versions_seen[__interrupt__]`。
- Interrupt semantics：HumanGate pause 写 `__interrupt__`，resume 写 `__resume__`；interruptBefore 会 pause 在目标节点前，resume 后继续执行原任务且不会重复中断。
- Annotation/channel factory：graph input 增加显式 `reducer` 与 `channel_class`，compiler 可生成 `BinaryOperatorAggregate(Append/MergeObject/Sum)`、`Topic`、`AnyValue`、`LastValue` 等 state channel。
- PregelLoop industrial detail：`PregelLoopCommit` 增加 `new_versions`，`writes_committed` event payload 可量化本轮 channel version 增量。
- Subgraph namespace：新增 LangGraph-style namespace helper，支持 `parent|child` 与 `namespace:taskId`；SubGraph child run 会持久化 `checkpoint_ns`，run summary 也暴露该字段。
- 更新 mapping 文档，标记上述五段的已对齐范围与剩余差距。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- Targeted tests 通过：`task_graph::pregel_loop`、`test_pregel_checkpoint_tuple_prefers_latest_superstep_checkpoint`、`recovery_replays_pending_start_writes_without_rerunning_start`、`interrupt_before_pauses_and_resume_runs_original_task`、`e2e_smoke_business_roles_supersteps_channels_and_resume`、`compile_graph_indexes_entrypoint_edges_and_joins`、`checkpoint_namespace_helpers_match_langgraph_shape`。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：156 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- `git diff --check` 针对本轮相关 tracked 文件通过；尾随空白扫描无结果；仅有 Git CRLF 工作区提示。
- 本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

## 下一步

- 继续 060 时，建议下一轮按剩余差距推进：Command resume payload / GraphInterrupt envelope → checkpointer durability promises → scratchpad/read/write runtime API → 子图 checkpoint ID 与父 checkpoint 原子绑定 → stream/debug task envelope。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter.rs
- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/types.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/model.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/sub_graph_node.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs
- bb_backend/crates/bb_core/src/task_graph/tests/mod.rs
- bb_backend/crates/bb_cli/src/http/task_graph/mod.rs
- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
