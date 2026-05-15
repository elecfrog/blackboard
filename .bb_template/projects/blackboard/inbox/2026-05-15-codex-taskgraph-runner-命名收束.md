# 将 TaskGraph interpreter 命名整体改为 runner

时间: 2026-05-15T09:39:46Z
来源: codex
项目: blackboard

## 做了什么

- 将 `bb_backend/crates/bb_core/src/task_graph/interpreter.rs` 重命名为 `runner.rs`，`task_graph/mod.rs` 改为 `pub mod runner` 并从 `runner` 导出 run/resume API。
- 将 `InterpreterOptions` 重命名为 `RunnerOptions`，`StepResult` 重命名为 `RunnerStepResult`；core 内 `coordinator`、`executor`、`llm`、`runtime`、`node_exec`、subgraph 调用均改为 runner 命名。
- 将 `bb_backend/crates/bb_cli/src/http/task_graph/interpreter_config.rs` 重命名为 `runner_config.rs`，并将 `build_interpreter_opts` / `InterpreterOverrides` 改为 `build_runner_opts` / `RunnerOverrides`；daemon、HTTP task_graph、agent_sessions、HTTP tests 均已更新。
- 将测试目录 `task_graph/tests/interpreter/` 重命名为 `task_graph/tests/runner/`，测试模块路径变为 `task_graph::tests::runner::*`。
- 更新 `task_graph/README.md` 的模块说明和 tests 目录说明，删除旧 interpreter 语义。
- 按词边界扫描确认无 `interpreter` / `Interpreter` / `StepResult` / `interpreter_config` / `tests/interpreter` 残留。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，156 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `rg -n "\\binterpreter\\b|\\bInterpreter\\b|\\bStepResult\\b|interpreter_config|tests/interpreter" ...`：无结果。
- `git diff --check` 针对本轮相关文件：通过，仅有 CRLF 工作区提示。

## 下一步

- 后续如继续清理，可考虑将 `execute_run` / `resume_run` 保持为 public verb，但把内部文档明确为 compile-based Pregel runner facade。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/runner.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/executor.rs
- bb_backend/crates/bb_core/src/task_graph/runtime/mod.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/sub_graph_node.rs
- bb_backend/crates/bb_core/src/task_graph/tests/runner/mod.rs
- bb_backend/crates/bb_cli/src/http/task_graph/runner_config.rs
- bb_backend/crates/bb_cli/src/daemon.rs
- bb_backend/crates/bb_core/src/task_graph/README.md
