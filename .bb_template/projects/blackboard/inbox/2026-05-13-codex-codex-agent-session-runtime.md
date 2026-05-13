# Inbox Note

时间: 2026-05-13T07:11:25Z
来源: codex
项目: blackboard

## 做了什么

- Codex TaskGraph LLM/Agent nodes now execute through AgentSession and persist structured events.
- Codex JSONL parser handles current thread/turn/item schema plus older event_msg/response_item schema.
- TaskGraph runtime list removes codex-interactive; Codex execution uses codex exec only.
- LLM/Agent variant maps to Codex model_reasoning_effort config.

## 验证了什么

- cargo test --manifest-path bb_backend/Cargo.toml agent_session passed.
- cargo test --manifest-path bb_backend/Cargo.toml task_graph passed.
- npm run build --prefix bb_web passed.
- Real Codex TaskGraph smoke with gpt-5.4-mini + low persisted status/text/usage_update events.

## 下一步

- （未填写）

## 相关位置

- C:\Dev\blackboard\bb_backend\crates\bb_core\src\agent_session\providers\codex.rs
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\agent_session\runtime.rs
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\task_graph\llm.rs
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\task_graph\node_exec\runtime_nodes\llm_node.rs
- C:\Dev\blackboard\bb_web\src\components\task-graph\TaskGraphLlmNodeForm.vue
