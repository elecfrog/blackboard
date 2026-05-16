# Codex handoff: ticket deprecated archive entry

时间: 2026-05-16T14:26:13Z
来源: codex
项目: blackboard

## 做了什么

- Added structured ticket deprecation flow: core moves active ticket Markdown into tickets/_deprecated and rebuilds active ticket index.
- Deprecation cleanup removes active depends_on/dependencies references and ticket-kind attachment references pointing at the deprecated ticket.
- Added REST endpoint POST /api/projects/{project}/tickets/{id}/deprecate and MCP tool schema/handler deprecate_ticket.
- Added web detail-panel action to confirm and move a ticket into tickets/_deprecated, then refresh active board/summary.

## 验证了什么

- cargo test -p bb_core deprecate_ticket_moves_file_and_resolves_active_relationships: passed before later unrelated TaskGraph compile break surfaced.
- cargo test -p bb_cli rest_deprecate_ticket_moves_file_and_resolves_relationships: passed before later unrelated TaskGraph compile break surfaced.
- npm run build --prefix bb_web: passed.
- Later Rust reruns are currently blocked by existing TaskGraph scripts_dir migration compile errors in bb_core task_graph files, not by the deprecate-ticket files.

## 下一步

- After the TaskGraph scripts_dir migration is completed, rerun bb_core/bb_cli test suite including stdio MCP tool contract tests.

## 相关位置

- bb_backend/crates/bb_core/src/ticket/mod.rs
- bb_backend/crates/bb_core/src/ticket/index.rs
- bb_backend/crates/bb_cli/src/http/tickets.rs
- bb_backend/crates/bb_cli/src/mcp_tools/mod.rs
- bb_web/src/components/TicketDetailPanel.vue
- bb_web/src/views/BoardView.vue
- bb_web/src/data/tickets.ts
