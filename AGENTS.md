# Blackboard

Blackboard 是一个本地多 Agent 协作工作台，将 Codex、CodeBuddy、OpenCode 等 AI 编程工具的上下文（project、ticket、inbox、wiki）统一聚合管理。

## 技术栈

### 后端 (`bb_backend`)

基于 Rust 构建的高性能后端服务，负责：
- 多 Agent 上下文管理与聚合
- Project、Ticket、Inbox、Wiki 数据模型与 API
- 与 Codex/CodeBuddy/OpenCode 等工具的数据同步

### 前端 (`bb_web`)

基于 Vue 构建的 Web 界面，提供：
- 统一的上下文浏览与搜索
- Project、Ticket、Inbox、Wiki 可视化展示
- 响应式设计，支持多视图切换

### 桌面端 (`bb_desktop`)

基于 Tauri 构建的跨平台桌面应用，特性：
- 原生桌面体验，系统托盘集成
- 本地数据存储，离线可用
- 轻量级打包

## 快速运行

```
python3 scripts/dev.py
```