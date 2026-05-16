# Codex remove duplicate skill and MCP profile stats

时间: 2026-05-16T12:26:31Z
来源: codex
项目: blackboard

## 做了什么

- Removed the top profile meta strip entries for Skills and MCP because dedicated sections already render those details below.
- Kept the coordinator-only worker stat logic unchanged.

## 验证了什么

- GET /api/projects: passed, confirmed blackboard project is available.
- npm run build --prefix bb_web: passed, with existing large chunk warnings.
- Browser DOM check on http://localhost:8060/#/projects/blackboard/agents: top profile area has no Skills/MCP stats; lower MCP ??? and ?? sections remain visible.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/agents/AgentProfileSection.vue
