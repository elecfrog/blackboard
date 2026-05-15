# 调研 TDesign ChatEngine 自定义渲染能力

时间: 2026-05-14T08:48:15Z
来源: codex
项目: blackboard

## 做了什么

- 查阅 TDesign ChatEngine 官方文档与本地已安装的 @tdesign-vue-next/chat/tdesign-web-components 类型定义。
- 确认 ChatEngine 不只是输入框组件：包含 useChat、ChatEngine 消息状态、事件总线、默认/AG-UI 协议、toolcall/activity 注册与统一渲染。
- 确认自定义渲染推荐路径是 useAgentToolcall + ToolCallRenderer、useAgentActivity + ActivityRenderer，而不是在消息列表里零散手写判断。
- 确认 ChatSender 仍适合承载参考图里的输入区：input-prefix、footer-prefix、suffix、attachmentsProps、loading/stop 可直接用于 Agent/模型/变体工具条。

## 验证了什么

- 通过：打开官方页面 https://tdesign.tencent.com/chat/components/chat-engine 并查阅 GitHub 源文档。
- 通过：本地检查 bb_web/node_modules/@tdesign-vue-next/chat/esm/chat-engine 与 tdesign-web-components/lib/chat-engine 类型定义。
- 未运行构建：本轮只做组件能力调研，未修改源码。

## 下一步

- 若继续实现，先保留现有 OpenCode AgentSession REST/SSE 通道，把前端消息结构改成 TDesign ChatMessage/ChatEngine typed content。
- 在输入区先使用 ChatSender slots 重做参考图样式；随后把工具调用/任务进度用 ActivityRenderer 或 ToolCallRenderer 接入。

## 相关位置

- bb_web/node_modules/@tdesign-vue-next/chat/esm/chat-engine/hooks/useChat.d.ts
- bb_web/node_modules/@tdesign-vue-next/chat/esm/chat-engine/components/toolcall/types.d.ts
- bb_web/node_modules/@tdesign-vue-next/chat/esm/chat-engine/components/activity/types.d.ts
- bb_web/node_modules/tdesign-web-components/lib/chat-engine/type.d.ts
- https://github.com/Tencent/tdesign-vue-next/blob/develop/packages/pro-components/chat/chat-engine/chat-engine.md
