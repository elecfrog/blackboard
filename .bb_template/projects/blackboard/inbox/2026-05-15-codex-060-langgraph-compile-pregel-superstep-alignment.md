# 060 对齐 LangGraph 图编译与 Pregel superstep 内核

时间: 2026-05-15T06:32:50Z
来源: codex
项目: blackboard

## 做了什么

- 阅读并对齐 LangGraphJS 源码：graph/state compile、pregel algo/loop、BaseChannel、EphemeralValue、NamedBarrierValue，以及已蒸馏的 Pregel/checkpoint/channel 文档。
- 新增 mapping 文档 `wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md`，列出 LangGraph 模型、Blackboard 已做、缺口和补齐任务。
- 新增 Blackboard Pregel 内核模块 `bb_backend/crates/bb_core/src/task_graph/pregel.rs`：`PregelCheckpoint`、`PregelTask`、`PregelWrite`、`prepare_next_tasks`、`apply_writes`、channel availability、barrier readiness、versions_seen/channel_versions。
- 扩展 run state：run.json 与 superstep checkpoint 现在保存 `pregel_checkpoint`，包含 `channel_values/channel_versions/versions_seen/updated_channels`。
- Coordinator 调度入口从 cursor-based 改为 Pregel checkpoint 驱动：基于 compiled processes 的 triggers 和 channel versions 生成本轮 runnable tasks；cursor 退为 UI/projection。
- 节点 outcome 在 barrier 前转为 Pregel channel writes；barrier 统一 apply writes 并写入 checkpoint，再发 writes/checkpoint/run events。
- HumanGate resume 现在会把批准结果写回 Pregel checkpoint，触发下游 channel，避免 resume 后只改 cursor。
- 补齐 loop 隐式回边兼容：当 node_exec 返回 loop continuation 时，可写入已编译的 `branch:to:<loop>` channel。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，3 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，134 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check -- <060 touched files>`：通过；仅有 Windows LF/CRLF 提示，无 whitespace error。

## 下一步

- 060 仍未完整复刻 LangGraph 的 PUSH/TASKS/Send 动态任务派发；当前已建 reserved channel 和 Pregel task model，后续需要把 Send packet 写入/读取接进执行路径。
- interruptBefore/interruptAfter 还需要按 LangGraph 的 channel version + triggered tasks 语义实现。
- Channel 类型目前按 Blackboard compiled channel kind 实现 MVP；后续 061 需要补 LastValue/Topic/Aggregate/ArtifactRef 的 reducer parity 与 durable channel store。

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/model.rs
- bb_backend/crates/bb_core/src/task_graph/run_state/lifecycle.rs
- bb_backend/crates/bb_core/src/task_graph/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
