# 060 继续对齐 LangGraph：pending writes replay / skip done task MVP

时间: 2026-05-15T07:30:23Z
来源: codex
项目: blackboard

## 做了什么

- 继续 060 长程对齐，未收口 ticket；本轮从 LangGraph `PregelLoop.putWrites`、`_prepareSingleTask` 的 recovery 语义切入。
- Pregel 层新增 `prepare_next_tasks_with_pending_writes`，同 task id 如果已有非 error pending writes，则跳过重跑并把 task/writes 放入 replay 集合。
- `PregelPreparedStep` 新增 `replayed_tasks` / `replayed_writes`，让 barrier 可以把已完成 task 的 writes 重新 apply 到 checkpoint。
- run_state 新增 `pending_pregel_writes.json` 读写/清理入口，用于在 barrier checkpoint 前持久化 task writes。
- Coordinator 启动时读取 run-local pending writes；plan 阶段传入 recovery pending writes；record_superstep 时合并 replayed writes + 本轮 writes，一起进入 Pregel checkpoint，并在成功 checkpoint 后清理 pending 文件。
- 补 interpreter 级 recovery 测试：模拟 start task writes 已持久化但 barrier checkpoint 未保存，重启后不重跑 start，而是回放 writes 后继续执行。
- 更新 mapping 文档 L6/B/C/E，把 pending writes replay / skip-done-tasks 标为 MVP 已完成，同时保留 checkpoint tuple / namespace / parent checkpoint 等后续缺口。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，14 passed。
- `cargo test -p bb_core task_graph::tests::run_state_tests::test_pending_pregel_writes_roundtrip_and_clear --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- `cargo test -p bb_core recovery_replays_pending_start_writes_without_rerunning_start --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，149 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/executor.rs bb_backend/crates/bb_core/src/task_graph/coordinator.rs bb_backend/crates/bb_core/src/task_graph/interpreter.rs bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs bb_backend/crates/bb_core/src/task_graph/outcome.rs bb_backend/crates/bb_core/src/task_graph/run_state/mod.rs bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs bb_backend/crates/bb_core/src/task_graph/tests/mod.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅有 CRLF warning。

## 下一步

- 继续 060：补 checkpoint tuple / parent checkpoint / namespace，把 run-local pending writes 模型升级成更贴近 LangGraph CheckpointTuple 的结构。
- 继续 060：补 interruptBefore/interruptAfter 的 `versions_seen[__interrupt__]` 精确语义。
- 继续 060：补 retry/cache/no_writes/error channel 与 task scratchpad/read/write runtime API。

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/mod.rs
- bb_backend/crates/bb_core/src/task_graph/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs
- bb_backend/crates/bb_core/src/task_graph/tests/mod.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
