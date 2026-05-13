+++
id = "000039"
lane = "bbq"
title = "Task Graph MVP 打磨：真实清理工作流与验收体验 polish"
created_at = "2026-05-09"
updated_at = "2026-05-10"
status = "archived"
area = "TaskGraph"
assignee = "codebuddy"
depends_on = "000038"
kind = "quality"
parent = "000028"
+++

# 当前进展

- 交付目标：在 000038 端到端验收之后，继续打磨 Task Graph MVP 的真实清理工作流、运行反馈与验收体验。
- 范围：优化 inbox-cleanup-pipeline 的循环上限、run artifact 汇总、catalog 最近运行展示、失败/部分成功反馈，以及真实清理后的用户确认路径。
- 约束：不扩大 MVP 架构面，不把旧 Ticket Graph/List 视图重新纳入本单。

- 2026-05-09：完成 Task Graph MVP 打磨，新建 000039 polish 单并标记 000028/000030-000038 为 done

- 2026-05-09：将 inbox-cleanup-pipeline 从前端 mock 补成真实 Task Graph system graph

- 2026-05-09：改清理流程为 Task Graph Loop 编排，每轮只处理一条 inbox note
- 2026-05-09：新增 bb-internal 注册任务路径，避免 OpenCode MCP 断连影响真实清理
- 2026-05-09：修复 inbox index 中文截断时的字符边界 panic
- 2026-05-09：从页面运行真实清理流水线，成功沉淀并删除 3 条 Task Graph inbox note

- 2026-05-09：将 Task Graph registered task 的 opencode 执行改为 `opencode run --format json`，复用 multica 的 JSONL event 思路解析 `text`、`tool_use`、`error`、`step_finish`
- 2026-05-09：移除 `bb-internal` 清理执行路径，让 `task-graph-inbox-cleanup` 真实运行 `bb-pm`，由 LLM 读取 inbox 并决定如何沉淀
- 2026-05-09：为 Task Graph run 增加运行中的节点日志写入，opencode event 到达时立即更新 `log_tail`，前端监听 run state 时能看到实时工具调用
- 2026-05-09：将 `.json` artifact 写入收口为真实 JSON，避免把 fenced markdown 文本写进 JSON artifact
- 2026-05-09：补齐取消语义：run 被 cancel 后 interpreter 会杀掉当前 runtime 子进程，并把当前节点收为 skipped/cancelled
- 2026-05-09：通过真实 system graph run 验证 `bb-pm` 更新 ticket 并删除 inbox note，且运行耗时为分钟级真实 LLM 执行

- 2026-05-10：修复 task-graph-inbox-cleanup prompt 直接读取 graph inputs，{{inputs.max-iterations}} 已可解析；移除 vars 中转，保留 {{env.*}}/{{inputs.*}}/{{nodes.*}} 明确表达式语义；补充 graph inputs 渲染测试，修复 Task Graph 测试中过期的 frontend-smoke fixture 引用

- 2026-05-10：让 LLM config.inputs 作为节点显式输入绑定层参与 prompt 渲染，保持 {{inputs.xxx}} 语义；为 inbox-batch-cleanup LLM 节点声明 batch-count 输入绑定；在 TaskGraphEditorPanel LLM 节点 inspector 加入 inputs binding list；补充中英文添加输入文案与 LLM 显式 inputs 渲染测试

# 记录

- 来源：用户要求 038 后跟新单做打磨，并将前序 Task Graph MVP 工单收口。
- 依赖：000038 已完成真实 system graph run 验收，inbox 清理可从页面点击后真实沉淀 ticket 并删除 note。

- 来源：inbox/2026-05-09-codex-task-graph-polish-ticket.md

- 来源：inbox/2026-05-09-codex-cleanup-workflow-fix.md（已删除，facts 后补入）
- 相关路径：task_graphs/system/inbox-cleanup-pipeline.json

- 注：inbox note 删除在 append 之前发生，属于流程失误；facts 已补录到 000039

- 来源：inbox/2026-05-09-codex-task-graph-opencode-events.md

- 来源：inbox/2026-05-10-codex-task-graph-llm-prompt-直读-graph-inputs.md
- 相关路径：agents/prompts/task-graph-inbox-cleanup.md; bb_backend/crates/bb_core/src/task_graph/eval.rs; task_graphs/system/inbox-batch-cleanup.json

- 来源：inbox/2026-05-10-codex-task-graph-llm-显式-inputs-绑定.md
- 相关路径：bb_web/src/components/TaskGraphEditorPanel.vue

# 下一步

- 梳理 000038 验收后的 polish 清单。
- 补充真实清理 workflow 的 run summary 和 UI 可见反馈。
- 评估是否需要把 bb-internal task 能力抽成更通用的内部节点执行器。
