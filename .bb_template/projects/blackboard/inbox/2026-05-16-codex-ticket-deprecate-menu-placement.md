# Codex handoff: ticket deprecate menu placement

时间: 2026-05-16T14:32:42Z
来源: codex
项目: blackboard

## 做了什么

- Moved the deprecate/delete affordance out of the main TicketDetailPanel header actions.
- Graph is now followed by a hamburger icon button; its dropdown contains the dangerous delete action, which still opens the in-app confirm dialog before moving the ticket to tickets/_deprecated.

## 验证了什么

- npm run build --prefix bb_web: passed.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/TicketDetailPanel.vue
- bb_web/src/i18n.ts
- bb_web/src/styles.css
