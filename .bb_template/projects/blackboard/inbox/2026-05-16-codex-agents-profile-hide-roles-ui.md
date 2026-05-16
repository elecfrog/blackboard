# Codex hide agent roles from profile UI

时间: 2026-05-16T09:54:30Z
来源: codex
项目: blackboard

## 做了什么

- Removed AgentProfile roles tags from the profile header.
- Removed the Roles/?? count from the profile metadata strip.
- Kept roles in the registry/data model for compatibility; this is UI-only.

## 验证了什么

- npm run build --prefix bb_web: passed, with existing large chunk warnings.
- Browser DOM check on http://localhost:8060/#/projects/blackboard/agents: profile no longer contains ?? or known roles tags such as generalist/engineering/pm.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/agents/AgentProfileSection.vue
