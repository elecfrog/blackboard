# 内置 Agent Chat 支持模型与变体设置

时间: 2026-05-14T08:14:12Z
来源: codex
项目: blackboard

## 做了什么

- 给 AgentChatFab 增加 OpenCode 入口/Profile、模型、变体设置区，并从项目 Agent registry 加载 OpenCode profiles 默认值。
- 扩展 direct AgentSession 创建 API，支持 model/variant 请求体字段，session 持久化 variant，OpenCode runtime 启动时传递 --model/--variant。
- 补充前端友好网络错误提示，避免直接暴露 Failed to fetch。
- 补充 agent_session_create_rejects_blank_prompt HTTP 测试。

## 验证了什么

- bb_list_projects/find_work_context: passed，定位到 blackboard ticket 000052。
- bb_list_agents: passed，确认 codex 为 active assignable agent 后开始 ticket work。
- cargo check: passed。
- cargo test -p bb_cli agent_session_create_rejects_blank_prompt: passed。
- npm run build --prefix bb_web: passed（仅 Vite 既有大 chunk warning）。
- 8060 代理 POST /api/projects/devkit/agent-sessions 带 variant=high: passed，session.variant=high，OpenCode 输出 VARIANT OK。

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/AgentChatFab.vue
- bb_web/src/data/agentSessions.ts
- bb_web/src/i18n.ts
- bb_web/src/styles.css
- bb_backend/crates/bb_cli/src/http/agent_sessions.rs
- bb_backend/crates/bb_core/src/agent_session/runtime.rs
- bb_backend/crates/bb_core/src/agent_session/model.rs
- bb_backend/crates/bb_core/src/agent_session/store.rs
