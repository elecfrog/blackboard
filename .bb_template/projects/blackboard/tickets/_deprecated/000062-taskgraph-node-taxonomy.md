+++
id = "000062"
lane = "bbt"
title = "TaskGraph 062：Interrupt / Command / Resume Control Plane"
created_at = "2026-05-14"
updated_at = "2026-05-15"
status = "todo"
area = "TaskGraph"
assignee = "codex"
attachments = "[{\"kind\":\"wiki\",\"target\":\"proposal/taskgraph-superstep-agent-orchestration.md\",\"label\":\"Superstep TaskGraph Proposal\"}]"
depends_on = "000061"
explicitly_excludes = "business-node-taxonomy,palette-agent-roles,prompt-contract"
kind = "langgraph-distillation-control-plane"
layer = "control-plane"
parent = "000055"
proposal = "wiki/proposal/taskgraph-superstep-agent-orchestration.md"
requested_by = "user"
rewrite_version = "2026-05-15-corrected-langgraph-boundary"
scope = "interrupt-command-resume-approval-durable-control"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出节点模型重做子单。
- 2026-05-15：基于 LangGraphJS `supervisor`、`swarm`、`langgraph-core graph/prebuilt` 与 Pregel 蒸馏重写边界：本 ticket 聚焦业务节点 taxonomy、runtime binding、权限声明和 UI 可配置节点。

- codex 开始执行：开始实现 business node taxonomy 与 runtime binding registry；保持现有 llm/shell/human_gate 执行器兼容，在 registry 层表达 Explorer/Implementer/Verifier 等业务角色。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-062-business-node-taxonomy-and-runtime-binding.md`。

# 背景

TaskGraph 最终要让 Agent 自动协作，但第一阶段不应直接进入“动态生成任意图”。先要有稳定节点模型，让固定模板和人工编排都可运行、可审计、可验收。LangGraph 的 supervisor/swarm/handoff 说明多 Agent 协作不能只靠聊天上下文，控制权转移、状态更新和工具调用都应该成为图节点与命令。

参考 proposal：`wiki/proposal/taskgraph-superstep-agent-orchestration.md`。

# 定位

062 是 TaskGraph 的业务执行层。它定义第一阶段可用节点分类、节点 schema、权限声明、runtime session binding 和前端 palette/inspector 呈现。

062 依赖 #000060 的 NodeRun/SuperstepRun 执行语义，也依赖 #000061 的 typed pins/channel/artifact contract。

# 节点分类

## Control Nodes

负责控制流，不直接调用外部 Runtime：

- `start`
- `end`
- `branch`
- `join`
- `foreach`
- `human_gate`
- `approval_gate`
- `retry_policy`

## Runtime Nodes

负责调用真实 Runtime，并产生 artifact：

- `explorer_agent`
- `implementer_agent`
- `verifier_agent`
- `reviewer_agent`
- `handoff_writer`
- `opencode_session`
- `codex_session`
- `claude_session`
- `local_shell`

## Transform Nodes

负责转换数据，不拥有长期 session：

- `summarize`
- `extract_json`
- `merge_findings`
- `schema_map`
- `diff_summary`
- `test_result_parser`

## Artifact Nodes

负责写入或更新 Blackboard 事实源：

- `create_ticket`
- `update_ticket`
- `attach_ticket_ref`
- `write_wiki_doc`
- `write_handoff_note`
- `record_artifact`

## Integration Nodes

负责外部系统交互：

- `feishu_notify`
- `webhook_call`
- `git_status`
- `git_commit`
- `git_pr`
- `schedule_followup`

# 节点声明契约

每个节点必须声明：

- `id`
- `type`
- `category`
- `display_name`
- `input_pins`
- `output_pins`
- `permissions`
- `runtime`
- `retry_policy`
- `timeout`
- `artifact_outputs`
- `ui_renderer`

权限示例：

- `read_project`
- `read_worktree`
- `write_scoped`
- `run_tests`
- `network`
- `git_operation`
- `external_notify`
- `update_ticket`
- `write_wiki`

# 第一阶段标准 SWE 模板

固定模板先支持：

1. `start`
2. `explorer_agent`：只读调查，输出 findings。
3. `implementer_agent`：限定范围改代码，输出 diff/artifact。
4. `verifier_agent`：运行测试和冒烟，输出 test_result。
5. `reviewer_agent`：审查实现风险，输出 review_comments。
6. `handoff_writer`：整理验收摘要，输出 handoff_summary。
7. `human_gate`：等待用户验收。
8. `end`

后续可以扩成前端设计/后端设计并发、契约 review、拆票、多 agent coding、多轮修复。

# 实施计划

1. 盘点当前 TaskGraph control_nodes/runtime_nodes 和前端 palette。
2. 定义 node category enum 和 node type registry。
3. 定义第一阶段 node spec schema，全部节点使用 #000061 的 typed pins。
4. 定义 runtime binding schema：provider、profile、model、variant、session_resume_policy。
5. 定义 permissions schema，与 AgentSession/Runtime 权限门禁衔接。
6. 实现 Runtime nodes MVP：先接 OpenCode session 和 local shell。
7. 实现 Artifact nodes MVP：write wiki、update ticket、attach ticket ref。
8. 实现 Control nodes MVP：start/end/branch/join/human_gate。
9. 前端 palette 按分类展示节点，inspector 能编辑 pins/runtime/permissions。
10. run UI 能按节点类型展示日志、artifact、approval 和 runtime session。

# Task 列表

- [ ] 梳理现有节点定义和前端 palette，标记保留/合并/废弃。
- [ ] 定义 `NodeCategory`、`NodeType`、`NodeSpec`、`RuntimeBinding`、`PermissionSpec`。
- [ ] 定义第一阶段内置节点 registry。
- [ ] 实现节点 schema validation，缺少 pins/runtime/permissions 时 compile fail。
- [ ] 实现 OpenCode Runtime node binding，复用 AgentSession session id。
- [ ] 实现 local shell Runtime node binding，明确 cwd、timeout、permission。
- [ ] 实现 human_gate/approval_gate 与 #000060 interrupt/resume 对接。
- [ ] 实现 write_wiki_doc/update_ticket/attach_ticket_ref artifact nodes。
- [ ] 改造前端 TaskGraph palette 和 inspector。
- [ ] 改造 run UI：不同 node category 有不同 renderer。
- [ ] 写 SWE 固定模板 graph fixture。
- [ ] 增加 e2e：固定模板可以产生 findings、diff/test_result、handoff summary 和 ticket/wiki refs。

# 验收标准

- 节点分类稳定，markdown/json 不再被当成业务节点类型，而是 data/artifact type。
- 每个节点都有明确 permissions、input/output pins、runtime binding 和 artifact outputs。
- 标准 SWE 模板可以被 compile，并在 #000060/#000061 的执行与数据层上运行。
- Ticket 可以通过 attachments 引用 proposal/wiki/artifact/ticket。
- 前端 palette/inspector 能看到并编辑第一阶段节点。

# 依赖关系

- 上游：#000055 TaskGraph Task Execution。
- 上游：#000060 Superstep-based Execution Kernel。
- 上游：#000061 Dataflow Channels 与 Artifact Contract。
- 相关后续：#000063 Shell/local tool 节点、#000064 Graph run UI、#000065 Feishu/Webhook integrations。

# 记录

- 验证：cargo test -p bb_core task_graph::node_registry --lib：通过，3 passed。
- 验证：cargo test -p bb_core task_graph --lib：通过，128 passed。
- 验证：cargo check -p bb_cli：通过。
- 验证：npm run build --prefix bb_web：通过，仅保留既有 chunk size warning。

- 2026-05-15 纠偏重写：本票从“Business Node Taxonomy 与 Runtime Execution Layer”纠正为 LangGraph 蒸馏的控制平面票。062 只关心 interrupt、command、resume、approval gate、durable human-in-the-loop，不定义业务 Agent 角色。
- 工程边界：权限审批可以通过 interrupt 表达，但权限模型属于 Blackboard 上层实践；062 提供的是可暂停、可恢复、可携带 resume value 的执行控制语义。
- 验收口径：不是 Explorer prompt 从哪里来，而是任意节点在执行中能抛出 interrupt，系统持久化中断点，用户/外部系统提交 Command 后可从同一 checkpoint 继续。

# 下一步

- 把 registry 的 permissions/runtime binding 接入 pre-run validation 和 runtime dispatch 门禁；再实现 write_wiki_doc/update_ticket 这类 artifact node 的真实 executor。

- 定义 interrupt payload：id、node_id、superstep、reason、required_action、request_schema、created_at、resumable。
- 定义 Command 模型：resume、update_state、goto、cancel、retry、approve、deny，并明确哪些命令可以改变 run cursor。
- 实现 interrupt lifecycle：node 抛出 interrupt 后 run 进入 needs_approval/blocked，checkpoint 记录 interrupt，不丢失 pending state。
- 实现 resume value 注入：用户输入或权限审批结果作为 resume payload 交回原节点或下一节点。
- 实现 permission-as-interrupt：runtime adapter 遇到高风险操作时返回 interrupt，而不是直接失败或绕过。
- 实现 deterministic resume：同一个 interrupt id 只能消费一次；重复提交 Command 不得重复执行已完成节点。
- 补测试：before-node interrupt、during-node interrupt、approve resume、deny cancel、resume value 注入、重复 resume 幂等。
