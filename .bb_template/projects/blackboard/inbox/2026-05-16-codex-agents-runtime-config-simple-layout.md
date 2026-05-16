# Agents page runtime config simplified layout

时间: 2026-05-16T14:19:34Z
来源: Codex
项目: blackboard

## 做了什么

- Simplified the Agents page runtime config component so Runtime/Model/Variant render as plain label/value text instead of bordered floating boxes.
- Kept the runtime config section as a sibling of MCP 服务器 and Skill 配置列表, and kept the existing inline edit/save/cancel behavior for configuration changes.
- Added dedicated Runtime/Model/Variant labels for the runtime config component so the visible labels match the requested concise wording.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- Browser verification at `http://localhost:8060/#/projects/blackboard/agents`: runtime fields render as `Runtime value`, `Model value`, `Variant value`; computed styles confirm transparent background, `0px` border, no box shadow, and no padding on the field containers.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_web\src\components\agents\AgentRuntimeConfig.vue
- D:\Dev\blackboard\bb_web\src\i18n.ts
- D:\Dev\blackboard\bb_web\src\styles.css
