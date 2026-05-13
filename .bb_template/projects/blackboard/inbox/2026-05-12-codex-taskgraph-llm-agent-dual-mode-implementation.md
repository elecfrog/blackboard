# Handoff: TaskGraph LLM/Agent dual mode

时间: 2026-05-12T18:18:30Z
来源: codex
项目: blackboard

## 做了什么

- Added TaskGraph LLM node run_as resolver for llm vs agent modes; Agent mode resolves Agent Profile runtime/model/skills/MCP/custom env/args and reads prompt from instructions_path.
- Wired OpenCode AgentSession and legacy Codex runtime to resolved invocation data, including node/profile skills, MCP config, custom env, and custom args.
- Updated TaskGraph editor to expose LLM / Agent mode forms; Agent mode selects project agents, LLM mode edits runtime/model/provider agent/inline prompt/skills/MCP/advanced env args.
- Fixed bb-pm profile model to opencode-available minimax-cn-coding-plan/MiniMax-M2.7-highspeed.
- Created and ran project smoke graph agent-profile-mode-smoke-20260513-020952; successful run run-20260512-181102-7ab4fb0f persisted AgentSession as-20260512-181102-5f43d484 with 28 events, 6 tool events, and usage.

## 验证了什么

- bb_list_projects: passed; bb_find_work_context: passed; bb_list_agents(project=blackboard): passed.
- cargo test --manifest-path bb_backend/Cargo.toml -p bb_core task_graph: passed (91 tests).
- cargo test --manifest-path bb_backend/Cargo.toml -p bb_core agent_session: passed (1 test).
- cargo test --manifest-path bb_backend/Cargo.toml -p bb_cli task_graph: passed (0 matching tests, compile passed).
- npm run build --prefix bb_web: passed.
- REST smoke: GET agent session/events passed; session as-20260512-181102-5f43d484 has event types status/text/tool_use/tool_result/usage_update.

## 下一步

- （未填写）

## 相关位置

- C:\Dev\blackboard\bb_backend\crates\bb_core\src\task_graph\llm.rs
- C:\Dev\blackboard\bb_backend\crates\bb_core\src\task_graph\node_exec\runtime_nodes\llm_node.rs
- C:\Dev\blackboard\bb_web\src\components\TaskGraphEditorPanel.vue
- C:\Dev\blackboard\.bb_template\runtime\agent_sessions\blackboard\as-20260512-181102-5f43d484
