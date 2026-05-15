# 060 Pregel cleanup pass completed

时间: 2026-05-15T08:53:52Z
来源: codex
项目: blackboard

## 做了什么

- 将原 `bb_backend/crates/bb_core/src/task_graph/pregel.rs` 拆成 `pregel/model.rs`、`checkpoint.rs`、`prepare.rs`、`writes.rs`、`command.rs`、`interrupt.rs`、`namespace.rs`、`runtime_channels.rs`、`tests.rs`，公开出口统一到 `pregel/mod.rs`。
- 将原 `bb_backend/crates/bb_core/src/task_graph/pregel_loop.rs` 并入 `pregel/loop_state.rs`，让 PregelLoop 与 Pregel 算法同属一个模块树。
- 移除 `task_graph` 顶层 Pregel re-export 复制层；内部调用改为走 `task_graph::pregel::*`，不继续保护旧 alias。
- 更新 mapping 文档，新增 Blackboard Module Ownership 表，明确 compile / checkpoint / prepare / writes / command / interrupt / loop_state / coordinator 的职责边界。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check` 针对本轮相关文件：通过，仅有既有 CRLF 工作区提示。

## 下一步

- 后续 cleanup 建议继续拆 `coordinator.rs` 的 persistence/event projection adapter；当前 coordinator 仍是下一个最容易变胖的文件。
- 后续再补 LangGraph 新能力时，先更新 mapping 的 owner module，再落代码和测试。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel/mod.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/model.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/checkpoint.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/command.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/interrupt.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs
- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
