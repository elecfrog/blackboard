# 060 继续对齐 LangGraph：State write mapper / Command / Send / PUSH args

时间: 2026-05-15T07:18:31Z
来源: codex
项目: blackboard

## 做了什么

- 继续 060 长程对齐，未收口 ticket；本轮基于 LangGraph `StateGraph.attachNode`、`ChannelWrite`、`Command`、`Send` 源码推进。
- compile IR 扩展 `CompiledWriteValue::StateKey`，非 End process 现在带隐藏 state writers，表达 StateGraph `ChannelWriteTupleEntry` / `_getUpdates` 的 writer 语义。
- Pregel `writes_from_node_outcome` 改为可校验的 `Result<Vec<PregelWrite>>`，并新增 state update mapper：节点返回对象或 `Command.update` 时，按 key 写入匹配的 `state:<key>` channel。
- 补 `Command.goto` / `Send` 输出映射：`goto` 生成 branch writes，`Send` 生成 `__pregel_tasks` packets；同时修复 `state:<node>` 被误判为控制边的 channel 匹配问题。
- 修正 `parse_send`，只从对象格式解析 Send，避免 serde 把 array 误解析成 struct sequence。
- executor 增加 Pregel task-local input：`ReadyNode` 携带 task kind/input，PUSH task 的 `args` 会作为本次节点执行的 `run.context.input`，PULL 保持原 run input projection。
- 更新 mapping 文档 L4/L7/A/C，明确已完成 State write mapper、Command/Send writes、PUSH args executor input；也保留 Command graph/subgraph、scratchpad/runtime API、recovery 等后续差距。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，12 passed。
- `cargo test -p bb_core task_graph::executor --lib --manifest-path bb_backend/Cargo.toml`：通过，2 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，145 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/executor.rs bb_backend/crates/bb_core/src/task_graph/coordinator.rs bb_backend/crates/bb_core/src/task_graph/interpreter.rs bb_backend/crates/bb_core/src/task_graph/outcome.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅有 CRLF warning。

## 下一步

- 继续 060：对齐 LangGraph checkpoint recovery，重点是 pending writes replay、skip-done-tasks、checkpoint tuple / parent / namespace。
- 继续补 interruptBefore/interruptAfter 的 versions_seen / __interrupt__ 精确语义。
- 继续补 StateGraph Annotation/Zod schema -> channel factory，以及 Command graph/subgraph/PARENT 的完整语义。

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/executor.rs
- bb_backend/crates/bb_core/src/task_graph/coordinator.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter.rs
- bb_backend/crates/bb_core/src/task_graph/outcome.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
