# Removed fake online status from Agents page

时间: 2026-05-16T14:43:34Z
来源: Codex
项目: blackboard

## 做了什么

- Confirmed the Agents page online/idle/offline display was derived from static agent registry `status` values, not live runtime heartbeat or session presence.
- Removed the profile header online status pill from the Agents page.
- Removed the left agent list status light while preserving assignment count badges.
- Removed the now-unused agent status label mapping and i18n copy for online/idle/offline.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- Browser verification at `http://localhost:8060/#/projects/blackboard/agents`: `.aw-profile-actions .aw-meta-pill` count = 0, `.aw-list-items .aw-status-dot` count = 0, visible `在线` text = false; assignment count metadata still renders when present.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_web\src\components\agents\AgentProfileSection.vue
- D:\Dev\blackboard\bb_web\src\components\agents\AgentListPanel.vue
- D:\Dev\blackboard\bb_web\src\i18n.ts
- D:\Dev\blackboard\bb_web\src\styles.css
