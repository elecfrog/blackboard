# Blackboard Codex 全局规则

## Codex 工具发现协议

- Codex 当前客户端可能不会在初始工具列表里直接暴露 Blackboard MCP 工具。
- Codex 默认延迟使用 Blackboard 工具：先完成用户当前明确指定的本地代码、文档、搜索或分析工作；完成后再做 Blackboard project/ticket/inbox 检索和交接。
- 如果当前轮次看不到 `bb_*` 或 `mcp__bb__` 工具，不要把它当成启动阻塞。等用户指定工作完成、需要写 handoff、需要读取历史上下文，或必须进行 ticket/lane 写入时，再用 `tool_search` 搜 `Blackboard bb list_projects search_tickets inbox notes`。
- 只有当用户明确给出 ticket ID、要求操作 ticket/lane、或任务本身必须依赖 Blackboard 历史上下文才能安全推进时，才在工作前加载并使用 Blackboard 工具。
- 在 Codex 里，Blackboard 工具可能显示为命名空间形式，例如 `mcp__bb__.list_projects`、`mcp__bb__.find_work_context`、`mcp__bb__.create_inbox_note`；这与 `bb_list_projects` 等裸 MCP 工具名是同一组能力的展示差异。
- 最终回答和 inbox 交接里记录实际调用名和结果，例如 `mcp__bb__.list_projects: 通过`，不要把“初始工具列表未显示”误报成 Blackboard 工具不存在。

## Blackboard 起止协议

- Blackboard 是 project 化的，**必须以 `bb_*` MCP 工具返回的数据和权限为准**。
- 工具名以当前客户端工具列表为准；通常显示为 `bb_*`。少数裸 MCP 客户端可能显示无前缀原始名，这是同一 Blackboard 工具的展示差异，不代表存在两套 API。
- Codex 可以在完成用户明确指定的工作后，再通过 `bb_list_projects` 获取可见 project 并写交接；如果用户给了 project 名、ticket ID 或关键词，且该上下文是安全推进任务的前置条件，再用 `bb_search_tickets`、`bb_list_tickets`、`bb_read_ticket_by_id` 检索上下文。除 `bb_list_projects` 外，每次调用都必须显式传 `project`。
- 如果 active ticket 检索不充分、有歧义，或者请求里出现很可能有历史上下文的关键词，再用 `bb_search_notes`、`bb_list_inbox_notes`、`bb_read_inbox_note` 搜对应 project 的 inbox。不要扫本地 `projects/*` 目录来替代 MCP 检索。
- Blackboard 是协作上下文事实。找到相关 ticket 时按其范围推进工作。
- 工作结束后，通过 `bb_create_inbox_note` 在对应 project 写一份简洁交接。
- inbox 笔记要简洁：只写已经完成的工作流水、实际验证、相关 ticket/tool/context。风险、决策、下一步和阻塞优先进 ticket，不把 handoff 写成小型 ticket。
- 仅当请求纯粹是对话式回答、或是不会产生任何长期 project 上下文的一次性小命令时，才可以跳过 inbox 笔记。

## Blackboard Ticket 工具门禁

- 对 ticket 或 lane 做任何变更时，必须使用 `bb_*` 结构化 MCP 工具（如 `bb_create_ticket`、`bb_update_ticket`、`bb_append_ticket_sections`、`bb_list_lanes`、`bb_upsert_lane`、`bb_archive_lane`）。不要手改 ticket Markdown，除非用户明确要求裸文件流程且确认当前 project 使用本地文件存储。
- Ticket ID、lane、status、frontmatter / extra、正文追加、索引维护和权限判断都由 `bb_*` 工具后端负责。Agent 不跨 project 猜号、不根据文件名推断权威状态、不直接读写 `__tickets__.json` 或 `__project__.json`。
- Ticket 可能没有本地文件路径，也可能对当前 Agent 只读。工具返回 blocked / forbidden / not_found / conflict 时，停止本项 ticket/lane 写入并把工具结果作为阻塞事实汇报，不要绕过权限改文件。
- 只有当任务本身是 Blackboard 本地后端/脚本/数据迁移开发，并且用户明确要求本地文件流程时，才把 `python "$BB_SCRIPTS_DIR/check_ticket_ids.py"`、`qmd embed` 作为本地数据门禁；云端或远端 ticket/lane 变更以 `bb_*` 工具响应为门禁。
- 如果改动涉及 Blackboard Web 前端源码（`bb_web/src` 下），还需追加 `npm run build --prefix bb_web` 验证前端构建。
- 最终回答以及 inbox 交接笔记里必须写清楚：实际调用了哪些 `bb_*` 工具或本地验证命令，每条的结果是通过、失败还是被阻塞。
- 必需的 `bb_*` 工具不可用时，不要宣称 ticket/inbox/lane 任务完成；在最终回答里记录缺失工具、已完成的代码工作、以及需要恢复工具后补跑的完整操作。

## 并发 Agent Worktree 协议

- 假定与当前任务无关、不在范围内、不属于你的 worktree 改动，可能属于用户或另一个并发 Agent。不要因为它们在当前 ticket 或 Open Spec 范围之外，就去删除、回退、移动、重命名、重新格式化或"顺手清理"它们。
- 范围边界是"编辑边界"，不是"清理授权"。不在范围内的文件或改动要保持原样，仅在必要时作为上下文或残留风险提一下。
- 如果范围外的改动与当前任务直接冲突，停下来询问用户如何处理，不要通过删除来解决冲突。
- 审查时要区分"范围外的既有/并发工作"和"当前改动中的缺陷"。除非用户明确要求，否则不要指示实现者去移除不相关的工作。
- 准备提交时只 stage 当前任务应当产出的文件。未经用户明确许可，不要把并发 Agent 的工作一并提交或丢弃。
