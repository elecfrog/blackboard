+++
id = "000036"
lane = "bbd"
title = "Task Graph 前端：Editor、节点面板与控制流配置"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
deliverable = "bb_web/src/components/TaskGraphEditorPanel.vue; bb_web/src/data/taskGraphs.ts; bb_web/src/components/GraphCanvas.vue"
depends_on = "000034 000035"
kind = "frontend"
parent = "000028"
+++

# 当前进展

- 交付目标：实现 Task Graph Editor 的 MVP 编辑闭环，支持拖入节点、连接控制流边、配置节点、保存 project graph，并清晰表达 Branch 与 Loop。
- 节点范围：Start、End、LLM、Registered Task、Human Gate、Branch、Loop。
- Inspector 范围：LLM 配 runtime/agent/model/prompt；Registered Task 选 tasks.toml task；Branch 配 first_match rules；Loop 配 max_iterations 和 body 边界；Human Gate 配提示文本。
- 控制流 UI：Branch 出口需要显示 label/condition；Loop 必须显式显示 body 与 retry/exit 路径，避免用户画出不可解释任意环。
- 保存规则：system graph 只读，只有 project graph 可保存；保存前前端做基本校验，最终以后端校验为准。

- 已新增 TaskGraphEditorPanel.vue，接入 035 的 project graph edit route，支持 node palette 点击/拖入画布、GraphCanvas 节点拖拽、连线、删边、节点/边选择。
- 已实现节点 inspector：LLM runtime/agent/model/prompt、Registered Task task_id/override、Human Gate title/instructions、Branch rules/default_rule_id、Loop max_iterations/body/exit/return 相关字段、End result。
- 已在 taskGraphs.ts 补 saveProjectTaskGraph、loadRegisteredTaskCatalog 与 validateTaskGraph；真实 API 可用时 PATCH，未实现时保存到 project localStorage mock。
- 已实现前端控制流校验：start/end、edge from/to、branch rule outgoing、loop max/body/exit/return、registered task catalog、LLM runtime 与 loop return cycle。
- 已在 editor 提供 MVP 示例插入，可生成 LLM -> Registered Task -> Branch -> Loop/Human Gate -> End 的完整样例。
- 已扩展 GraphCanvas edgeSelect 事件，供 editor 选择并配置控制流边。

- Task Graph cleanup: 完成 000036 实现：新增 `bb_web/src/components/TaskGraphEditorPanel.vue` 并接入 035 的 project graph edit route。
- Task Graph cleanup: Editor 支持 node palette 点击/拖入画布、GraphCanvas 节点拖拽、连线、删边、节点选择和边选择。
- Task Graph cleanup: Inspector 覆盖 LLM、Registered Task、Human Gate、Branch、Loop、End 的 MVP 配置字段。
- Task Graph cleanup: `bb_web/src/data/taskGraphs.ts` 新增 task catalog fallback、project graph save fallback 和前端控制流校验。

# 记录

- 本单依赖 Canvas 和 Catalog，可在后端 API 未完成时先使用 mock save。
- 验收标准：能从空白 project graph 创建完整 MVP 示例；非法 loop 缺少 max_iterations 时给出 UI 错误；保存后重新打开布局和配置不丢失。
- 验证要求：npm run build --prefix bb_web 通过；browser-use 操作创建 LLM->Branch->Loop->End 的 graph 并保存/重开。

- 验证：npm run build --prefix bb_web 通过。
- 验证受限：browser-use 对 http://127.0.0.1:8060/ 访问被浏览器安全策略阻断，未能执行创建 LLM->Branch->Loop->End 并保存/重开的浏览器 smoke。
- 维护受限：bb stdio 二进制读取既有中文 inbox 摘要时触发 UTF-8 byte boundary panic，本次 ticket 回写改用文件补丁，并通过脚本门禁复核。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-editor.md。

# 下一步

- 实现 node palette 与 drag-to-canvas。
- 实现 selected node inspector 表单。
- 实现 branch edge label/condition 编辑。
- 实现 loop controller 的可视化与校验提示。

- 后续 037 可基于 TaskGraph Catalog / Editor 的 run 入口接入 run UI、logs、artifact 与 human gate 占位。
- 后端 031 API 到位后，当前 saveProjectTaskGraph 会优先使用真实 PATCH 保存。
