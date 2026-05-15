# 060 cleanup: task_graph root files grouped by responsibility

时间: 2026-05-15T12:27:22Z
来源: codex
项目: blackboard

## 做了什么

- `bb_list_projects` passed; confirmed `blackboard` project is visible.
- `bb_begin_ticket_work(project=blackboard,id=000060,agent=codex)` passed and moved ticket back to in_progress for cleanup.
- Collapsed `bb_core/src/task_graph` root surface to only `mod.rs` and `README.md`.
- Moved graph definition files into `definition/`: `types.rs`, `store.rs`, `pins.rs`, `upgrade.rs`, and `validation/`.
- Moved compile files into `compile/`: `compiler.rs` and `channels.rs`.
- Moved execution orchestration files into `engine/`: `runner.rs`, `coordinator.rs`, `executor.rs`, and `outcome.rs`.
- Moved node-support files into `nodes/`: `eval.rs`, `llm.rs`, and `registry.rs` while keeping node dispatch/control/runtime/subgraph there.
- Moved `schedules.rs` into `schedules/mod.rs`.
- Updated `task_graph/mod.rs` to expose responsibility modules while preserving existing public reexports such as `task_graph::types`, `task_graph::runner`, `task_graph::compiler`, and `task_graph::node_registry`.
- Updated internal imports in moved files to point at real submodule paths where practical.
- Updated `task_graph/README.md` to document the new module map and placement rules.

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed.
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- `git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.
- Final root file check passed: `bb_backend/crates/bb_core/src/task_graph` now has only `mod.rs` and `README.md` as files.

## 下一步

- No frontend build was run because this change did not touch `bb_web/src`.
- Existing larger 060/Pregel/runner working-tree changes are still present; this cleanup did not stage or revert unrelated concurrent changes.

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_backend/crates/bb_core/src/task_graph/README.md
- bb_backend/crates/bb_core/src/task_graph/definition/
- bb_backend/crates/bb_core/src/task_graph/compile/
- bb_backend/crates/bb_core/src/task_graph/engine/
- bb_backend/crates/bb_core/src/task_graph/nodes/
- bb_backend/crates/bb_core/src/task_graph/schedules/
