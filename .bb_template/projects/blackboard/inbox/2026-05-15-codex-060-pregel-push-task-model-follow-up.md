# 060 补齐 Pregel PUSH/TASKS 任务模型

时间: 2026-05-15T06:35:22Z
来源: codex
项目: blackboard

## 做了什么

- 在 `pregel.rs` 增加 `PregelSend` 与 `send_packet`，`prepare_next_tasks` 现在会从 `__pregel_tasks` channel 解析 Send packet 并生成 `PregelTaskKind::Push` 任务。
- `apply_writes` 支持向 `__pregel_tasks` 写入单个 packet 或 packet array，后续动态编排可以走同一 Pregel 调度入口。
- 新增 PUSH 单测，确认 `__pregel_tasks` 中的 `{ node, args }` 会生成目标节点的 PUSH task。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，4 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，135 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。

## 下一步

- 剩余 LangGraph parity 主要是 interruptBefore/interruptAfter 的版本判断和后续 061 reducer/channel store 深化。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
