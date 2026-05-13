+++
id = "000033"
lane = "bbt"
title = "Task Graph 后端：Workflow Interpreter 与 daemon 执行器"
created_at = "2026-05-09"
updated_at = "2026-05-09"
status = "archived"
area = "TaskGraph"
depends_on = "000031 000032"
kind = "backend"
parent = "000028"
+++

# 当前进展

- 交付目标：在 bb daemon 中实现 Task Graph workflow interpreter，支持顺序节点、Branch、Loop、Human Gate、LLM 节点和 registered task 节点的 MVP 执行闭环。
- 执行模型：不使用纯拓扑排序；使用 run context + cursor/queue 的解释器模型，根据节点结果解析下一条控制流边。
- LLM 节点：复用现有 runtime adapter/build_runtime_command，按 node config 选择 runtime、agent、model、prompt_template，并将输出写入 artifact。
- Registered task 节点：复用 agents/tasks.toml 中已注册任务，以当前 project context 执行，并记录 stdout/stderr tail。
- Branch 节点：读取上游 node output/run context，按 first_match rules 选择出口；MVP 优先 deterministic JSON/path/operator 条件。
- Loop 节点：支持 max_iterations 必填、iteration snapshot、退出原因记录；禁止无限循环。
- Human Gate 节点：执行到 gate 时 run 进入 paused，等待后续 API/人工动作恢复。

- 2026-05-09：实现 bb_core::task_graph::interpreter 模块（约 700 行），含 cursor/queue 解释器主循环、next-edge 解析器、各节点类型执行处理器。
- 2026-05-09：新增 16 个 interpreter 测试（branch ×6、loop ×3、full run ×7），task_graph 测试共 70 个通过，HTTP 测试 11 个通过。

- 2026-05-09：Task Graph registered task 改为 opencode run --format json，复用 JSONL 解析 text/tool_use/error/step_finish event

- 2026-05-09：移除 bb-internal 清理路径，task-graph-inbox-cleanup 真实运行 bb-pm；增加 run 节点实时日志写入 opencode event；.json artifact 改为真实 JSON；补齐 cancel 语义

- 2026-05-09：Loop interpreter 持久化 loop frame，body path 自然结束时使用 active frame 返回 Loop controller
- 2026-05-09：Loop iteration history 在控制权返回 Loop 节点时标记 active frame iteration 为完成
- 2026-05-09：Loop 校验不再要求用户可见 return edge，兼容旧 graph 的 return target
- 2026-05-09：默认 Loop pins 只暴露 exec_in、body、exit、index 四个 pin

# 记录

- 本单是后端 MVP 的执行核心，依赖 API 可创建 run，也依赖 run state 可持久化状态。
- 验收标准：能执行包含 LLM/registered_task/branch/loop/human_gate 的示例 graph，并在失败、暂停、循环达到上限时给出可读状态。
- 验证要求：补 executor 单测或集成测试，至少覆盖 no-failure 分支、failure 分支、loop retry、max_iterations、human gate pause。

- 来源：inbox/2026-05-09-codebuddy-033-interpreter.md

- 来源：inbox/2026-05-09-codex-task-graph-opencode-events.md

- 来源：inbox/2026-05-09-codex-task-graph-opencode-events.md

- 来源：inbox/2026-05-09-codex-task-graph-loop-frame.md

# 下一步

- 新增 daemon task-graph-runner 入口或等价 run pickup 机制。
- 实现 workflow interpreter cursor/queue 与 next-edge resolver。
- 接入 runtime adapter 执行 LLM 与 registered task 节点。
- 输出 node logs/artifacts，并向 run state 写入 branch decisions 与 loop iterations。
