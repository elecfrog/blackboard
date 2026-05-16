# Agents page skill config editor

时间: 2026-05-16T13:48:35Z
来源: Codex
项目: blackboard

## 做了什么

- Added backend `GET /api/agents/skills` so the Agents page can list registered skills from `<bb_root>/skills/*` instead of hard-coded UI data.
- Implemented Skill 配置列表 add/delete editing in `bb_web`: bottom-right `新增 Skill` text button, registered-skill dropdown, immediate save on selection, and right-side delete action per row.
- Removed the old free-text skills editor from the agent profile edit form to keep agent skills constrained to registered skill directories.
- Restarted the local dev stack so the new backend route is available to the running browser session.

## 验证了什么

- `npm run build --prefix bb_web`: passed; Vite reported only existing large chunk warnings.
- `cargo test -p bb_cli agent_skills_list_returns_registered_skill_dirs` from `bb_backend`: passed.
- `cargo test -p bb_core skills` from `bb_backend`: passed.
- `GET http://localhost:8060/api/agents/skills`: 200, returned registered `triage` skill with description/path.
- Browser check on Agents page: BB PM shows `triage` with a right-side delete button and disabled add button because all registered skills are already selected; Codex shows `新增 Skill`, opens a dropdown containing `triage`, and no save was triggered during verification to avoid mutating Codex config.

## 下一步

- （未填写）

## 相关位置

- D:\Dev\blackboard\bb_backend\crates\bb_cli\src\http\mod.rs
- D:\Dev\blackboard\bb_backend\crates\bb_cli\src\http\tests\mod.rs
- D:\Dev\blackboard\bb_web\src\data\agents.ts
- D:\Dev\blackboard\bb_web\src\components\AgentWorkbench.vue
- D:\Dev\blackboard\bb_web\src\components\agents\AgentSkillsTable.vue
- D:\Dev\blackboard\bb_web\src\components\agents\AgentProfileSection.vue
- D:\Dev\blackboard\bb_web\src\i18n.ts
- D:\Dev\blackboard\bb_web\src\styles.css
