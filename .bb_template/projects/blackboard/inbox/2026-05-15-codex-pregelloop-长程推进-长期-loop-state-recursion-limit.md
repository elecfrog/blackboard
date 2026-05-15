# 060 PregelLoop long-lived state increment

时间: 2026-05-15T07:55:55Z
来源: Codex
项目: blackboard

## 做了什么

- 新增 `bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs`，`PregelLoop` 现在持有 compiled graph、active checkpoint、pending writes、step、stop、status。
- `PregelLoop.prepare_next` 负责从 checkpoint/pending writes 准备下一轮任务；`PregelLoop.put_writes` 接收 task-local Pregel writes；`PregelLoop.commit_step` 从 loop-local pending writes 合并 executed writes 与 replayed writes，并在 barrier 后产出新 checkpoint。
- `GraphCoordinator` 改为持有长期 `PregelLoop` state，不再在 prepare/commit 阶段临时重建 loop；pending writes 持久化改为来自 `pregel_loop.pending_writes()`。
- graph metadata 新增可选 `recursion_limit`，接入 `PregelLoop.stop`；超过上限时 prepare 阶段返回 `PregelLoopStatus::OutOfSteps`，coordinator 会给出明确失败原因。
- `writes_committed` event payload 保留量化字段：`executed_task_count`、`replayed_task_count`、`completed_task_count`、`applied_pregel_write_count`。
- 更新 mapping 文档，记录长期 PregelLoop state、`put_writes`、`recursion_limit` 与仍未对齐的 LangGraph `tick()` 差距。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_core task_graph::pregel_loop --lib --manifest-path bb_backend/Cargo.toml` 通过：3 passed。
- `cargo test -p bb_core recovery_replays_pending_start_writes_without_rerunning_start --lib --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_core superstep_barrier_records_parallel_pending_writes --lib --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_core e2e_smoke_business_roles_supersteps_channels_and_resume --lib --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：152 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- `git diff --check` 针对本轮相关 tracked 文件通过；`rg -n "[ \\t]+$"` 针对相关 tracked/untracked 文件无尾随空白；仅有 Git CRLF 工作区提示。
- 本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

## 下一步

- 继续贴 LangGraph `PregelLoop.tick()`：interruptBefore/interruptAfter、durability/checkpoint promise、stream output、scratchpad/read/write runtime API、managed values、subgraph namespace。
- 下一段建议优先做 interrupt 语义，因为它会决定 HumanGate/resume 与 checkpoint `versions_seen` 的正确边界。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/types.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
