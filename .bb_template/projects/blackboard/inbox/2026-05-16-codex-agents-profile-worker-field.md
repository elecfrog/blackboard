# Codex agent profile worker label and guard

时间: 2026-05-16T08:46:10Z
来源: codex
项目: blackboard

## 做了什么

- Renamed the coordinator worker metric from ?? to ?? Worker / Worker agents.
- Hid the worker-count metric unless the agent is org_role=coordinator.
- Normalized frontend agent writes so non-coordinator payloads omit workers and non-worker payloads omit coordinator.
- Tightened backend registry validation so only coordinators can declare workers, with regression coverage.

## 验证了什么

- npm run build --prefix bb_web: passed, with existing large chunk warnings.
- cargo test -p bb_core agents_registry from bb_backend: passed, 19 tests.
- Browser DOM check on http://localhost:8060/#/projects/blackboard/agents: selected worker agent no longer shows ??/worker-count metric.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/i18n.ts
- bb_web/src/components/agents/AgentProfileSection.vue
- bb_web/src/data/agents.ts
- bb_backend/crates/bb_core/src/agents_registry/validation.rs
- bb_backend/crates/bb_core/src/agents_registry/tests/mod.rs
