# earendil-works/pi 调研

调研日期：2026-05-21  
对象：<https://github.com/earendil-works/pi>  
本地源码快照：`4868222e3414554987bf4b05fbb393fc65080aa0`

## Executive Finding

Pi 是一个 TypeScript/Node.js 的 agent harness monorepo，核心定位不是 project/ticket 协作系统，而是“可自定义的终端 coding agent”。它把产品能力拆成四层：

- `@earendil-works/pi-coding-agent`：CLI、交互 TUI、print/JSON/RPC/SDK 模式、会话、资源加载、扩展和包管理。
- `@earendil-works/pi-agent-core`：agent 状态机、tool calling、事件流、steering/follow-up 队列、并行工具执行。
- `@earendil-works/pi-ai`：多 provider LLM 统一 API、模型发现、工具调用、thinking/reasoning、跨 provider handoff。
- `@earendil-works/pi-tui`：差分渲染的终端 UI 组件库。

对 Blackboard 的价值主要在“agent harness 可扩展层”和“会话/资源/工具协议”设计，而不是直接替代 Blackboard 的 project/ticket/inbox/wiki 协作模型。Pi 的哲学是最小核心，缺省不内置 MCP、sub-agent、plan mode、permission popup、todo、background bash；这些能力通过 TypeScript extensions、skills、prompt templates 或 pi packages 实现。

## 当前状态快照

- GitHub README 将仓库定义为 Pi Agent Harness Mono Repo，包含 coding agent、agent runtime、unified LLM API 和 TUI 库。
- GitHub 页面显示最新 release 为 `v0.75.4`，发布时间 `2026-05-20`；各 workspace package 的 `package.json` 也为 `0.75.4`。
- 根 `package.json` 要求 Node.js `>=22.19.0`，workspace 为 `packages/*` 以及若干 extension example。
- 仓库公开指标较高：GitHub 页面显示约 `52.1k` stars、`6.2k` forks、`4,227` commits。
- License 为 MIT。

## 功能模型

### 1. CLI 与运行模式

Pi 的主入口是 `pi` CLI，支持四类使用方式：

- Interactive：默认 TUI 交互体验。
- Print/JSON：`pi -p "prompt"` 或 `--mode json`，适合脚本和事件流消费。
- RPC：`pi --mode rpc`，通过 stdin/stdout JSONL 协议嵌入 IDE、外部 UI 或其他进程。
- SDK：从 `@earendil-works/pi-coding-agent` 直接创建 `AgentSession`，适合 Node.js 应用内嵌。

默认工具是 `read`、`write`、`edit`、`bash`；CLI reference 还暴露 `grep`、`find`、`ls` 等内置工具，可通过工具 allowlist 控制。

### 2. Session 与上下文

Pi 会将 session 存储为 JSONL 树结构。一个 session 文件内可以保留多个 branch，通过 `/tree` 回到历史节点继续，`/fork` 和 `/clone` 可以派生会话。长会话通过 compaction 保持上下文可用，完整历史仍保留在 JSONL 里。

这套模型适合 agent IDE/harness：它强调“会话可回放、可分叉、可导出、可分享”。Blackboard 的 ticket/inbox 更强调协作事实和任务状态，两者可以互补：Pi session 可作为 run artifact，Blackboard ticket/inbox/wiki 保留结构化协作上下文。

### 3. 资源加载

Pi 启动时加载：

- 全局和项目级 `AGENTS.md` / `CLAUDE.md`。
- `SYSTEM.md` 或 `APPEND_SYSTEM.md`。
- skills、prompt templates、themes、extensions。

资源位置同时支持全局 `~/.pi/agent/...` 和项目内 `.pi/...`。这和 Blackboard 的 project-scoped context 很贴近：资源发现最好是显式、分层、可热重载，而不是把所有上下文一次性塞进系统提示。

### 4. Extension 机制

Extension 是 Pi 最关键的设计。它是 TypeScript module，可以：

- 注册 LLM 可调用工具：`pi.registerTool()`。
- 注册 slash command：`pi.registerCommand()`。
- 监听生命周期、模型、tool call、用户输入等事件。
- 拦截或阻止工具调用，例如危险 bash confirmation。
- 注入动态上下文、替换/定制 compaction。
- 创建自定义 TUI 组件、status/footer/header/overlay/editor。
- 注册 provider、修改 active tools、切换模型。

Pi 文档明确提醒 extension 以用户权限执行任意代码，因此三方 package/extension 是高信任边界能力。

### 5. Agent Core

`@earendil-works/pi-agent-core` 是独立 agent runtime：

- `Agent` 维护 system prompt、model、thinking level、tools、messages、streaming state。
- `agentLoop`/`Agent.prompt()` 产生 `agent_start`、`turn_start`、`message_update`、`tool_execution_*`、`agent_end` 等事件。
- 工具执行默认可并行；单个工具可要求 sequential。
- `beforeToolCall` 可拦截工具，`afterToolCall` 可后处理结果或返回 terminate hint。
- 支持 steering/follow-up 队列：运行中用户消息可在当前工具批次结束后插入，或等 agent 完成后追加。

这对 Blackboard 的启发是：agent run state 与 UI/transport 可以分层；工具调用生命周期应有一等事件，而不是只保存最终文本。

### 6. LLM Provider 层

`@earendil-works/pi-ai` 统一多 provider API。文档列出的 provider 覆盖 OpenAI、Anthropic、Google/Vertex、Azure OpenAI、Mistral、Groq、Cerebras、Cloudflare、xAI、OpenRouter、Vercel AI Gateway、MiniMax、Together AI、GitHub Copilot、Amazon Bedrock、Kimi For Coding、Xiaomi MiMo、OpenAI-compatible API 等。

关键设计点：

- 只收录 tool-calling 模型。
- TypeBox schema 作为 tool 参数定义和校验基础。
- 标准化 streaming event：text、thinking、tool call、usage、stop/error。
- 支持 image input；image generation 是独立 API。
- 跨 provider handoff 会将不同 provider 的 thinking/tool/message 结构转换为兼容上下文。
- OAuth provider 覆盖 Anthropic subscription、OpenAI Codex、GitHub Copilot 等。

### 7. TUI 层

`@earendil-works/pi-tui` 是可独立复用的 terminal UI 包，特点包括：

- 差分渲染和 synchronized output，降低闪烁。
- Editor/Input/Markdown/SelectList/SettingsList/Image/Overlay 等组件。
- 支持 inline images、autocomplete、IME cursor marker、ANSI width/truncation 工具。

它对 Blackboard Web 前端没有直接复用价值，但对未来 Blackboard CLI/TUI 或 agent terminal 面板有参考价值。

## 与 Blackboard 的关系判断

Pi 与 Blackboard 的核心边界不同：

| 维度 | Pi | Blackboard |
| --- | --- | --- |
| 核心对象 | Agent session、tools、extensions、models | Project、ticket、inbox、wiki、agent 协作上下文 |
| 主要形态 | CLI/TUI/SDK/RPC agent harness | 本地多 agent 协作工作台 + Web/Desktop |
| 状态保存 | JSONL session tree | Project 化结构化数据与 BDD ticket |
| 扩展方式 | TypeScript extension / skill / prompt / package | MCP tools、backend API、frontend views、ticket/workflow |
| MCP | 默认不内置，建议用 CLI tools/README 或 extension 自建 | Blackboard MCP 是结构化协作入口 |

结论：Pi 适合被 Blackboard “接入/适配/借鉴”，不适合作为 Blackboard 的底座替换。

## 可借鉴点

1. **最小核心 + extension/package 生态**
   Blackboard 可以保持 project/ticket/inbox 权威模型稳定，把 agent-specific workflow、tool gate、provider bridge、view augment 作为插件/connector 扩展。

2. **ResourceLoader 分层**
   Pi 对 `AGENTS.md`、skills、prompts、themes、extensions 的分层加载值得参考。Blackboard 可以把 project wiki、ticket spec、inbox handoff、agent profile 做成明确的 context bundle，而不是隐式目录扫描。

3. **Session tree 作为 run artifact**
   Blackboard task graph / agent run 可以保存可分叉的 session tree 或事件流，再将关键事实沉淀为 ticket progress/inbox/wiki。这样既保留调试细节，也避免 ticket 变成流水账。

4. **工具调用事件化**
   Pi 的 `tool_execution_start/update/end` 与 `message_update` 事件可作为 Blackboard run observability 的参考。对长任务和并发 agent，事件流比最终文本更可诊断。

5. **RPC 与 SDK 双入口**
   Pi 对非 Node 进程提供 RPC，对 Node 应用提供 SDK。Blackboard 也可以类似地区分：HTTP/MCP 给外部 agent，Rust/TS SDK 给内部桌面/Web/CLI。

6. **Supply-chain hardening**
   Pi 对 npm 安装使用 `--ignore-scripts`、精确 pin 直接依赖、shrinkwrap CLI 发布依赖、检查 lockfile 变化。这对 Blackboard 若引入 agent plugin/package 安装非常重要。

## 风险与限制

- **Extension 安全边界高**：Pi extension 以用户权限执行任意代码，适合高信任本地环境；如果 Blackboard 引入类似机制，需要权限模型、审计和显式安装确认。
- **不内置 MCP**：Pi 明确不把 MCP 放进核心。若要接入 Blackboard MCP，需要写 Pi extension 或把 `bb` CLI/README 暴露为 skill。
- **Node/TypeScript 运行时假设强**：Pi 要求 Node `>=22.19.0`，对 Blackboard Rust/Vue/Tauri 架构不是直接同构。
- **会话不是协作事实源**：Pi session 很适合回放 agent 过程，但 ticket 状态、验收定义、交接责任仍需要 Blackboard 这类结构化模型。
- **默认少权限护栏**：permission gate、path protection、sandbox、sub-agent、plan mode 都不是内置核心能力；用 Pi 时要靠 extension 或外部容器补齐。

## Blackboard 可能的接入方案

### 方案 A：Pi skill 调用 Blackboard CLI

给 Pi 提供 `.pi/skills/blackboard/SKILL.md`，说明如何通过 `bb` CLI 查询 project/ticket/inbox/wiki，并要求写入走结构化命令。优点是简单，贴合 Pi “No MCP, use CLI tools with READMEs” 的哲学；缺点是工具 schema 和权限反馈不如 MCP 强。

### 方案 B：Pi extension 封装 Blackboard MCP/HTTP

写 TypeScript extension，注册 `bb_list_projects`、`bb_read_ticket_by_id`、`bb_create_inbox_note` 等工具，内部调用 Blackboard MCP HTTP JSON-RPC 或后端 API。优点是对 LLM 暴露结构化 tool schema；缺点是需要维护安全边界、session auth 和错误映射。

### 方案 C：Blackboard 记录 Pi session artifact

把 Pi JSONL session 或导出的 HTML/Gist 链接作为 Blackboard ticket/inbox/wiki attachment。优点是保留完整过程；缺点是需要提炼机制，防止长期事实散落在 session 里。

## 后续建议

1. 若目标是“让 Pi 参与 Blackboard 项目协作”，优先做方案 A 的 skill proof-of-concept，再评估是否升级为 extension。
2. 若目标是“改进 Blackboard agent run 体验”，优先借鉴 Pi 的 session tree、event stream、resource loader，而不是复制 TUI。
3. 若目标是“支持第三方 agent package”，先设计安全策略：安装来源、版本 pin、生命周期脚本禁用、权限声明、审计日志。
4. 可以单独调研 Pi 的 `packages/coding-agent/src/core/resource-loader.ts`、`agent-session.ts`、`packages/agent/src/agent-loop.ts`，提炼 Blackboard task graph/runtime 可复用的事件和上下文边界。

## Source Evidence

- GitHub repo / README: <https://github.com/earendil-works/pi>
- Pi docs overview: <https://pi.dev/docs/latest>
- Quickstart: <https://pi.dev/docs/latest/quickstart>
- Extensions docs: <https://pi.dev/docs/latest/extensions>
- SDK docs: <https://pi.dev/docs/latest/sdk>
- RPC docs: <https://pi.dev/docs/latest/rpc>
- Root package manifest: <https://github.com/earendil-works/pi/blob/main/package.json>
- Coding agent package manifest: <https://github.com/earendil-works/pi/blob/main/packages/coding-agent/package.json>
- Agent core package manifest: <https://github.com/earendil-works/pi/blob/main/packages/agent/package.json>
- AI package manifest: <https://github.com/earendil-works/pi/blob/main/packages/ai/package.json>
- TUI package manifest: <https://github.com/earendil-works/pi/blob/main/packages/tui/package.json>
- Contributing guide: <https://github.com/earendil-works/pi/blob/main/CONTRIBUTING.md>
- Project agent rules: <https://github.com/earendil-works/pi/blob/main/AGENTS.md>
