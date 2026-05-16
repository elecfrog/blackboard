# Codex remove agent profile timestamp placeholders

时间: 2026-05-16T13:16:21Z
来源: codex
项目: blackboard

## 做了什么

- Removed the hard-coded created/updated footer from AgentProfileSection.
- Removed unused agentProfileCreated/agentProfileUpdated i18n keys.
- Removed unused .aw-profile-footer CSS.

## 验证了什么

- GET /api/projects: passed, confirmed blackboard project is available.
- npm run build --prefix bb_web: passed, with existing large chunk warnings.
- Browser DOM check on http://localhost:8060/#/projects/blackboard/agents: profile no longer contains ???/??? placeholders; runtime, MCP ???, and ?? sections remain visible.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/agents/AgentProfileSection.vue
- bb_web/src/i18n.ts
- bb_web/src/styles.css
