# Codex remove agent org relationship model

时间: 2026-05-16T12:34:24Z
来源: codex
项目: blackboard

## 做了什么

- Removed the Agent profile organisation section and org-role tag from the web UI.
- Removed org_role/coordinator/workers from the web AgentProfile type and stopped normalizing those fields on writes.
- Removed agent org relationship fields from the Rust AgentProfile model, upsert cleanup, registry validation, tests, and MCP upsert_agent schema.
- Removed org relationship i18n keys and unused profile CSS for org chips / meta stats.
- Removed org_role entries from .bb_template/agents/agents.toml while preserving spec-coordinator as an agent id/name.

## 验证了什么

- GET /api/projects: passed, confirmed blackboard project is available.
- npm run build --prefix bb_web: passed, with existing large chunk warnings.
- cargo test -p bb_core agents_registry: passed, 14 tests.
- cargo test -p bb_cli stdio::tests::lists_tools_and_calls_inbox_tools_with_project_routing -- --test-threads=1: passed.
- cargo test -p bb_cli stdio::tests::creates_note_and_rejects_traversal_over_json_rpc -- --test-threads=1: passed.
- cargo test -p bb_cli stdio: failed on pre-existing all_listed_tools_return_standard_content_results fixture issue: sync_agent_connector expects agents/CODEX.md, then ENV_LOCK poison caused two follow-on failures.
- Browser DOM check on http://localhost:8060/#/projects/blackboard/agents: profile has no organisation section, no org empty state, and no worker/coordinator tag; MCP/?? sections remain visible.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/agents/AgentProfileSection.vue
- bb_web/src/components/AgentWorkbench.vue
- bb_web/src/data/agents.ts
- bb_web/src/i18n.ts
- bb_web/src/styles.css
- bb_backend/crates/bb_core/src/agents_registry/model.rs
- bb_backend/crates/bb_core/src/agents_registry/mod.rs
- bb_backend/crates/bb_core/src/agents_registry/validation.rs
- bb_backend/crates/bb_core/src/agents_registry/tests/mod.rs
- bb_backend/crates/bb_cli/src/mcp_tools/schema.rs
- .bb_template/agents/agents.toml
