# OpenCode provider 补齐 reasoning/thinking 事件

时间: 2026-05-14T09:25:53Z
来源: codex
项目: blackboard

## 做了什么

- OpenCode AgentSession 启动命令增加 `--thinking`，因为 OpenCode 只有开启 thinking 后才会在 `run --format json` 中 emit `reasoning` 事件。
- OpenCode provider parser 新增 `reasoning` / `thinking` 事件处理，将 part.text/thinking/content/summary 中的非空文本写入 `AgentEventType::Thinking`。
- OpenCode provider 日志输出新增 Reasoning 摘要，便于 artifact/log 中追踪。
- 补充 OpenCode provider 单测：JSONL 中出现 `reasoning` 后会持久化 Thinking 事件。

## 验证了什么

- 通过 `cargo test -p bb_core opencode_events_persist_status_text_tool_usage_and_error`。
- 通过 `cargo fmt`。
- 再次通过 `cargo test -p bb_core opencode_events_persist_status_text_tool_usage_and_error`。

## 下一步

- 在真实 OpenCode 模型会产生 reasoning 的配置上跑一次端到端 FAB 对话，确认前端 ChatMessage thinking segment 可见。

## 相关位置

- bb_backend/crates/bb_core/src/agent_session/runtime.rs
- bb_backend/crates/bb_core/src/agent_session/providers/opencode.rs
