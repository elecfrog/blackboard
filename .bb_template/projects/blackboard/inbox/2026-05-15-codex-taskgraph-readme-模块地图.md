# 补充 TaskGraph 模块地图 README

时间: 2026-05-15T09:04:07Z
来源: codex
项目: blackboard

## 做了什么

- 新增 `bb_backend/crates/bb_core/src/task_graph/README.md`，用中文说明 TaskGraph 主执行链路、顶层模块职责、Pregel 子模块、run_state 子模块、node_exec 子模块和新能力落点。
- README 明确 `pregel/` 是 LangGraph/Pregel parity 的 owner，`coordinator.rs` 只保留 orchestration glue，避免后续继续长成大文件。
- README 增加常用验证命令，方便后续修改者按范围选择测试。

## 验证了什么

- `rg -n "[ \\t]+$" bb_backend/crates/bb_core/src/task_graph/README.md`：无尾随空白。
- `git diff --check -- bb_backend/crates/bb_core/src/task_graph/README.md`：通过。
- 本次只新增文档，未改 Rust/前端源码，未跑 cargo/npm 构建。

## 下一步

- 后续如果继续拆 `coordinator.rs`，同步更新 README 的模块职责和新能力落点。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/README.md
