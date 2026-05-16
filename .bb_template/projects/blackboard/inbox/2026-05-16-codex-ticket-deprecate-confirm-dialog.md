# Codex handoff: ticket deprecate confirm dialog

时间: 2026-05-16T14:29:17Z
来源: codex
项目: blackboard

## 做了什么

- Replaced the browser-native window.confirm in TicketDetailPanel with an in-app Blackboard-styled alertdialog for the deprecate-ticket action.
- The dialog shows the ticket id/title, explains the tickets/_deprecated move, supports cancel/confirm actions, and uses existing button/icon/color styling.

## 验证了什么

- Select-String confirmed TicketDetailPanel no longer calls window.confirm.
- npm run build --prefix bb_web: passed.

## 下一步

- （未填写）

## 相关位置

- bb_web/src/components/TicketDetailPanel.vue
- bb_web/src/i18n.ts
- bb_web/src/styles.css
