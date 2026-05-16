+++
id = "000063"
lane = "bbt"
title = "TaskGraph 063：Blackboard Runtime Adapter Layer for Existing Nodes"
created_at = "2026-05-14"
updated_at = "2026-05-16"
status = "todo"
area = "TaskGraph"
assignee = "codex"
attachments = "[{\"kind\":\"wiki\",\"target\":\"proposal/taskgraph-superstep-agent-orchestration.md\",\"label\":\"Superstep TaskGraph Proposal\"}]"
depends_on = "000062"
explicitly_excludes = "inventing-new-business-agent-model,replacing-agent-profile,prompt-source-redesign"
kind = "runtime-adapter-layer"
layer = "adapter"
parent = "000055"
proposal = "wiki/proposal/taskgraph-superstep-agent-orchestration.md"
requested_by = "user"
rewrite_version = "2026-05-15-corrected-langgraph-boundary"
scope = "adapt-existing-llm-agent-profile-shell-opencode-to-superstep-runtime"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出 Shell 与本地工具节点子单。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-完善-000063-shell-local-tool-节点-ticket-描述与计划.md`。

- agent 开始执行：开始实施 Shell/Local Tool Node：新增 shell 节点类型、后端执行器、前端配置入口与验证测试。

- Codex 完成阶段工作，handoff 写入 `2026-05-15-codex-实施-taskgraph-shell-local-tool-node.md`。

- 2026-05-12：新增 TaskGraph LLM node run_as resolver，支持 llm vs agent 两种模式；Agent 模式解析 Agent Profile runtime/model/skills/MCP/custom env/args 并从 instructions_path 读取 prompt。
- 2026-05-12：将 OpenCode AgentSession 和 legacy Codex runtime 接入 resolved invocation data，包含 node/profile skills、MCP config、custom env 和 custom args。
- 2026-05-12：更新 TaskGraph editor 暴露 LLM/Agent 模式表单；Agent 模式选择 project agents，LLM 模式编辑 runtime/model/provider agent/inline prompt/skills/MCP/advanced env args。
- 2026-05-12：修复 bb-pm profile model 使用 opencode-available minimax-cn-coding-plan/MiniMax-M2.7-highspeed。
- 2026-05-12：创建并运行 project smoke graph agent-profile-mode-smoke-20260513-020952，成功 persisted AgentSession as-20260512-181102-5f43d484，含 28 events、6 tool events。

# 记录

- 目标：补齐 Shell 节点，让 TaskGraph 能真实执行编译、测试、git diff、本地脚本和项目命令。
- 最小字段：cwd/worktree、command、args/env、timeout、permission、stdout、stderr、exit_code、artifact capture。
- 定位：没有 Shell 节点，TaskGraph 很难形成从实现到验证的工程闭环。

- 2026-05-15：补充工作描述：本单负责在 TaskGraph Execution 主干内提供可审计、可取消、可产物化的本地命令执行节点，使编译、测试、Git 状态读取与项目脚本能作为标准节点接入 TaskRun。
- 范围边界：聚焦 Shell/Local Tool 节点自身的配置 schema、权限边界、进程执行、输出事件流、exit_code 语义与 artifact capture；不重做 000060 的 TaskRun/NodeRun 状态机，也不实现 000064 的前端 Run UI。
- 第一阶段节点形态：支持指定 cwd/worktree、command、args、env、timeout、permission profile、artifact capture 规则，并把 stdout、stderr、exit_code、duration、truncated/timeout/cancelled 等执行结果写入 NodeRun 事件和 Artifact。
- 权限与安全原则：cwd 必须约束在 project/worktree 内；环境变量默认最小暴露并支持脱敏；长输出要截断但保留 artifact；写入型、网络型、Git 修改型命令需要显式 permission profile 或审批中断。
- 验收标准：能以统一 ShellNode 配置跑通 npm run build、cargo test、git diff/git status、项目脚本四类样例；成功和失败命令都能产生完整 stdout/stderr/exit_code artifact；超时、取消、非零退出码、权限拒绝都有明确事件和可恢复状态。

- 验证：bb_list_projects：通过，确认 project 为 blackboard。
- 验证：bb_read_ticket_by_id 000063：通过，读取原始 ticket。
- 验证：bb_read_ticket_by_id 000055：通过，读取父单上下文。
- 验证：bb_append_ticket_sections 000063：通过，maintenance consistency passed，export completed。
- 验证：bb_read_ticket_by_id 000063：通过，确认新增内容已写入且 updated_at=2026-05-15。

- 验证：bb_list_projects：通过，确认 project 为 blackboard。
- 验证：bb_read_ticket_by_id 000063：通过，读取 ticket 上下文。
- 验证：bb_begin_ticket_work 000063 with agent=Codex：失败，后端返回 assignee `Codex` 不是 blackboard project active registered agent。
- 验证：bb_begin_ticket_work 000063 without agent：通过，ticket status 更新为 in_progress，maintenance consistency passed，export completed。
- 验证：cargo test -p bb_core task_graph --manifest-path bb_backend\\Cargo.toml：通过，最终 120 passed。
- 验证：cargo test --manifest-path bb_backend\\Cargo.toml：通过，bb_cli 75 passed，bb_core 251 passed，doc-tests 0 passed。
- 验证：npm run build --prefix bb_web：首次失败于并发 schedule 面板类型/i18n 缺口；修正当前剩余 `selectedRef` 类型后重跑通过，Vite 仅提示 chunk size warning。
- 验证：cargo fmt --all --check --manifest-path bb_backend/Cargo.toml：失败但仅报告并发/既有文件 bb_cli/src/http/task_graph/mod.rs 与 bb_core/src/task_graph/schedules.rs 的格式差异；063 相关 Rust diff 已通过 git diff --check。
- 验证：git diff --check -- <063 touched files>：通过，仅有 LF/CRLF warning，无 whitespace error。

- 2026-05-15 纠偏重写：本票从“Shell/local tool 单点节点”纠正为 Blackboard 既有 runtime adapter 层。063 的任务是把现有 LLM 节点、Agent Profile、Shell、本地工具、OpenCode/Codex/Claude session 接入 060-062 的执行语义，而不是重新发明业务 Agent 模型。
- 工程边界：LLM 节点和 Agent Profile 是 Blackboard 自己的工程实践，063 只做 adapter contract：如何执行、如何返回 writes/events/interrupts、如何绑定 session、如何报告失败和产物引用。
- 验收口径：不是新增 Explorer/Implementer palette，而是现有节点类型能作为 RunnableNode 接入 superstep engine，输出统一 NodeOutcome，并在需要权限时走 interrupt/Command。

- 来源：2026-05-12-codex-taskgraph-llm-agent-dual-mode-implementation.md
- 代码位置：bb_backend/crates/bb_core/src/task_graph/llm.rs, bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/llm_node.rs, bb_web/src/components/TaskGraphEditorPanel.vue
- 验证：cargo test -p bb_core task_graph: passed (91 tests); cargo test -p bb_core agent_session: passed; npm run build --prefix bb_web: passed; REST smoke: GET agent session/events passed

- 来源：inbox/2026-05-15-codex-完善-000063-shell-local-tool-节点-ticket-描述与计划.md

- 来源：inbox/2026-05-15-codex-实施-taskgraph-shell-local-tool-node.md

# 下一步

- 定义 ShellNode 配置 schema 与权限边界。
- 实现命令输出事件流与 stdout/stderr/exit_code artifact。
- 用 npm build / cargo test / git diff 作为第一批验收样例。

- 梳理并落地 ShellNode 配置 schema：cwd/worktree、command/args、env、timeout、permission、capture、expected_exit_codes、inputs/outputs。
- 定义本地工具节点与 000060 运行态的事件契约：started、stdout_chunk、stderr_chunk、artifact_created、exit、timeout、cancelled、permission_required、permission_denied。
- 实现最小 backend executor：安全解析 cwd，启动子进程，流式采集 stdout/stderr，记录 exit_code/duration，并支持 timeout 与 cancel。
- 补齐 artifact capture：将 stdout、stderr、combined log、exit metadata、可选文件产物写入统一 Artifact Contract，供后续 000064 展示。
- 设计 permission profile 最小集：read_only、project_write、git_write、network，并明确哪些命令进入审批/blocked 状态。
- 建立验收样例与自动化验证：npm run build、cargo test、git status/diff、一个本地脚本；覆盖成功、失败、超时、权限拒绝四类路径。

- 后续可直接按 000063 下一步从 ShellNode 配置 schema 和 permission profile 开始实现。

- 后续如要提交，需只 stage 063 相关 hunk；当前 worktree 同时包含 000059/000064 schedule/canvas 等并发改动，不能整文件盲 stage。
- 可后续把 Shell artifact 拆成 stdout/stderr/metadata 多 artifact，或在 060 superstep 落地后迁移到 channel/artifact contract。

- 定义 RuntimeAdapter trait/interface：prepare、invoke、cancel、resume、health、capabilities，输入为 compiled node + state snapshot，输出为 NodeOutcome。
- 定义 NodeOutcome：writes、events、artifacts/ref、interrupt、error、retry_hint、session_binding，禁止 adapter 直接改 checkpoint。
- 接入现有 LLM 节点：沿用现有 prompt/config/Agent Profile 机制，只把执行结果包装成 NodeOutcome。
- 接入 Shell/local tool：cwd/command/env/timeout/exit_code/stdout/stderr 进入 events 与 writes；权限风险返回 interrupt。
- 接入 OpenCode/Codex/Claude session：保存 runtime session id、resume policy、日志引用和取消能力。
- 实现 adapter validation：capability 不满足、权限缺失、session 不可恢复时在 compile/pre-run 阶段给出结构化错误。
- 补测试：LLM dry-run adapter、Shell success/failure/timeout、permission interrupt、session resume binding、adapter error 不破坏 checkpoint。
