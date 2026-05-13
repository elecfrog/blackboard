+++
id = "000021"
lane = "bbd"
title = "Wiki 前端视图：一级导航 + 三栏浏览（文件树 / 正文 / TOC）"
created_at = "2026-05-08"
updated_at = "2026-05-12"
status = "archived"
assignee = "opencode"
depends_on = "000019"
+++

# 当前进展

- 父单 #000019 已把 `projects/<project>/wiki/` 定为第三类一等资源，后端子单 #000020 提供只读 tree/file/asset 三端点。本单在 bb_web 主站接入前端，让用户能在看板里像翻 inbox/ticket 一样翻 wiki。
- 目标形态：三栏布局（左=wiki 文件树 / 中=Markdown 正文 / 右=该文档 TOC），作为与 Board / Tickets / Inbox / Agents / Graph / Settings 并列的一级 section。

- 2026-05-08：WikiPanel.vue 实现三栏布局（文件树/正文/TOC），新增左侧边栏可拖拽调整宽度（120-600px）和折叠按钮，localStorage 持久化 treeWidth 和 treeCollapsed

# 记录

- 现有路由在 `bb_web/src/main.ts`：section 联合类型为 `'board' | 'tickets' | 'inbox' | 'agents' | 'graph' | 'settings'`，都挂在 `BoardView` 上并通过 `dashboardProps(section)` 传参；顶栏在 `components/AppNav.vue`，顶层页面 shell 在 `views/BoardView.vue`（34KB，section 切换由 `BoardView` 内部分发）。
- 父单显式声明：本期不做 wiki 的写入/上传 UI，也不做全局搜索；wiki 内容由用户的外部工作流拷入 `projects/<project>/wiki/`，前端只负责读取与渲染。

- inbox/2026-05-08-opencode-wiki-前端视图-一级导航-三栏浏览.md

- 来源：2026-05-11-codex-wiki-tree-inbox-style-visual-alignment.md

# 下一步

- 路由：在 `bb_web/src/main.ts` 的 `dashboardProps` section 联合里新增 `'wiki'`，并注册：\n  - `/projects/:project/wiki` → `BoardView` + `section='wiki'`\n  - `/projects/:project/wiki/*path`（通配嵌套路径）→ `BoardView` + `section='wiki'`，把 `path` 以 query 或 params 形式传入，用于在首屏直接定位到某个 `.md`。
- 顶层导航：在 `BoardView` 现有 section 切换处加入 `Wiki` 入口（和 Board/Tickets/Inbox/... 同级按钮），图标/文案走 `i18n.ts`（`nav.wiki` 等新 key），保持 zh/en 双份。
- 新组件：`components/WikiTree.vue`（左栏，递归渲染 `WikiTreeNode`，目录折叠、当前项高亮、按文件名字典序排序、目录排在文件前）、`components/WikiDocument.vue`（中栏，展示 `MarkdownRenderer` + 面包屑 + 当前文件的相对路径）、`components/WikiToc.vue`（右栏，封装 `TableOfContents`，随滚动高亮）。
- 数据层：`src/data/wiki.ts` 新增两个函数 `fetchWikiTree(project)` 与 `fetchWikiFile(project, path)`，对应后端 `/api/projects/{project}/wiki/tree` 与 `/api/projects/{project}/wiki/file?path=...`；沿用 `tickets.ts` 的 fetch 风格（`baseUrl` / 错误解码 / 类型导出）。
- 视图层：新增 `views/WikiView.vue`（也可直接在 `BoardView` 里按 `section==='wiki'` 分发以减少重复 shell 代码，择一即可，实现时统一）。空目录时显示提示（引导用户把外部工作流产物拷入 `projects/<project>/wiki/`，给出具体路径）。
- 相对链接与资源：`MarkdownRenderer` 渲染出的相对链接（`./modules/session.md`、`../index.md`）需要被改写为应用内路由跳转而不是浏览器原生导航——实现方式：给 `MarkdownRenderer` 传入一个 link 解析器 / 或在外层捕获点击并 `router.push`。图片/svg 等相对资源重写为 `/api/projects/{project}/wiki/asset?path=...`（依赖 #000020 的 asset 端点；若 #000020 推迟 asset，前端此处降级为按原样请求并在 README 说明限制）。
- 锚点：`.md` 内部的 `#section` 链接保持原行为（`TableOfContents` 已走 `slugifyHeading`），确保与 markdown-it-anchor 的 slug 一致。
- Mermaid / 代码高亮 / 任务列表：`MarkdownRenderer` 已内置；确认在主站 vite 构建下 mermaid 按需加载不炸 bundle（必要时做 dynamic import 验证）。
- 交互细节：左栏宽度可拖拽、折叠状态本地持久化（`localStorage` key 带 project 命名空间）；URL 始终反映当前 `.md` 路径，便于分享。