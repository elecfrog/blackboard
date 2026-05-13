+++
id = "000032"
lane = "bbt"
title = "Task Graph 后端：Run Snapshot、Context 与运行态存储"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000029"
kind = "backend"
parent = "000028"
+++

# 当前进展

- 交付目标：实现 Task Graph 每次运行的冻结快照、run context、node run state、日志与 artifact 存储，为 executor 和前端 Run UI 提供稳定读写层。
- 存储范围：.bb_runtime/task_graph_runs/<project>/<run-id>/ 下保存 run.json、graph.snapshot.json、nodes/<node-id>.json、logs/<node-id>.log、artifacts/<node-id>.*。
- 状态范围：run status 覆盖 pending、running、paused、succeeded、failed、cancelled；node status 覆盖 idle、queued、running、succeeded、failed、skipped、paused。
- 控制流记录：Branch 必须记录 decision path；Loop 必须记录 iteration index、退出原因和 max_iterations 命中情况。
- 可追溯要求：system graph 后续升级不影响历史 run；run detail 永远读取 graph.snapshot.json。

- Task Graph cleanup: 新增 `bb_core/src/task_graph/run_state.rs` — 运行态完整实现
- Task Graph cleanup: 类型定义：TaskGraphRun, RunStatus(6态), NodeRunStatus(7态), RunContext, BranchDecision, LoopIterationState, TaskGraphRunNode, TaskGraphRunDetail, TaskGraphRunSummary, RunPaused, OutputArtifact 等
- Task Graph cleanup: 存储操作：create_run（冻结 snapshot + 初始化节点 idle）、list_runs、read_run、read_run_detail（聚合读取）
- Task Graph cleanup: 状态管理：update_run_status（含转移合法性校验）、set_run_paused、update_cursor

# 记录

- 本单可与 graph store/API 并行推进，双方共同依赖契约 ticket。
- 验收标准：创建 run 时生成完整目录与 snapshot；能更新节点状态、追加日志、写 artifact、读取 run detail。
- 验证要求：补单测覆盖 run 创建、状态转移、snapshot 冻结、branch/loop 元数据写入。

- Task Graph cleanup pipeline: 已沉淀并删除 inbox/2026-05-09-codebuddy-task-graph-run-state.md。

# 下一步

- 定义 TaskGraphRun、TaskGraphRunNodeState、TaskGraphRunContext 类型。
- 实现 run id 分配与 runtime 目录写入。
- 实现 run detail 聚合读取。
- 提供 executor 使用的 append log/write artifact/update node state helper。
