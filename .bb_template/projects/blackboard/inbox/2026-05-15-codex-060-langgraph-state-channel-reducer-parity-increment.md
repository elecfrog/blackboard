# 060 继续对齐 LangGraph：State channels 与 reducer channel class MVP

时间: 2026-05-15T07:00:22Z
来源: codex
项目: blackboard

## 做了什么

- 继续按 060 的 LangGraph compile + Pregel 对齐目标推进，没有收口 060。
- 在 compile 层新增/完善 state channel 编译：graph inputs 现在会生成 `state:<input_id>` channels，`InputVar` 节点读取对应 state channel。
- 扩展 compiled channel class：加入 `AnyValue` 与 `BinaryOperatorAggregate`，并导出 `CompiledReducer`。
- 在 Pregel initial checkpoint 中把对象输入投影到已编译的 state channels，使 state 进入 `channel_values/channel_versions`。
- 在 Pregel apply writes 中补 `AnyValue` last-write-wins 与 `BinaryOperatorAggregate` append / object merge / sum reducer MVP。
- 更新 mapping 文档，把本轮对齐状态写入 L0/L3/L7、Current Blackboard State 和 Required Alignment Tasks。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，10 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，141 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check -- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md bb_backend/crates/bb_core/src/task_graph/compiler.rs bb_backend/crates/bb_core/src/task_graph/pregel.rs bb_backend/crates/bb_core/src/task_graph/mod.rs`：通过；仅提示 `mod.rs` CRLF warning。

## 下一步

- 继续 060，不切到完成态：下一步对齐 LangGraph `StateGraph.attachNode` 的 state write mapper，让业务节点返回对象后能按 state key 写入对应 channel。
- 继续补 `Command/goto/Send` writer 入 IR，与现有 `__pregel_tasks` PUSH task 准备逻辑接起来。
- 随后补 interruptBefore/interruptAfter 的 versions_seen 语义，以及 checkpoint tuple / pending writes recovery。

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
