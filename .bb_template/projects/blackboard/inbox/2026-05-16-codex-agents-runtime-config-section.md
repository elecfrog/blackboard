# Agents page runtime config section

时间: 2026-05-16T14:17:17Z
来源: Codex
项目: blackboard

## 做了什么

- Moved runtime/model/variant out of the profile card into a dedicated `运行配置` section that is a sibling of MCP 服务器 and Skill 配置列表.
- Added `AgentRuntimeConfig.vue` with three floating text boxes and an independent inline edit/save/cancel flow using `upsertAgent`.
- Removed the old profile-card meta block and its stale CSS selectors; added i18n labels/placeholders for the new runtime config section.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- Browser verification at `http://localhost:8060/#/projects/blackboard/agents`: profile card no longer contains runtime/model/variant; `运行配置` appears after profile and before MCP/Skill sections; runtime/model/variant render as three fields.
- Browser edit check: the `运行配置` inline edit button opens three inputs with the current BB PM values, and cancel exits without saving.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_web\src\components\agents\AgentRuntimeConfig.vue
- D:\Dev\blackboard\bb_web\src\components\agents\AgentProfileSection.vue
- D:\Dev\blackboard\bb_web\src\components\AgentWorkbench.vue
- D:\Dev\blackboard\bb_web\src\i18n.ts
- D:\Dev\blackboard\bb_web\src\styles.css
