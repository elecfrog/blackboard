# Blackboard

Blackboard 是一个本地多 Agent 协作工作台，把 Codex、CodeBuddy、OpenCode 等工具的 project、ticket、inbox、wiki 和 Agent 上下文收敛到同一个地方。

## Quick Start

```bash
python3 scripts/setup.py
python3 scripts/dev.py
python3 scripts/desktop.py
```

## 功能

### Projects

[Projects 图片占位]

- 按项目隔离 tickets、inbox、wiki、lanes 和工作流。
- 每个项目都有自己的协作上下文，适合多 repo / 多产品并行推进。

### Tickets

[Tickets 图片占位]

- 支持 Kanban、依赖图、列表三种视图。
- 用统一状态追踪任务从待办、执行、验收到完成的流转。

### Inbox

[Inbox 图片占位]

- 给 Agent 和人类留下轻量交接。
- 适合记录阶段结果、验证信息和需要后续整理的上下文。

### Wiki

[Wiki 图片占位]

- 浏览项目文档树并预览 Markdown。
- 支持上传文件和文件夹，把项目知识沉淀在同一个工作台里。

### Agents

[Agents 图片占位]

- 统一管理 Agent registry、运行时配置和组织关系。
- 通过连接器同步 Blackboard 规则和 MCP 配置到本机 Agent 工具。

### Task Graphs

[Task Graphs 图片占位]

- 查看、编辑、运行多 Agent 工作流。
- 支持运行历史、人类 gate 和任务取消。

### Desktop

[Desktop 图片占位]

- 用 Open Folder 把本地代码目录接入 Blackboard。
- 桌面端自动启动本地 sidecar，让普通项目也能使用同一套协作工作台。

## References

Blackboard 的很多设计思路来自社区已有项目和框架。

- [Multica](https://github.com/multica-ai/multica)：Agent session / Runtime connection / Structured events
- [OpenCode](https://github.com/anomalyco/opencode)：Blackboard 当前大量 Agent 执行、tool call、JSON event 和本地 runtime 接入都参考并依赖 OpenCode 生态。
- [Google ADK JS](https://github.com/google/adk-js)：Agent 编排、session、tool 和 runtime 分层是 Blackboard 设计 Graph + Agent + LLM 三层模型时的重要参考。


