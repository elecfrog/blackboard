+++
id = "000004"
lane = "bbp"
title = "Blackboard 转为正常 C/S 数据路径，移除离线快照语义"
created_at = "2026-05-06"
updated_at = "2026-05-08"
status = "archived"
assignee = "codex"
depends_on = "000001"
parent = "000001"
+++

# 当前进展

- 2026-05-06：产品方向确认，Blackboard 不再以“离线快照 / 前端尽量 ReadOnly”为目标。
- 2026-05-06：本票落号，作为前后端一起清理静态快照兜底、转向 bb-server 实时数据源的产品票。
- 2026-05-06：首轮改造完成。Web 数据层已移除 projects/tickets/inbox/lanes 的静态 JSON runtime fallback，Board / Inbox UI 不再展示“离线快照 / 本地聚合”语义。
- 2026-05-06：bb-server HTTP 文档与注释已改为正常 C/S runtime API，`/api/projects` 响应补充 `generated_at`，导出 JSON 只保留为 export/debug artifact。

- 2026-05-06：Web 数据层改为只读 /api/** runtime 数据，移除 projects/tickets/inbox/lanes 静态 JSON fallback

# 记录

## 背景

早期 Blackboard 以“bb-server 不在时前端仍能读静态 JSON 快照”为重要目标，因此 web 数据层保留了 `/blackboard/projects/<project>/{tickets,inbox,lanes}.json` 的静态兜底路径，并在 UI 上显示“离线快照”等状态。

现在产品目标已经转向正常 C/S：本地 bb-server 是运行时服务，web 前端应默认连接服务端拿实时数据和执行写操作。服务端不可用时应该明确报错，而不是悄悄降级到旧快照造成“看起来能用但实际不是运行时状态”的错觉。

## 范围

- 前端数据层默认走 `/api/**`，移除 tickets / inbox / lanes / projects 的静态 JSON fallback。
- 前端 UI 移除“离线快照”“本地聚合”等旧提示语义。
- 后端 HTTP 文档和代码注释从“只读 / 静态快照优先”调整为正常 C/S runtime API。
- 保留 `export:tickets` 脚本作为归档、审计或调试产物，不再作为 web 运行时兜底事实源。

## 验收

- 未启动 bb-server 时，web 明确提示 API 不可用。
- 启动 bb-server 时，Board / Tickets / Inbox / Settings 均从运行时 API 渲染。
- 代码里不再出现“falling back to static snapshot / 离线快照”作为 web 运行时路径。

## 验证

- `npm run build --prefix blackboard/web` 通过。
- `cargo test`（`blackboard/bb-server`）通过。
- 8060 代理 API 冒烟通过：`/api/projects`、`/api/projects/blackboard/tickets`、`/api/projects/blackboard/inbox/notes`、`/api/projects/blackboard/lanes`、`/api/projects/blackboard/board/summary`。

- 来源：2026-05-06-codex-blackboard-cs-runtime-data-path.md
- export:tickets 保留为 export/debug artifact，不再是 Web runtime 兜底事实源