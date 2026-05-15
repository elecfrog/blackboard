# Codex handoff：060 compile/superstep kernel 源码对齐实现

时间: 2026-05-15T06:07:30Z
来源: codex
项目: blackboard

## 做了什么

- 读取 `C:\Dev\langgraphjs` 源码确认 LangGraph compile 思路：Graph/StateGraph compile 降到 Pregel processes/channels/triggers/writers，而不是简单 adjacency map。
- 新增 `bb_backend/crates/bb_core/src/task_graph/compiler.rs`，输出 Pregel-style `CompiledGraph`：processes、channels、trigger_to_nodes、writers、reserved START/END、branch channel、join barrier channel、node output stream channels。
- `create_run` 现在会写 `graph.compiled.json`，每个 run 都带可审计编译产物。
- Coordinator 使用 compiled entrypoint/join_nodes，新增 `SuperstepPlan`，并将节点输出作为 `PendingWrite` 写入 superstep checkpoint。
- Event log 增加 `run_started`、`superstep_started`、`node_started`、`node_finished`、`writes_committed`、`checkpoint_saved`。
- 补测试覆盖 compiler IR、compiled snapshot 落盘、并行 superstep pending writes checkpoint、事件流粒度。
- 通过 bb_append_ticket_sections 更新 000060 进展、记录和下一步。

## 验证了什么

- cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml：通过。
- cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml：通过，131 passed。
- cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml：通过，80 passed。
- git diff --check -- <060 touched backend files>：通过，仅有 Windows LF/CRLF warning，无 whitespace error。
- bb_append_ticket_sections 000060：通过，maintenance consistency passed，export completed。

## 下一步

- 当前 Coordinator 仍保留 cursor-based 执行兼容层；下一阶段需要等 061 ChannelState/VersionsSeen 纳入 checkpoint 后，把 scheduler 改为 LangGraph 式 `_prepareNextTasks`：按 trigger_to_nodes、channel_versions、versions_seen 计算 PULL tasks。
- 062 落 interrupt/Command 后，再接入 interruptBefore/interruptAfter 与 RESUME/INTERRUPT reserved channels。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/lifecycle.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs
- tickets/000060-taskgraph-execution-foundation.md
