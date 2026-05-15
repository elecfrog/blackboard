# 060 cleanup: engine module merged into pregel

时间: 2026-05-15T12:37:26Z
来源: codex
项目: blackboard

## 做了什么

- `bb_list_projects` passed; confirmed `blackboard` project is visible.
- `bb_begin_ticket_work(project=blackboard,id=000060,agent=codex)` passed.
- Removed the misleading `task_graph/engine/` module from the filesystem.
- Moved `runner.rs`, `coordinator.rs`, `executor.rs`, and `outcome.rs` into `task_graph/pregel/`, so Pregel is the single graph execution engine boundary.
- Updated `task_graph/pregel/mod.rs` to expose `runner`, `coordinator`, `executor`, and `outcome` alongside the Pregel kernel modules.
- Updated root `task_graph/mod.rs`: removed `pub mod engine`, re-exported runner/outcome/coordinator/executor through `pregel`, and documented that Pregel is the graph execution engine.
- Updated all internal imports from `task_graph::engine::*` to `task_graph::pregel::*`.
- Updated `task_graph/README.md` to remove the separate engine/facade concept and describe `pregel/` as the graph execution engine package.

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed.
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- Stale naming scan for `task_graph::engine`, `pub mod engine`, `engine/`, and facade wording returned no matches in task_graph/bb_cli task_graph code.
- `git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.
- Filesystem root directory check showed no `task_graph/engine` directory; `pregel/` now contains `runner.rs`, `coordinator.rs`, `executor.rs`, and `outcome.rs`.

## 下一步

- No frontend build was run because no `bb_web/src` files changed.
- No staging/revert was performed; the working tree still includes broader 060 cleanup/Pregel changes from the ongoing task.

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/pregel/mod.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/runner.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/executor.rs
- bb_backend/crates/bb_core/src/task_graph/pregel/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_backend/crates/bb_core/src/task_graph/README.md
