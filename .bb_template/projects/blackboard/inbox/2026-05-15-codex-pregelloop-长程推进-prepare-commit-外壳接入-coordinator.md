# 060 PregelLoop prepare/commit kernel increment

时间: 2026-05-15T07:46:57Z
来源: Codex
项目: blackboard

## 做了什么

- 新增 `bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs`，抽出 Blackboard `PregelLoop`：持有 compiled graph、active checkpoint、recovered pending writes、loop status，并提供 `prepare_next`、`put_writes`、`commit_step`。
- `PregelLoop.commit_step` 统一合并 replayed tasks/writes 与 executed writes，调用现有 Pregel `apply_writes` 产生新 checkpoint，并清理已提交 task 对应的 recovered pending writes。
- `GraphCoordinator` 的 plan 生成改为通过 `PregelLoop.prepare_next`；superstep barrier checkpoint 改为通过 `PregelLoop.commit_step`。
- `writes_committed` event payload 增加 `executed_task_count`、`replayed_task_count`、`completed_task_count`、`applied_pregel_write_count`，方便量化每轮 Pregel barrier 产出。
- 更新 `.bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md`，记录 PregelLoop 已对齐部分和仍未对齐的 LangGraph `tick()` 差距。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` 通过：151 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` 通过：80 passed。
- `git diff --check` 针对本轮相关文件通过；仅有 Git CRLF 工作区提示。
- 本轮未修改 `bb_web/src`，所以未跑 `npm run build --prefix bb_web`。

## 下一步

- 继续贴 LangGraph `PregelLoop.tick()`：把 stop/recursion limit、interruptBefore/interruptAfter、durability/checkpoint promise、stream output、scratchpad/read/write runtime API 逐步搬进 Blackboard loop。
- 后续可把 coordinator 的 pending writes 持久化也下沉进 `PregelLoop.put_writes` / loop-local persistence adapter，进一步减少 coordinator 的 Pregel 细节。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
