# 060 增量推进：Compile channel class 与 State channel 语义

时间: 2026-05-15T06:45:22Z
来源: codex
项目: blackboard

## 做了什么

- 按用户要求把 `taskgraph-langgraph-compile-pregel-mapping.md` 改成阶段验收表，明确 L0-L8 parity 状态，避免把骨架对齐误报成全量完成。
- 在 `CompiledChannel` 增加 LangGraph-style `CompiledChannelClass`：`EphemeralValue { guard }`、`LastValue`、`Topic { unique, accumulate }`、`NamedBarrierValue`。
- Compile 产物现在区分 channel class：`__start__/__end__` 为 guarded `EphemeralValue`，`branch:to:*` 为 unguarded `EphemeralValue(false)`，join 为 `NamedBarrierValue`，data/node output 为 `LastValue`。
- Pregel `apply_writes` 从按 Blackboard kind 更新，推进到按 channel class 更新：LastValue 同 step 多写报错，Topic 支持 PubSub 展平，NamedBarrierValue 继续做 sender readiness，EphemeralValue 支持 guard。
- 新增 Pregel 单测覆盖 LastValue 多写错误与 Topic checkpoint value 行为。

## 验证了什么

- `cargo fmt --all --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_core task_graph::compiler --lib --manifest-path bb_backend/Cargo.toml`：通过，1 passed。
- `cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`：通过，6 passed。
- `cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`：通过，137 passed。
- `cargo check -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过。
- `cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml`：通过，80 passed。
- `git diff --check -- <本轮触达文件>`：通过；仅 Windows LF/CRLF 提示。

## 下一步

- 继续保持 000060 in_progress，不视为完成。下一小步建议补 StateGraph Annotation/reducer -> channel factory 的 Blackboard 对齐入口，或先补 interruptBefore/interruptAfter 的 LangGraph 版本判断。

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-langgraph-compile-pregel-mapping.md
- bb_backend/crates/bb_core/src/task_graph/compiler.rs
- bb_backend/crates/bb_core/src/task_graph/pregel.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
