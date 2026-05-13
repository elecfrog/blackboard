+++
id = "000037"
lane = "bbd"
title = "Task Graph 前端：Run UI、分支决策、循环轮次与日志查看"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000035 000032"
kind = "frontend"
parent = "000028"
+++

# 当前进展

- 交付目标：实现 Task Graph 运行态 UI，让用户能启动 graph run、观察节点状态、查看 Branch 决策、Loop 轮次、日志和 artifacts。
- 启动范围：从 catalog/detail/editor 触发 run，运行前显示 scope/id/version 与是否为 system/project graph。
- 状态范围：节点显示 idle/queued/running/succeeded/failed/skipped/paused；run 顶部显示整体状态与耗时。
- 控制流可视化：Branch 节点显示实际选择的出口；Loop 节点显示当前/历史 iteration、退出原因和 max_iterations 状态。
- 日志范围：点击节点打开 logs/artifacts drawer，展示 stdout/stderr tail、LLM 输出 artifact、错误原因。
- Human Gate：run paused 时提供继续/拒绝的 UI 占位，MVP 可先只显示暂停原因，恢复动作另拆或接入后端能力。

- Task Graph cleanup: 完成 037 Task Graph Run UI 前端实现，新增运行态详情面板。
- Task Graph cleanup: 补齐 Task Graph run detail、run node、branch decision、loop iteration、artifact/log 的前端数据契约。
- Task Graph cleanup: 增加 mock run 持久化和 read/start client，后端 API 不可用时可用 fixture 完整验收。
- Task Graph cleanup: 将 catalog 卡片、详情预览和 editor 的运行入口接入统一 Run UI。

- - 2026-05-09：为 Task Graph run detail 增加 SSE events 端点，前端 run panel 改为订阅 run 会话更新
- - 2026-05-09：将 task-graph-inbox-cleanup 的 no-candidate 结果改为 cleanup-one skipped，并在 artifact/log_tail 写入 reason=no_candidate
- - 2026-05-09：更新 Inbox 清理循环 system graph 和注册任务文案，避免把 bb-internal fast path 误显示成 AI/agent 执行

- 2026-05-09：新增 graph-level inputs 契约，默认值持久化到 system/project graph JSON，运行创建时合并用户输入注入 run.context.input
- 2026-05-09：Loop 支持 max_iterations_ref 绑定 {{inputs.max-iterations}}，registered task prompt vars 支持 {{inputs.*}} 绑定
- 2026-05-09：前端 Catalog/Editor/Run 画布改为 UE 风格 Loop/Branch 执行 pin 显示，Loop 区分 Loop Body 与 Completed
- 2026-05-09：前端新增运行前 input 参数填写区，Editor 内 graph inputs 编辑和变量绑定入口

# 记录

- 本单前端可先用 run state fixture 并行开发；最终接后端 run detail API。
- 验收标准：能展示一条包含 branch/loop/human_gate 的 run；失败节点有明确错误；日志/artifact 可查看。
- 验证要求：npm run build --prefix bb_web 通过；browser-use 用 fixture 或真实 API 验证 run UI 各状态可读。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codex-task-graph-run-ui.md。

- 来源：inbox/2026-05-09-codex-task-graph-run-events-noop.md
- 相关路径：bb_backend/crates/bb_cli/src/http.rs, bb_backend/crates/bb_core/src/task_graph/interpreter.rs, bb_web/src/components/TaskGraphRunPanel.vue, bb_web/src/data/taskGraphs.ts, task_graphs/system/inbox-cleanup-pipeline.json

- 来源：inbox/2026-05-09-codex-task-graph-inputs-loop-ui.md

# 下一步

- 实现 run create/read data client。
- 实现 node status overlay 与 run summary。
- 实现 branch decision/loop iteration 面板。
- 实现 logs/artifacts drawer。
