+++
id = "000019"
lane = "bbp"
title = "项目级 Wiki 系统：文件布局、上传与前端渲染"
created_at = "2026-05-08"
updated_at = "2026-05-08"
status = "archived"
assignee = "opencode"
+++

# 当前进展

- 背景：bb 目前每个项目下有 `tickets/` 和 `inbox/` 两类内容，缺少对“项目级知识沉淀（源码蒸馏 / 架构 wiki）”的承载；用户已在外部工作流产出，希望在 bb 项目侧同级引入。
- 目标结构（初稿，与 tickets/inbox 同级）：`projects/<project>/wiki/`，内部约定与 opencode wiki 对齐——根目录 `index.md` 作为入口，并可包含 `architecture/`、`modules/`、`topics/`、`references/` 等子目录，支持模块下再嵌套自己的 wiki（如 `modules/<sub>/index.md + modules/... + topics/...`）。
- 范围约束（本单只覆盖“看得见”的第一阶段）：文件上传 / 摆放、前端浏览与渲染；后端专属“QMD 系统级索引”与检索能力延后单独开单。

- 2026-05-08：实现 019 Wiki 系统，新增 bb_server wiki HTTP routes、bb_core wiki 模块、前端 WikiPanel 组件、前端路由与 Wiki Tab 集成

# 记录

- 现有项目布局样例（`projects/blackboard/`）：`__project__.json`、`__tickets__.json`、`__inbox__.json`、`tickets/`、`inbox/`；wiki 作为第三类同级资源引入，不改动既有两类。
- lane 选择：本单落在 `bbp`（产品）以定义“wiki 作为项目一等资源”的基线；后端/前端具体实现可作为后续子单在 `bbt`/`bbd` 展开。

- inbox/2026-05-08-opencode-project-wiki-system.md

- 2026-05-08：子单 #000020（后端）/ #000021（前端）/ #000022（集成）首轮实现验收未通过，由统管单 #000023 一次性擦屁股收尾。后端 wiki.rs + http.rs 重写、路径安全双保险、asset 端点补齐；前端 data/wiki.ts 拆分、WikiTreeItem 独立、WikiPanel 相对链接与 `<img>` 重写、`wikiEmpty` 空状态、wiki 路由改 `:path(.*)*`；cargo test -p bb_core -p bb_server 46/46 绿，pnpm -C bb_web run build 通过。详见 `tickets/000023-wiki-system-rework.md`。

# 下一步

- 定稿 wiki 目录契约：确认 `projects/<project>/wiki/index.md` 为必须入口；子目录 `architecture/` `modules/` `topics/` `references/` 为可选但命名约定，允许 `modules/<sub>/` 下再嵌套 wiki。
- 文件上传/落盘路径：约定通过“用户自有工作流在本地产出 → 直接拷贝 / 上传到 `projects/<project>/wiki/` 目录”的方式接入；本阶段不要求 bb 自己生成 wiki 内容，仅保证 bb 能识别与展示。
- 前端浏览：左侧按文件树展示 `wiki/` 目录结构，右侧渲染所选 `.md`；相对链接（如 `./modules/session.md`、`../index.md`）需在 wiki 内部正确跳转，图片等相对资源同域可达。
- 最小后端支持：提供只读接口列出 `projects/<project>/wiki/` 下的文件树 + 读取单个 `.md` 内容（参考现有 `inbox` / `ticket` 的 HTTP 路由风格）；MCP 工具层本期可不新增，必要时在后续单里补齐。
- 延后项（另开单）：系统级 QMD / 语义索引、跨 project wiki 搜索、从 tickets 链接到 wiki 段落、wiki 版本化策略、wiki 写入 / 上传 UI（本单假设由外部工作流产出后放入目录）。
