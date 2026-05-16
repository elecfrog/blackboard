# Agents profile summary no-card layout

时间: 2026-05-16T14:30:31Z
来源: Codex
项目: blackboard

## 做了什么

- Removed card treatment from the Agents profile header area: renamed the wrapper from `aw-profile-card` to `aw-profile-summary` and removed border, radius, surface background, clipping, and card-like spacing.
- Kept the profile content itself unchanged: avatar, display name, description, status, and inline edit action remain in the same top summary position.
- Updated mobile profile header padding so the summary stays unframed across responsive layouts.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- Browser verification at `http://localhost:8060/#/projects/blackboard/agents`: `.aw-profile-card` no longer exists; `.aw-profile-summary` is present with transparent background, `0px` border, `0px` radius, and visible overflow.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_web\src\components\agents\AgentProfileSection.vue
- D:\Dev\blackboard\bb_web\src\styles.css
