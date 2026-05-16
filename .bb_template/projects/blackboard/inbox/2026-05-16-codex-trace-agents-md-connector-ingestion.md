# AGENTS.md connector ingestion trace

时间: 2026-05-16T10:49:36Z
来源: codex
项目: blackboard

## 做了什么

- 按 Blackboard 协议调用 list_projects，确认可见 project 为 blackboard。
- 调用 find_work_context(project=blackboard, query=AGENTS.md Codex agent instructions_path 特化)，未找到直接相关 ticket 或 note。
- 调用 list_agents(project=blackboard)，确认当前 agent registry source_path 为 D:\Dev\blackboard\.bb_template\agents\agents.toml，bb_root 对应 D:\Dev\blackboard\.bb_template。
- 本地只读追踪 .bb_template/agents/AGENTS.md、agents_config、agents_registry、HTTP Settings endpoints、前端 AgentConnectorPanel/data 层、TaskGraph LLM runtime prompt/MCP 注入路径。

## 验证了什么

- bb_list_projects/mcp__bb__.list_projects: 通过，返回 blackboard project。
- bb_find_work_context/mcp__bb__.find_work_context: 通过，无直接 ticket/note 命中。
- bb_list_agents/mcp__bb__.list_agents: 通过，返回 blackboard project agents 和 registry source_path。
- 本地 rg/Get-Content 只读检查: 通过；未修改文件，未运行构建或测试。

## 下一步

- （未填写）

## 相关位置

- .bb_template/agents/AGENTS.md
- bb_backend/crates/bb_core/src/agents_config/mod.rs
- bb_backend/crates/bb_core/src/agents_config/model.rs
- bb_backend/crates/bb_core/src/agents_config/paths.rs
- bb_backend/crates/bb_core/src/agents_registry/mod.rs
- bb_backend/crates/bb_core/src/agents_registry/model.rs
- bb_backend/crates/bb_cli/src/http/mod.rs
- bb_web/src/data/agentConnectors.ts
- bb_web/src/components/AgentConnectorPanel.vue
- bb_backend/crates/bb_core/src/task_graph/nodes/llm.rs
