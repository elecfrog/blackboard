# 062 node taxonomy 与 runtime binding registry 交接

时间: 2026-05-15T04:42:03Z
来源: codex
项目: blackboard

## 做了什么

- 新增 node_registry 模块，定义 NodeCategory、NodeRole、RuntimeBinding、PermissionSpec、ArtifactOutputSpec 和 NodeSpec。
- 内置 SWE 第一阶段业务角色：explorer_agent、implementer_agent、verifier_agent、reviewer_agent、handoff_writer、opencode_session、codex_session、local_shell、write_wiki_doc、update_ticket。
- registry 采用 role-on-existing-node 设计：业务节点复用 llm/shell/sub_graph 执行器，避免破坏现有图 schema。
- 前端 palette 新增 Explorer/Implementer/Verifier/Reviewer/Handoff/Local Shell 角色入口，创建节点时写入 config.role，并按角色展示色彩和元信息。

## 验证了什么

- cargo test -p bb_core task_graph::node_registry --lib：通过，3 passed。
- cargo test -p bb_core task_graph --lib：通过，128 passed。
- cargo check -p bb_cli：通过。
- npm run build --prefix bb_web：通过，仅保留既有 chunk size warning。

## 下一步

- 把 registry 的 permissions/runtime binding 接入 pre-run validation 和 runtime dispatch 门禁；再实现 write_wiki_doc/update_ticket 这类 artifact node 的真实 executor。

## 相关位置

- bb_backend/crates/bb_core/src/task_graph/node_registry.rs
- bb_backend/crates/bb_core/src/task_graph/mod.rs
- bb_web/src/components/task-graph/taskGraphNodeVisuals.ts
- bb_web/src/components/task-graph/TaskGraphNodePalette.vue
- bb_web/src/components/TaskGraphEditorPanel.vue
