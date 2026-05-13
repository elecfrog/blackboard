+++
id = "000031"
lane = "bbt"
title = "Task Graph 后端：Merged Catalog、CRUD 与 Fork API"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000030"
kind = "backend"
parent = "000028"
+++

# 当前进展

- 交付目标：提供 Web 可用的 Task Graph HTTP API，把 BB 内置 system graph 与当前 project graph 合并成统一 catalog，并支持 project graph 的创建、编辑、删除和 system graph fork。
- Catalog 范围：GET /api/projects/{project}/task-graphs 返回 system + project 两类条目，标注 scope、readonly、origin、title、description、updated_at。
- Graph 范围：读取 graph detail 时返回完整 definition；project graph 可 PATCH 保存；system graph 只读，编辑前必须 fork。
- Fork 范围：POST system graph customize/fork 时复制 snapshot 到 projects/<project>/task_graphs/，写入 origin.scope/id/version。
- 兼容范围：API payload 必须与契约 ticket 样例一致，前端可以直接替换 mock 数据。

- Task Graph cleanup: 在 `bb_cli/src/http.rs` 新增 Task Graph HTTP API 完整实现
- Task Graph cleanup: 6 个端点：list catalog (GET), read system graph (GET), read project graph (GET), create (POST), patch (PATCH), delete (DELETE), fork (POST)
- Task Graph cleanup: TaskGraphApiError — 独立错误适配，映射契约错误码（graph_not_found/readonly_graph/validation_failed/stale_version/duplicate_graph_id）
- Task Graph cleanup: 6 个 DTO structs：TgCatalogResponse, TgGraphResponse, TgWriteResponse, TgCreateBody, TgPatchBody, TgForkBody

# 记录

- 本单依赖后端模型与校验，不依赖 executor，可先让前端完成 catalog/editor 保存闭环。
- 验收标准：system graph 可见但不可编辑；project graph 可创建/编辑；system fork 后生成 project graph；非法 graph 保存返回结构化错误。
- 验证要求：补 HTTP tests 覆盖 list/detail/create/patch/fork/error cases。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codebuddy-task-graph-http-api.md。

# 下一步

- 新增 task_graph HTTP routes。
- 实现 merged catalog DTO 与 detail DTO。
- 实现 project graph create/patch/delete 与 system fork endpoint。
- 补齐错误码：not_found、readonly_graph、validation_failed、conflict。
