# Blackboard Wiki · 项目知识库入口

> 这是 `.bb/projects/blackboard/wiki/` 的入口文档，作为 `#000023` Wiki 系统收尾单的端到端冒烟样例而存在，也可作为其他项目建立 Wiki 的最小参考。

## 导航

- [architecture/overview.md](./architecture/overview.md) — 架构总览（含一张 Mermaid 流程图）
- [architecture/task-graph-mvp-contract.md](./architecture/task-graph-mvp-contract.md) — Task Graph MVP 前后端契约
- [architecture/task-graph-mvp-e2e-validation.md](./architecture/task-graph-mvp-e2e-validation.md) — Task Graph MVP 端到端验收记录
- [modules/context-management/index.md](./modules/context-management/index.md) — 嵌套 wiki 示例（目录内部再含 `index.md` + 相对图）
- [modules/session.md](./modules/session.md) — 单文件模块示例
- [references/glossary.md](./references/glossary.md) — 术语与引用

## 约定

| 目录 | 用途 |
| --- | --- |
| `architecture/` | 跨模块的架构、数据流、部署图 |
| `modules/` | 单模块或子模块的蒸馏文档，允许嵌套 `modules/<sub>/index.md + ...` |
| `topics/` | 横切主题（比如“错误处理策略”“权限模型”） |
| `references/` | 外部资料引用、术语、变更日志 |

## 构建/上传约束

- 本期由外部蒸馏工作流产出 `.md`（及相对 `.svg` / `.png` 资源），通过右上角的 **上传** 按钮或拷贝的方式放入 `.bb/projects/<project>/wiki/`。
- 服务端拒绝 `..` / 绝对路径 / Windows 盘符 / 非白名单扩展名（仅允许 `md/markdown/txt/png/jpg/jpeg/gif/svg/webp/pdf`）。
- 前端对相对 `<img src>` 会自动改写到 `/api/projects/<project>/wiki/asset?path=...`。
