# 060 cleanup: node_exec renamed to nodes

时间: 2026-05-15T09:53:08Z
来源: codex
项目: blackboard

## 做了什么

- `bb_list_agents(project=blackboard)` passed; confirmed registered agent id is `codex` after initial `begin_ticket_work(agent=Codex)` was rejected.
- `bb_begin_ticket_work(project=blackboard,id=000060,agent=codex)` passed and marked ticket in progress.
- Renamed `bb_core/src/task_graph/node_exec/` to `nodes/`.
- Renamed internal node folders/files: `control_nodes/` -> `control/`, `runtime_nodes/` -> `runtime/`, `sub_graph_node.rs` -> `subgraph.rs`.
- Updated module wiring: `task_graph/mod.rs`, `executor.rs`, `runner.rs`, and node dispatch now use `nodes`.
- Cleaned nodes internals by replacing nested `super::super::super` imports with explicit `crate::task_graph::...` imports.
- Updated `task_graph/README.md` module map and corrected node execution docs to reflect final reducer writes plus live runtime progress projections.
- Renamed runner test module file `tests/runner/runtime_nodes.rs` -> `tests/runner/runtime.rs`.

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml` passed after renaming the runtime test file.
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml` passed: 156 tests passed.
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml` passed.
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml` passed: 80 tests passed.
- Repository-level stale scan `rg -n "node_exec|control_nodes|runtime_nodes|sub_graph_node" . -g '!bb_backend/target/**' -g '!node_modules/**' -g '!bb_web/node_modules/**' -g '!**/.git/**'` returned no matches.
- `rg -n "super::super" bb_backend/crates/bb_core/src/task_graph/nodes` returned no matches.
- `git diff --check -- bb_backend/crates/bb_core/src/task_graph bb_backend/crates/bb_cli/src/http/task_graph` passed; only CRLF conversion warnings were emitted.

## 下一步

- No functional follow-up required for this rename. Existing working tree still contains broader 060/runner/Pregel changes from the current long-running task; this cleanup did not stage or revert them.

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/nodes/
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_backend/crates/bb_core/src/task_graph/executor.rs
- bb_backend/crates/bb_core/src/task_graph/runner.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/README.md
- bb_backend/crates/bb_core/src/task_graph/tests/runner/runtime.rs
