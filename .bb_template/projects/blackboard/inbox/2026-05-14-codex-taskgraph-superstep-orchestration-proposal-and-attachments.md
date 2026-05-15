# TaskGraph superstep proposal, ticket attachments, and 060-062 rewrite

时间: 2026-05-15T00:00:00+08:00
来源: Codex
项目: blackboard

## 做了什么

- Read LangGraphJS distilled modules under C:\Dev\kb\projects\agentic\langgraphjs\modules, with focus on pregel/checkpoint/api/sdk/supervisor/swarm.
- Created wiki proposal at wiki/proposal/taskgraph-superstep-agent-orchestration.md.
- Added ticket attachments contract: HTTP tickets list exposes attachments; PATCH accepts structured attachments and persists them through frontmatter extra JSON.
- Added TicketDetailPanel attachments editor and BoardView optimistic save path.
- Rewrote tickets 000060/000061/000062 around Execution Kernel, Dataflow/Artifacts, and Business Node Taxonomy; each references the proposal attachment.

## 验证了什么

- tool_search for bb_* returned no callable tools, so ticket edits were applied to project files and index directly.
- python scripts/check_ticket_ids.py --project blackboard passed.
- cargo fmt --all passed from bb_backend.
- cargo test -p bb_cli passed: 75 tests.
- npm run build --prefix bb_web passed; Vite reported only existing chunk-size warnings.
- git diff --check passed with line-ending warnings only.
- In-app browser reload of ticket graph preview 000062 found title, attachments section, and proposal reference.

## 下一步

- After bb_* MCP tools are available, optionally re-apply/sync the 060/061/062 metadata through structured ticket tools so the tool audit trail is complete.
- Next implementation ticket should start from 000060 and build the superstep execution kernel minimal vertical slice.

## 相关位置

- .bb_template/projects/blackboard/wiki/proposal/taskgraph-superstep-agent-orchestration.md
- .bb_template/projects/blackboard/tickets/000060-taskgraph-execution-foundation.md
- .bb_template/projects/blackboard/tickets/000061-taskgraph-dataflow-type-system.md
- .bb_template/projects/blackboard/tickets/000062-taskgraph-node-taxonomy.md
- bb_backend/crates/bb_cli/src/http/tickets.rs
- bb_web/src/components/TicketDetailPanel.vue
