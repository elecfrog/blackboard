+++
id = "000051"
lane = "bbt"
title = "Roadmap：Runtime 连接与 AgentSession 维护"
created_at = "2026-05-13"
updated_at = "2026-05-17"
status = "archived"
assignee = "codex"
+++

# 当前进展


- codex 完成阶段工作，handoff 写入 `2026-05-13-codex-codex-agent-session-runtime.md`。

- agent 开始执行：Implement CodeBuddy CLI TaskGraph AgentSession runtime path.

- codex 完成阶段工作，handoff 写入 `2026-05-13-codex-codebuddy-cli-taskgraph-agentsession-runtime.md`。

- 2026-05-13：新增 CodeBuddy 作为 TaskGraph AgentSession runtime，支持 CLI path overrides 和 AgentTurnRequest 字段
- 2026-05-13：实现 CodeBuddy stream-json provider 规范化（status/text/thinking/tool_use/tool_result/usage_update/error/log 事件）
- 2026-05-13：生成 session-local CodeBuddy MCP config/settings 工件，含 bb MCP 注入、reasoningEffort variant 设置、Windows codebuddy.cmd 解析和 <bb-root> 扩展
- 2026-05-13：更新 TaskGraph LLM 执行/测试和前端 runtime 下拉验证以支持 codebuddy
- 2026-05-13：运行 project smoke graph codebuddy-agent-session-smoke-20260513-162752，AgentSession as-20260513-084724-33be1a1a 创建成功，状态 CODEBUDDY_GRAPH_OK

- 2026-05-16：将外部 CLI 运行时启动统一通过 crate::platform::resolve_spawn_program 解析可执行文件路径后再 Command::new；覆盖 AgentSession OpenCode/Codex/CodeBuddy runtime、legacy TaskGraph OpenCode runtime、Codex app-server client、agent tool npm/version 命令执行、shell node 命令执行；保留 opencode run --agent 不变

- 2026-05-16：实现 Windows only attach 路径处理 named OpenCode agents：启动临时 local opencode serve，等待 TCP 就绪后运行 opencode run --attach <url> --agent <agent>，执行完毕后拆除 server；保留早期 Windows native executable resolver 处理 npm/scoop shims

- 2026-05-16：为 opencode/codex/codebuddy 的 AgentTurnRequest runtime environments 添加 BB_SCRIPTS_DIR；新增 RunnerOptions.scripts_dir 和 resolve_scripts_dir 支持 .bb_template dev roots 和 .bb packaged/user roots；新增 {{env.scripts_dir}} prompt 渲染并规范化路径为 forward slashes；扩展 Workspace::init_from_seed 在 scripts 可用时复制到目标 .bb workspace

- 2026-05-16：新增 .bb_template/agents/CODEX.md 作为 Codex 专用规则事实源；在 agents_config 中为 AgentConnectorTargetSpec 增加 source_template 解耦 connector 源文件和目标文件名；Codex connector 的 source_template 指向 <bb-root>/agents/CODEX.md，target 仍为 ~/.codex/AGENTS.md；inspect_connector 的 connector-level source hash 改用 primary target 实际 source

- 2026-05-16：实现 backend agent_tools domain，静态版本锁定 OpenCode(opencode-ai@1.15.0)/Codex(@openai/codex@latest)/CodeBuddy(@tencent-ai/codebuddy-code@latest)，新增 GET /api/agents/tools 和 POST /api/agents/tools/{id}/install endpoints，npm 白名单仅允许静态 spec

- 2026-05-16：实现 Windows spawn resolver，通过 resolve_spawn_program 解析 npm/scoop shims 到原生 exe，保留 --agent bb-pm 标志；同时移除 oh-my-openagent 插件并降级 OpenCode 至 1.15.0 解决 InstanceRef not provided 问题

# 记录

- tickets 系统强化暂时放轻，优先建设 Runtime 连接、Session 维护和结构化事件底座。
- 第一优先级 runtime 是 OpenCode 和 Codex；后续 Claude Code / Gemini 预留扩展点，具体接入可交给开源社区。
- 目标是让 AgentSession 成为独立于 TaskGraph 的 runtime/session 管理层，承接实时事件、resume、usage、tool replay 和 provider event normalization。

- 验证：cargo test --manifest-path bb_backend/Cargo.toml agent_session passed.
- 验证：cargo test --manifest-path bb_backend/Cargo.toml task_graph passed.
- 验证：npm run build --prefix bb_web passed.
- 验证：Real Codex TaskGraph smoke with gpt-5.4-mini + low persisted status/text/usage_update events.

- 验证：cargo test --manifest-path bb_backend/Cargo.toml agent_session: passed.
- 验证：cargo test --manifest-path bb_backend/Cargo.toml task_graph: passed (106 tests; existing unused-variable warning in validation/pre_run.rs).
- 验证：cargo test --manifest-path bb_backend/Cargo.toml codebuddy: passed (15 tests).
- 验证：npm run build --prefix bb_web: passed with existing Vite chunk-size warning.
- 验证：git diff --check on touched CodeBuddy/TaskGraph/frontend files: passed; only CRLF normalization warnings.
- 验证：Real CodeBuddy direct captures: pure text, PowerShell tool call, and bb MCP list_projects all succeeded before TaskGraph smoke.
- 验证：cargo build -p bb_cli default target was blocked by running bb.exe lock; rebuilt with --target-dir bb_backend/target-codebuddy-smoke and removed that temporary target after smoke.

- 来源：inbox/2026-05-13-codex-codebuddy-cli-taskgraph-agentsession-runtime.md
- 相关位置：bb_backend/crates/bb_core/src/agent_session/providers/codebuddy.rs
- 相关位置：bb_backend/crates/bb_core/src/task_graph/llm.rs
- 相关位置：bb_backend/crates/bb_core/src/task_graph/node_exec/runtime_nodes/llm_node.rs

- 来源：inbox/2026-05-13-codex-roadmap-todo-tickets-for-runtime-session-and-built-in-agent-capabilities.md

- 来源：inbox/2026-05-16-codex-standardize-spawn-program-resolution.md
- 代码位置：bb_backend/crates/bb_core/src/platform.rs, bb_backend/crates/bb_core/src/agent_session/runtime.rs, bb_backend/crates/bb_core/src/task_graph/runtime/mod.rs, bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs, bb_backend/crates/bb_cli/src/codex_client.rs, bb_backend/crates/bb_core/src/agent_tools.rs

- 来源：inbox/2026-05-16-codex-opencode-windows-taskgraph-run-agent-failure-correction.md
- 代码位置：bb_backend/crates/bb_core/src/opencode_process.rs, bb_backend/crates/bb_core/src/agent_session/runtime.rs, bb_backend/crates/bb_core/src/task_graph/runtime/mod.rs, bb_backend/crates/bb_core/src/platform.rs, bb_backend/crates/bb_core/src/lib.rs

- 来源：inbox/2026-05-16-codex-taskgraph-windows-scripts-dir.md
- 代码位置：bb_backend/crates/bb_core/src/agent_session/runtime.rs, bb_backend/crates/bb_core/src/task_graph/pregel/runner.rs, bb_backend/crates/bb_core/src/task_graph/nodes/eval.rs, .bb_template/agents/prompts/task-graph-inbox-cleanup.md

- 来源：inbox/2026-05-16-codex-codex-connector-specialized-codex-md-source-implementation.md
- 代码位置：.bb_template/agents/CODEX.md, bb_backend/crates/bb_core/src/agents_config/model.rs, bb_backend/crates/bb_core/src/agents_config/mod.rs, bb_backend/crates/bb_core/src/agents_config/tests/mod.rs

- 来源：inbox/2026-05-16-codex-agent-cli-npm-install-and-version-lock-implementation.md
- 相关位置：bb_backend/crates/bb_core/src/agent_tools.rs, bb_cli/src/http/mod.rs, bb_web/src/components/AgentConnectorRow.vue, bb_web/src/components/AgentConnectorPanel.vue, bb_web/src/data/agentTools.ts

- 来源：inbox/2026-05-16-codex-taskgraph-windows-runtime-spawn.md
- 来源：inbox/2026-05-16-codex-taskgraph-windows-runtime-spawn-1.md
- 来源：inbox/2026-05-16-codex-opencode-windows-taskgraph-run-agent-failure.md
- 来源：inbox/2026-05-16-codex-opencode-windows-spawn-resolver-minimal-fix.md
- 来源：inbox/2026-05-16-codex-opencode-1-15-0-taskgraph-failure-diagnosis.md

- 来源：inbox/2026-05-16-codex-codex-connector-specialization-source-filename-research.md
- 研究结论：OpenAI Codex 官方文档未将 CODEX.md 列为默认指令文件名；建议 Blackboard 维护 agents/CODEX.md 作为源，connector 同步到 ~/.codex/AGENTS.md
- 相关位置：.bb_template/agents/CODEX.md, bb_backend/crates/bb_core/src/agents_config/model.rs

- 来源：inbox/2026-05-16-codex-codex-delayed-blackboard-tool-usage-rule.md
- 更新 .bb_template/agents/CODEX.md 的 Codex 工具发现协议：默认先完成用户明确指定的本地工作，再进行 Blackboard 检索；延迟 bb 工具发现，等需要 handoff 或历史上下文时再触发

- 来源：inbox/2026-05-16-codex-opencode-oh-my-openagent-removal.md
- 移除 oh-my-openagent 全局插件：删除 active opencode.json 中的 plugin 入口和缓存目录；移除后 opencode run --agent 仍报 InstanceRef not provided，问题指向 OpenCode 1.15.1 而非插件本身

- 来源：inbox/2026-05-16-codex-opencode-spawn-windows-diagnosis.md
- 诊断 task-graph run spawn opencode 失败：确认 UseShellExecute=false 启动裸 opencode 失败；opencode.cmd 和真实 opencode.exe 可用；定位源码 REST task graph run 使用 build_runner_opts(..., None) 导致 opencode_path 解析问题；AgentSession spawn failure 路径缺少 session 失败态落盘兜底
- 代码位置：bb_cli/src/http/task_graph/runner_config.rs, bb_core/src/agent_session/runtime.rs

# 下一步

- 梳理 Runtime Provider 接口：启动参数、环境注入、MCP/skills 注入、事件解析、取消/超时和结果归一化。
- 补齐 OpenCode / Codex 两条 runtime 的能力矩阵，明确哪些走 AgentSession structured timeline，哪些暂留 legacy log。
- 设计 AgentSession resume / usage / tool replay 的持久化与 HTTP/SSE 读取边界。
