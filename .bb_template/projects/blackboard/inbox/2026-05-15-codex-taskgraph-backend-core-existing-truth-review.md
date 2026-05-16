# Reviewed feature/orni backend TaskGraph implementation vs main

时间: 2026-05-15T17:48:33Z
来源: Codex
项目: blackboard

## 做了什么

- Compared current branch `feature/orni` against `main`; branch contains commit `5c3c84a feat: pregel task engine`.
- Scoped review to backend core and node runtime; frontend was intentionally excluded per user request.
- Confirmed main branch used cursor-driven `interpreter/coordinator/executor/node_exec`; current branch replaces task graph execution core with `compile/`, `pregel/`, reorganized `nodes/`, and expanded `run_state/`.
- Identified implemented backend capabilities: compiled graph IR, Pregel checkpoint/task/write model, superstep prepare/apply barrier, pending write replay, run events/superstep checkpoints, Command/Send MVP, interrupt-before/after, Shell node, LLM AgentSession runtime, SubGraph checkpoint namespace propagation, node registry roles layered over existing executors.
- Identified not implemented in backend core: Pregel paper 3.4 runtime topology mutation, graph mutation request queue, add/remove node/edge at runtime, coordinator-driven graph modification, Arena/plan-review mechanism.

## 验证了什么

- Ran `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`: passed, 159 tests passed, 0 failed.
- Used `bb_list_projects`, `bb_find_work_context`, and `bb_create_inbox_note`; all succeeded.

## 下一步

- If this becomes an acceptance artifact, split into a dedicated `existing_truth.md` beside the Pregel topology mutation experiment and keep it separate from research/design specs.

## 相关位置

- （未填写）
