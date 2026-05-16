# Agents MCP and Skill empty states simplified

时间: 2026-05-16T14:36:51Z
来源: Codex
项目: blackboard

## 做了什么

- Changed the Agents page MCP empty state from an icon/card panel to a single inline text line.
- Changed the Skill configuration empty state the same way, keeping the add Skill action outside the empty text.
- Removed the now-unused `.aw-empty-panel` / icon / body styles and replaced them with a lightweight `.aw-empty-line` style.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- Browser verification at `http://localhost:8060/#/projects/blackboard/agents`: selected OpenCode, `.aw-empty-panel` count = 0, `.aw-empty-panel-icon` count = 0, MCP and Skill empty states render as transparent `<p.aw-empty-line>` text with `0px` border/radius.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_web\src\components\agents\AgentMcpTable.vue
- D:\Dev\blackboard\bb_web\src\components\agents\AgentSkillsTable.vue
- D:\Dev\blackboard\bb_web\src\styles.css
