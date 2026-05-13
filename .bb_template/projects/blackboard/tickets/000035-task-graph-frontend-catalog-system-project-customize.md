+++
id = "000035"
lane = "bbd"
title = "Task Graph 前端：Catalog、System/Project 分层与 Customize 入口"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
deliverable = "bb_web/src/components/TaskGraphCatalogPanel.vue; bb_web/src/data/taskGraphs.ts"
depends_on = "000029"
kind = "frontend"
parent = "000028"
+++

# 当前进展

- 交付目标：实现 Task Graph 入口页，展示 BB 内置 system graphs 与当前 project graphs，并提供运行、查看、创建和 customize/fork 的入口。
- Catalog 范围：列表区分 Built-in/System 与 Project/User，明确 readonly、origin、最近运行状态、更新时间。
- Customize 范围：用户编辑 system graph 时不直接改内置图，而是触发 fork 到当前 project 后进入 editor。
- Mock 并行：在后端 API 完成前先使用契约 ticket fixture；API 到位后替换为真实请求。
- 导航范围：新增 Task Graph 入口，遵守现有 Tickets/Wiki/Agents 页面布局密度，不做营销式 landing。

- 已新增 bb_web/src/data/taskGraphs.ts，按 000029 契约实现 catalog/read/create/fork/run 数据客户端；真实 API 可用时走 REST，后端未实现时回落到 mock fixture + project localStorage。
- 已新增 TaskGraphCatalogPanel.vue，提供 system/project 分层 catalog、过滤、GraphCanvas readonly 预览、run/view/create/customize/edit 入口。
- 已新增 /projects/:project/task-graphs 与 /projects/:project/task-graphs/:scope/:graphId/:mode? 路由，并接入左侧导航与 ProjectSwitcher。
- system graph 在 UI 中保持 readonly，customize/fork 会生成 project graph 后进入 edit route；project graph 具备编辑入口。
- 已补 Task Graph 相关中英文 i18n 文案。

- Task Graph cleanup: 完成 000035 实现：新增 Task Graph Catalog 工作区、左侧导航入口与 `/projects/:project/task-graphs` 路由。
- Task Graph cleanup: 新增 `bb_web/src/data/taskGraphs.ts`，按 000029 契约提供 catalog/read/create/fork/run 客户端，并在后端 API 未就绪时回落到 mock fixture + project localStorage。
- Task Graph cleanup: 新增 `bb_web/src/components/TaskGraphCatalogPanel.vue`，展示 system/project 分层列表、readonly/editable 状态、最近运行状态、GraphCanvas 只读预览和 create/customize/run/view/edit 入口。
- Task Graph cleanup: 更新 `BoardView.vue`、`ProjectSwitcher.vue`、`main.ts` 和 `i18n.ts`，把 Task Graph 工作区接入现有 Dashboard 外壳与中英文文案。

# 记录

- 本单与 Graph Canvas 抽取可并行推进，先完成信息架构和数据接线。
- 验收标准：能看到 system/project 两类 graph；system graph 只读；project graph 可进入编辑；customize 交互路径清晰。
- 验证要求：npm run build --prefix bb_web 通过；browser-use 检查 catalog 页面空状态、system/project 混合列表和 customize 入口。

- 验证：npm run build --prefix bb_web 通过。
- 验证受限：browser-use 对 http://127.0.0.1:8060/ 访问被浏览器安全策略阻断，未能执行 catalog 页面空状态、system/project 混合列表和 customize 入口的浏览器 smoke。
- 维护受限：bb stdio 二进制在读取既有中文 inbox 摘要时触发 UTF-8 byte boundary panic，本次 ticket 回写改用文件补丁，并通过脚本门禁复核。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-catalog.md。

# 下一步

- 新增 TaskGraphView 或相关 route。
- 新增 task graph data client 与 mock fixture。
- 实现 system/project catalog 列表与过滤。
- 实现 create project graph 与 customize system graph 的 UI flow。

- 036 可接管 project graph edit route，补 editor/inspector/control-flow 实现。
- 031 后端 API 到位后，当前 data client 会优先使用 REST catalog/create/fork/run 接口。
