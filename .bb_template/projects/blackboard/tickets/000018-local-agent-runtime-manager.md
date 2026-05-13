+++
id = "000018"
lane = "bbt"
title = "本地 Agent Runtime Manager 能力底座"
created_at = "2026-05-07"
updated_at = "2026-05-09"
status = "archived"
assignee = "codex"
+++

# 当前进展

- 2026-05-08：确认 Blackboard 要从全局 Agent 配置注入器，转向本地 Agent Runtime Manager

- 2026-05-08：参考 Multica/Slock.ai，明确 Web/Server 只负责 tickets、task queue 与 runtime 状态
- 2026-05-08：明确本机 bb daemon 负责检测 Agent CLI、创建 per-task 环境并启动 Codex/Claude/OpenCode
- 2026-05-08：明确不再以强写全局 AGENTS/MCP 配置为主路径，改为 per-task 注入临时上下文
- 2026-05-08：明确 Agent 操作 Blackboard 的主路径应是 bb CLI；MCP 保留为可选增强能力
- 2026-05-08：bb_server 新增 daemon 子命令原型，先以 OpenCode bb-pm 验证本地 Agent 可被拉起执行 handoff
- 2026-05-08：daemon 增加 .bb_runtime 状态记录与 dry-run/once/project/retry/max-dispatch 等试点参数
- 2026-05-08：通过 daemon 注入受控工具，验证 Agent 可走结构化操作而不是裸文件删除

- 2026-05-08：重构 bb daemon 从硬编码 inbox prompt 改为 agents/tasks.toml 注册表驱动模型
- 2026-05-08：daemon CLI 新增 `--task` 参数，默认 `inbox-cleanup`，支持 `ticket-audit` 等任意注册 task
- 2026-05-08：实现 `inbox-cleanup`（watch 触发）和 `ticket-audit`（manual 触发）两条流水线，均通过 dry-run 和真实执行验证
- 2026-05-08：扩展 BBPM prompt 允许（有明确证据时）更新 ticket status 和 assignee
- 2026-05-08：注册 opencode/claude 为全局 platform agent 并分配到 blackboard project
- 2026-05-08：daemon.rs 的 `--task` + prompt 模板架构为后续添加 dependency-sync 等任意流水线铺平道路

- 2026-05-09：修复 Codex app-server client 初始化自旋问题，根因是 request() 从 pending_notifications 读旧消息再塞回队列导致 thread/start response 等待时自旋；修复方案：request 等待阶段直接读 stdout 新消息并即时响应 app-server 主动发起的 approval/server request；设置 approvalPolicy 为 never 避免 Computer Use 工具调用进入审批等待态；增加 turn lifecycle 防护，只有收到 turn started 后才把 thread/status/changed idle 当成 turn 完成

# 记录

- 需求来源：当前对话调研 Multica/Slock.ai 后，决定在 Blackboard 内实现同类本地 daemon Agent 工具能力

- 来源：inbox/2026-05-08-codex-bb-daemon-bb-pm-prototype.md

- 来源：inbox/2026-05-08-codebuddy-daemon-task-registry.md

# 下一步

- 设计 bb daemon 的 Agent CLI 检测、task queue claim/heartbeat、per-task workdir 与 per-task home
- 设计 Codex app-server / Claude / OpenCode 的 runtime adapter，避免把任一 Agent 当作一次性 prompt 黑盒
- 设计 bb CLI 的 ticket get/update/comment/list，让 Agent 操作工单不依赖 MCP
- 保留 MCP 作为可选增强，不再作为 Agent 接入 Blackboard 的唯一主路径
