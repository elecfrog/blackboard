# Agent Chat project cwd 修正

时间: 2026-05-14T10:00:58Z
来源: codex
项目: blackboard

## 做了什么

- 定位到 direct project chat 创建 AgentTurnRequest 时只传 workspace_root，OpenCode --dir 因此落在 Blackboard data root，Devkit chat 会读到 bb 仓库状态。
- 为 AgentTurnRequest 增加 execution_root，运行 OpenCode/Codex/CodeBuddy 时用 execution_root 作为 cwd/--dir/-C，workspace_root 继续只用于 session/event 存储。
- Direct chat 现在从项目 __project__.json 的 repos 优先解析实际代码根；无 repos 时支持 .bb capsule 回退父目录和 .bb_template 回退源码根。
- Direct chat prompt 增加 Working directory，运行环境额外注入 BB_WORKSPACE_ROOT 与 BB_PROJECT_ROOT 便于诊断。

## 验证了什么

- bb_list_projects/list_projects: passed，确认可见 project 包含 blackboard 与 devkit，devkit repos 指向 C:\Dev\Devkit。
- bb_find_work_context: passed，定位 blackboard 活跃 ticket 000052；bb_search_tickets: passed，无特定 path bug 既有命中。
- cargo test -p bb_cli direct_chat_execution_root: passed，3 个路径解析单测通过。
- cargo test -p bb_core -p bb_cli: passed，bb_cli 74 tests 与 bb_core 237 tests 全部通过。
- python scripts\check_ticket_ids.py --project blackboard: passed。

## 下一步

- 重启/刷新后端后，在 Devkit project 的 OpenCode Chat 里重新询问本地修改，预期显示 C:\Dev\Devkit 的 git 状态而不是 C:\Dev\blackboard。

## 相关位置

- bb_backend/crates/bb_cli/src/http/agent_sessions.rs
- bb_backend/crates/bb_core/src/agent_session/runtime.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/llm_node.rs
