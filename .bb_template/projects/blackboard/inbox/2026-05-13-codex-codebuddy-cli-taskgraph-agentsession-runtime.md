# CodeBuddy TaskGraph AgentSession handoff

时间: 2026-05-13T08:53:23Z
来源: codex
项目: blackboard

## 做了什么

- Added codebuddy as a TaskGraph AgentSession runtime alongside opencode/codex, including CLI path overrides and AgentTurnRequest fields.
- Implemented CodeBuddy stream-json provider normalization for status, text, thinking, tool_use, tool_result, usage_update, error, and log events.
- Generated session-local CodeBuddy MCP config/settings artifacts with bb MCP injection, reasoningEffort variant settings, Windows codebuddy.cmd resolution, and <bb-root> expansion.
- Updated TaskGraph LLM execution/tests and frontend runtime dropdown validation for codebuddy.
- Added and ran project smoke graph codebuddy-agent-session-smoke-20260513-162752; latest successful run run-20260513-084723-58409159 created AgentSession as-20260513-084724-33be1a1a with CODEBUDDY_GRAPH_OK.

## 验证了什么

- cargo test --manifest-path bb_backend/Cargo.toml agent_session: passed.
- cargo test --manifest-path bb_backend/Cargo.toml task_graph: passed (106 tests; existing unused-variable warning in validation/pre_run.rs).
- cargo test --manifest-path bb_backend/Cargo.toml codebuddy: passed (15 tests).
- npm run build --prefix bb_web: passed with existing Vite chunk-size warning.
- git diff --check on touched CodeBuddy/TaskGraph/frontend files: passed; only CRLF normalization warnings.
- Real CodeBuddy direct captures: pure text, PowerShell tool call, and bb MCP list_projects all succeeded before TaskGraph smoke.
- cargo build -p bb_cli default target was blocked by running bb.exe lock; rebuilt with --target-dir bb_backend/target-codebuddy-smoke and removed that temporary target after smoke.

## 下一步

- （未填写）

## 相关位置

- bb_backend/crates/bb_core/src/agent_session/providers/codebuddy.rs
- bb_backend/crates/bb_core/src/agent_session/runtime.rs
- bb_backend/crates/bb_core/src/task_graph/llm.rs
- bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/llm_node.rs
- bb_backend/crates/bb_cli/src/http/task_graph/interpreter_config.rs
- bb_web/src/data/taskGraphs.ts
- .bb_template/projects/blackboard/task_graphs/codebuddy-agent-session-smoke-20260513-162752.json
