+++
id = "000052"
lane = "bbt"
title = "Roadmap：内置 Agent Chat 与任务下发"
created_at = "2026-05-13"
updated_at = "2026-05-16"
status = "archived"
assignee = "codex"
+++

# 当前进展


- codex 开始执行：继续实现内置 Agent Chat：给 FAB 增加模型与变体选择，并把选择透传到 OpenCode 会话创建。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-内置-agent-chat-支持模型与变体设置.md`。

- codex 开始执行：修正内置 Agent Chat 的连续对话语义：Chatbox 需要复用同一个 OpenCode provider session，而不是每轮看起来完全断开。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-agent-chat-续接-opencode-provider-session.md`。

- codex 开始执行：按参考图重做内置 Agent Chat 输入区，进一步使用 TDesign ChatSender 的组合能力，收敛模型/变体设置到输入框底部工具条。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-tdesign-chatengine-自定义渲染能力调研.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-agent-chat-fab-输入区改为-tdesign-chatsender-工具条.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-agent-chat-fab-streaming-thinking-ui.md`。

- codex 开始执行：补 OpenCode provider 的 reasoning/thinking 事件映射，让 Agent Chat FAB 能真实显示 thinking 过程。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-opencode-provider-thinking-event-mapping.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-14-codex-agent-chat-project-cwd.md`。

- 2026-05-14：给 AgentChatFab 增加模型/变体设置区，扩展 AgentSession API 支持 model/variant，补充前端网络错误提示和 HTTP 测试

- 2026-05-14：调研 TDesign ChatEngine 自定义渲染路径：useAgentToolcall+ToolCallRenderer / useAgentActivity+ActivityRenderer，ChatSender slots 可用于 Agent/模型/变体工具条

# 记录

- 近期 roadmap 调整：Blackboard 需要补齐自身 Agent 能力，在系统内支持直接 chatting 和任务下发，而不是只能通过 TaskGraph 间接触发。
- Chat 需要建立在 Agent Profile + AgentSession 之上：用户选择 agent/profile，发起会话，持续接收结构化事件、usage 和 tool 结果。
- 任务下发要能连接 tickets、inbox、TaskGraph 和 AgentSession，让用户可以把一次 chat、一个 ticket 或一个小任务交给合适的 Agent 执行。

- 验证：bb_list_projects/find_work_context: passed，定位到 blackboard ticket 000052。
- 验证：bb_list_agents: passed，确认 codex 为 active assignable agent 后开始 ticket work。
- 验证：cargo check: passed。
- 验证：cargo test -p bb_cli agent_session_create_rejects_blank_prompt: passed。
- 验证：npm run build --prefix bb_web: passed（仅 Vite 既有大 chunk warning）。
- 验证：8060 代理 POST /api/projects/devkit/agent-sessions 带 variant=high: passed，session.variant=high，OpenCode 输出 VARIANT OK。

- 验证：bb_find_work_context: passed，继续使用 blackboard ticket 000052。
- 验证：bb_begin_ticket_work: passed，ticket 回到 in_progress 并记录本轮修正。
- 验证：cargo check: passed。
- 验证：npm run build --prefix bb_web: passed（仅既有 Vite 大 chunk warning）。
- 验证：cargo test -p bb_cli agent_session_create_rejects_blank_prompt: passed。
- 验证：8060 代理真实两轮 OpenCode 对话: passed；两轮 as-* 不同，但 provider session 相同，第二轮正确回答上一轮暗号 BBCHAT42。

- 验证：通过：打开官方页面 https://tdesign.tencent.com/chat/components/chat-engine 并查阅 GitHub 源文档。
- 验证：通过：本地检查 bb_web/node_modules/@tdesign-vue-next/chat/esm/chat-engine 与 tdesign-web-components/lib/chat-engine 类型定义。
- 验证：未运行构建：本轮只做组件能力调研，未修改源码。

- 验证：通过：npm run build --prefix bb_web。
- 验证：通过：in-app browser 打开 devkit/settings 的 Agent Chat FAB，确认输入区出现 TDesign sender 内的引用状态、Agent/模型/变体工具条，并确认模型弹窗可打开。
- 验证：部分受限：最终一次刷新后的 in-app browser 截图通道出现 CDP screenshot timeout；已用构建和前一次可视检查覆盖主要风险。

- 验证：通过 `npm run build`（bb_web，vue-tsc + vite build）。
- 验证：通过临时 headless Chrome DOM 冒烟：localhost:8060 的 devkit settings 页 FAB 可打开，panel/message/sender/Agent-模型-变体控件存在，控制台仅有 Vite debug 日志。

- 验证：通过 `cargo test -p bb_core opencode_events_persist_status_text_tool_usage_and_error`。
- 验证：通过 `cargo fmt`。
- 验证：再次通过 `cargo test -p bb_core opencode_events_persist_status_text_tool_usage_and_error`。

- 验证：bb_list_projects/list_projects: passed，确认可见 project 包含 blackboard 与 devkit，devkit repos 指向 C:\Dev\Devkit。
- 验证：bb_find_work_context: passed，定位 blackboard 活跃 ticket 000052；bb_search_tickets: passed，无特定 path bug 既有命中。
- 验证：cargo test -p bb_cli direct_chat_execution_root: passed，3 个路径解析单测通过。
- 验证：cargo test -p bb_core -p bb_cli: passed，bb_cli 74 tests 与 bb_core 237 tests 全部通过。
- 验证：python scripts\check_ticket_ids.py --project blackboard: passed。

- 来源：inbox/2026-05-14-codex-agent-chat-续接-opencode-provider-session.md

- 来源：inbox/2026-05-14-codex-opencode-provider-thinking-event-mapping.md

- 来源：inbox/2026-05-14-codex-内置-agent-chat-支持模型与变体设置.md
- 代码位置：bb_web/src/components/AgentChatFab.vue, bb_backend/crates/bb_cli/src/http/agent_sessions.rs, bb_backend/crates/bb_core/src/agent_session/

- 来源：inbox/2026-05-14-codex-tdesign-chatengine-自定义渲染能力调研.md
- 文档位置：bb_web/node_modules/@tdesign-vue-next/chat/, https://github.com/Tencent/tdesign-vue-next/blob/develop/packages/pro-components/chat/chat-engine/chat-engine.md

# 下一步

- 定义 Direct Chat 的最小产品形态：入口、Agent/Profile 选择、session 创建、消息流、历史读取和取消。
- 设计任务下发模型：从 ticket/inbox/task graph/chat 创建 AgentSession 或 TaskGraph run 的边界。
- 明确 UI 信息架构：Chat workspace、ticket 内下发入口、Agent run history 与 structured timeline 的复用关系。

- 若继续实现，先保留现有 OpenCode AgentSession REST/SSE 通道，把前端消息结构改成 TDesign ChatMessage/ChatEngine typed content。
- 在输入区先使用 ChatSender slots 重做参考图样式；随后把工具调用/任务进度用 ActivityRenderer 或 ToolCallRenderer 接入。

- 下一步可把消息列表从 ChatItem 字符串渲染切到 TDesign ChatMessage typed content，为 toolcall/activity 自定义渲染铺路。
- 如果要更像宽屏参考图，可给 FAB 支持可拉宽/全屏模式，让 Agent/模型/变体始终单行展示。

- OpenCode provider 当前主要解析 text/tool/status/error；如果要真实看到 thinking，需要继续把 OpenCode 原始事件中的 reasoning/thinking 信息映射为 AgentEventType::Thinking。

- 在真实 OpenCode 模型会产生 reasoning 的配置上跑一次端到端 FAB 对话，确认前端 ChatMessage thinking segment 可见。

- 重启/刷新后端后，在 Devkit project 的 OpenCode Chat 里重新询问本地修改，预期显示 C:\Dev\Devkit 的 git 状态而不是 C:\Dev\blackboard。
