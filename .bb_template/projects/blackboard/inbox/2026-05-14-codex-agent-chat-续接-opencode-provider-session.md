# Agent Chat 续接 OpenCode provider session

时间: 2026-05-14T08:26:12Z
来源: codex
项目: blackboard

## 做了什么

- 确认用户观察正确：每轮 Blackboard AgentSession 的 as-* id 会变化。
- 新增 direct chat 请求字段 provider_session_id，并让 OpenCode runtime 用 opencode run --session 续接 provider session。
- FAB 在收到 OpenCode status event 的 session_id 后保存当前 provider session，并在下一轮发送时复用；新对话会清空该 provider session。
- FAB subtitle 和消息 meta 区分 OpenCode provider session 与每轮 Blackboard AgentSession run id。

## 验证了什么

- bb_find_work_context: passed，继续使用 blackboard ticket 000052。
- bb_begin_ticket_work: passed，ticket 回到 in_progress 并记录本轮修正。
- cargo check: passed。
- npm run build --prefix bb_web: passed（仅既有 Vite 大 chunk warning）。
- cargo test -p bb_cli agent_session_create_rejects_blank_prompt: passed。
- 8060 代理真实两轮 OpenCode 对话: passed；两轮 as-* 不同，但 provider session 相同，第二轮正确回答上一轮暗号 BBCHAT42。

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/AgentChatFab.vue
- bb_web/src/data/agentSessions.ts
- bb_web/src/i18n.ts
- bb_backend/crates/bb_cli/src/http/agent_sessions.rs
- bb_backend/crates/bb_core/src/agent_session/runtime.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/llm_node.rs
