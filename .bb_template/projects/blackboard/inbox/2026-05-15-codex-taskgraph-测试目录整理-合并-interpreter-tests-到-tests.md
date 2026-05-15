# 整理 TaskGraph interpreter tests 到 tests 目录

时间: 2026-05-15T09:12:19Z
来源: codex
项目: blackboard

## 做了什么

- 移除顶层 `bb_backend/crates/bb_core/src/task_graph/interpreter_tests/` 测试入口；`task_graph/mod.rs` 不再声明 `mod interpreter_tests`。
- 在 `bb_backend/crates/bb_core/src/task_graph/tests/mod.rs` 挂载 `mod interpreter;`，让解释器测试统一归到 `task_graph::tests` 下。
- 将原解释器大测试拆到 `bb_backend/crates/bb_core/src/task_graph/tests/interpreter/`：`eval.rs`、`runs.rs`、`runtime_nodes.rs`、`fork_join.rs`、`fixtures.rs`、`mod.rs`。
- 同步更新 `task_graph/README.md`，补充 `tests/interpreter/*` 的职责说明。
- 清理拆分后遗留的未使用 import 和旧注释乱码。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::tests::interpreter --lib --manifest-path bb_backend/Cargo.toml`：通过，33 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check` 针对本轮相关文件：通过，仅有 CRLF 工作区提示。
- `rg -n "interpreter_tests|�|[ \\t]+$"` 针对 `task_graph/tests` 与 README：无结果。

## 下一步

- 后续可继续把 `tests/mod.rs` 里原 store/run_state 大模块拆成 `store.rs`、`validation.rs`、`run_state.rs`，让整个 tests 目录完全主题化。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_backend/crates/bb_core/src/task_graph/tests/mod.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/mod.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/eval.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/runs.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/runtime_nodes.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/fork_join.rs
- bb_backend/crates/bb_core/src/task_graph/tests/interpreter/fixtures.rs
- bb_backend/crates/bb_core/src/task_graph/README.md
