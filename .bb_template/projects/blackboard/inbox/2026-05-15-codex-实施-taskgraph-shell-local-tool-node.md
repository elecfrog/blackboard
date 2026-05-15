# 000063 Shell Node 实施交接

时间: 2026-05-15T02:28:51Z
来源: Codex
项目: blackboard

## 做了什么

- 新增 TaskGraph `shell` 节点类型与 `ShellConfig`/permission/capture/expected_exit_codes 配置模型。
- 接入后端默认 pins、节点调度 dispatch、node_exec runtime dispatch、结构校验和 Shell executor。
- Shell executor 使用 `std::process::Command` 直跑 command+args，支持 cwd 约束、timeout、cancel 检测、权限拒绝、stdout/stderr 捕获、log streaming、JSON artifact 和 node output。
- 补充后端测试：config 默认值、raw command 校验、shell pins、git status artifact、cwd escape、expected nonzero exit code、permission_denied、timeout。
- 补充前端类型、默认 pins、palette、默认 shell 节点、inspector 配置表单、preview 颜色和本地校验；并为并发 schedule 面板修正 `selectedRef` 类型以恢复前端 build。

## 验证了什么

- bb_list_projects：通过，确认 project 为 blackboard。
- bb_read_ticket_by_id 000063：通过，读取 ticket 上下文。
- bb_begin_ticket_work 000063 with agent=Codex：失败，后端返回 assignee `Codex` 不是 blackboard project active registered agent。
- bb_begin_ticket_work 000063 without agent：通过，ticket status 更新为 in_progress，maintenance consistency passed，export completed。
- cargo test -p bb_core task_graph --manifest-path bb_backend\\Cargo.toml：通过，最终 120 passed。
- cargo test --manifest-path bb_backend\\Cargo.toml：通过，bb_cli 75 passed，bb_core 251 passed，doc-tests 0 passed。
- npm run build --prefix bb_web：首次失败于并发 schedule 面板类型/i18n 缺口；修正当前剩余 `selectedRef` 类型后重跑通过，Vite 仅提示 chunk size warning。
- cargo fmt --all --check --manifest-path bb_backend/Cargo.toml：失败但仅报告并发/既有文件 bb_cli/src/http/task_graph/mod.rs 与 bb_core/src/task_graph/schedules.rs 的格式差异；063 相关 Rust diff 已通过 git diff --check。
- git diff --check -- <063 touched files>：通过，仅有 LF/CRLF warning，无 whitespace error。

## 下一步

- 后续如要提交，需只 stage 063 相关 hunk；当前 worktree 同时包含 000059/000064 schedule/canvas 等并发改动，不能整文件盲 stage。
- 可后续把 Shell artifact 拆成 stdout/stderr/metadata 多 artifact，或在 060 superstep 落地后迁移到 channel/artifact contract。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/types.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/shell_node.rs
- bb_backend/crates/bb_core/src/task_graph/validation/config.rs
- bb_backend/crates/bb_core/src/task_graph/interpreter_tests/mod.rs
- bb_web/src/data/taskGraphs.ts
- bb_web/src/components/task-graph/TaskGraphNodeInspector.vue
- bb_web/src/components/TaskGraphEditorPanel.vue
- bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue
