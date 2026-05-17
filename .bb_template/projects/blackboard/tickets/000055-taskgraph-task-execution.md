+++
id = "000055"
lane = "bbt"
title = "TaskGraph Task Execution：运行态、Runtime Session 与验收闭环"
created_at = "2026-05-14"
updated_at = "2026-05-17"
status = "archived"
area = "TaskGraph"
depends_on = "000053"
kind = "platform-track"
parent = "000053"
requested_by = "user"
scope = "task-execution"
+++

# 当前进展

- 2026-05-14：从 TaskGraph Vision 拆出 Task Execution 主干 ticket。

- 2026-05-14：已拆出 6 个执行增强子单：Execution 基建、数据流类型系统、节点模型、Shell 节点、前端 Run UI、第三方生态。

- 2026-05-15：新增 node_registry 模块，定义 NodeCategory/NodeRole/RuntimeBinding/PermissionSpec/ArtifactOutputSpec/NodeSpec，内置 explorer_agent/implementer_agent/verifier_agent/reviewer_agent/handoff_writer/opencode_session/codex_session/local_shell/write_wiki_doc 业务角色，复用 llm/shell/sub_graph 执行器

# 记录

- 目标：负责 TaskRun 的启动、节点执行、Runtime Session 绑定、事件时间线、Artifact 归档、失败修复循环、权限中断、取消/恢复/验收。
- 运行状态：pending、running、blocked、needs_approval、failed、ready_for_review、done、cancelled。
- 核心原则：Execution 不重新解释用户意图，只执行 Compile 后的计划，并把过程和产物可恢复、可审计地记录下来。

- 子单：#000060 TaskGraph Execution 基建：TaskRun/NodeRun 状态机与事件流。
- 子单：#000061 TaskGraph 数据流与类型系统：Typed Pins、Context 与 Artifact Contract。
- 子单：#000062 TaskGraph 节点模型重做：Control、Runtime、Transform、Artifact、Integration 分类。
- 子单：#000063 TaskGraph Shell 与本地工具节点：编译、测试、Git 与命令执行闭环。
- 子单：#000064 TaskGraph 前端 Graph 与 Run UI：节点编辑、状态覆盖与 Artifact 面板。
- 子单：#000065 TaskGraph 第三方生态：飞书通知、Webhook、本地工具与插件接入。

- 来源：inbox/2026-05-15-codex-taskgraph-062-business-node-taxonomy-and-runtime-binding.md
- 代码位置：bb_backend/crates/bb_core/src/task_graph/node_registry.rs, bb_backend/crates/bb_core/src/task_graph/mod.rs, bb_web/src/components/task-graph/taskGraphNodeVisuals.ts, bb_web/src/components/task-graph/TaskGraphNodePalette.vue, bb_web/src/components/TaskGraphEditorPanel.vue

# 下一步

- 讨论 TaskRun/NodeRun/Event/Artifact 的最小运行态模型。
- 定义 OpenCode/Shell 第一阶段如何绑定 session、cwd、model、variant。
- 确定 ready_for_review 的验收摘要和通知形态。

- 逐张讨论 055 子单，先明确每张的边界、第一阶段验收标准和依赖关系。
