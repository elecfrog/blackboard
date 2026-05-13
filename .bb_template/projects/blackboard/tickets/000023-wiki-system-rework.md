+++
id = "000023"
lane = "bbt"
title = "Wiki 系统收尾：修复 #000019/#000020/#000021/#000022 的实现缺陷并端到端打通"
created_at = "2026-05-08"
updated_at = "2026-05-12"
status = "archived"
assignee = "opencode"
depends_on = "000020 000021 000022"
+++

# 当前进展

- 019/020/021/022 四单验收未通过：实现能编译，但端点路由、路径安全、前端树路径、上传语义、空状态体验等多处偏离子单契约，需要一次性擦干净。
- 本单作为统管单（lane=bbt，因主要缺陷落在后端 + 跨前后端契约），前端侧的修复一并在本单里完成后再批量 review，不再拆子单以免继续散。
- 完成后 019/020/021/022 一起关单；022 当前状态是 done 但实际有 bug，本单完成前先把 022 状态回退到 review。

- agent 开始执行：开始按 todo 执行：后端 wiki.rs 重写→ http.rs 路由拆分→测试→前端 data/组件/路由→冒烟→关单

- 2026-05-08 实现完成（待人工冒烟后统一推 done）：
  - 后端：`bb_core/src/wiki.rs` 重写（去掉 index.md 前置、统一 `validate_wiki_path` 双保险、新增 `read_wiki_asset` + `ensure_wiki_root`、`validate_wiki_path_for_upload` 面向上传复用）；`bb_server/src/http.rs` 路由拆成 `/wiki/tree`、`/wiki/file?path=`、`/wiki/asset?path=`、`/wiki/upload`，upload 首次自动建 wiki 目录；`InboxError` 不扩枚举，沿用 `InvalidInput`/`NotFound` 映射。
  - 测试：`bb_core` 新增 9 条 wiki 单测；`bb_server` `http/tests.rs` 新增 8 条 wiki 集成测试；`cargo test -p bb_core -p bb_server` 46/46 绿。
  - 前端：新建 `bb_web/src/data/wiki.ts`，从 `data/tickets.ts` 迁出 wiki 类型与函数；新建独立 `components/WikiTreeItem.vue`；重写 `components/WikiPanel.vue`（文件/目录点击分离、相对链接 segment-stack 解析、`<img>` 相对 src 重写到 asset 端点、空状态走 `wikiEmpty`、布局补 `min-width: 0`）；`main.ts` wiki 路由改 `:path(.*)*` 并在 `dashboardProps` 里把数组/字符串 path 归一；`i18n.ts` 新增 `wikiEmpty` zh/en。
  - 样例：`projects/blackboard/wiki/` 放 `index.md` + `architecture/overview.md`（mermaid）+ `modules/session.md`（代码块 + task list）+ `modules/context-management/index.md` & `diagram.svg` + `references/glossary.md`，覆盖 Mermaid、代码块、任务列表、嵌套相对链接、相对 svg 图片五个要点。
  - 构建：`pnpm -C bb_web run build` 通过（仅有非致命的 chunk-size 警告，属 mermaid / katex 本身体积）。

- 2026-05-08：收口 Mermaid 报错体验——解析失败只在 .mermaid-block 内显示 `Mermaid Diagram Syntax Error`，清理错误注入到页面主体的 stray SVG DOM

- 2026-05-08：收紧代码块 toolbar 视觉，语言标签与 Copy 合并为低对比标题栏控件；Copy 文本替换为图标按钮，保留可访问属性

# 记录

- 后端现况（`bb_backend/crates/bb_core/src/wiki.rs` + `bb_backend/crates/bb_server/src/http.rs`）：已有 `wiki_root` / `list_wiki_tree` / `read_wiki_content` / `wiki_upload_handler`；`WikiTreeResponse` 字段已对外稳定（`generated_at` + `tree: Vec<WikiTreeNode>`），不轻易改形状，只修语义。
- 前端现况：没有独立 `views/WikiView.vue`，也没有独立 `data/wiki.ts` —— 所有 wiki 代码塞进了 `components/WikiPanel.vue` + `data/tickets.ts`。BoardView 已按 `activeWorkspace === 'wiki'` 接了 `WikiPanel`，i18n 也已有 `wiki` / `wikiSubtitle` / `wikiSelectFile` / `upload` 等 key。
- 已识别的具体缺陷清单（=本单的修复目标，落到下面 next_step）。

- 来源：inbox/2026-05-08-codex-mermaid-error-block.md

# 下一步

- 【后端·路由冲突】`/api/projects/{project}/wiki/{*path}` 与 `/api/projects/{project}/wiki/tree` / `/api/projects/{project}/wiki/upload` 在 axum 0.8 的通配符匹配下存在歧义风险，并且把 `GET /wiki/anything` 全吞到 content handler。改为显式前缀：`GET /api/projects/{project}/wiki/tree`、`GET /api/projects/{project}/wiki/file` + query `?path=`（与 020 子单原始契约一致）、`POST /api/projects/{project}/wiki/upload`；同时新增 `GET /api/projects/{project}/wiki/asset?path=`（见下）。前端同步改。
- 【后端·空 wiki 契约】现在 `list_wiki_tree` 在 `wiki/` 目录不存在或缺 `index.md` 时都返回空 tree——‘缺 index.md 就当空’这条不符合 020/019 的契约（020 说‘空目录返回空 tree’，但存在其他文件时应该列出来；019 把 index.md 定为‘入口’，不是‘tree 存在的前提’）。修为：只要 `wiki/` 目录存在就递归列出；目录不存在返回空 tree；不再特判 index.md。
- 【后端·上传时 wiki 目录缺失】`wiki_upload_handler` 依赖 `wiki_root()` 返回 Some，但当前 `wiki_root()` 在目录不存在时返回 None → 首次上传永远 404。修为：上传端点在 wiki 目录不存在时自动 `create_dir_all(projects/<project>/wiki)` 再写入。
- 【后端·上传路径安全】当前只做字符串级 `contains("..")` + `starts_with('/')` 校验，绕过面大（例如 `a/..%2fb`、符号链接越界、Windows 下 `\\`、`C:`）。统一走 `validate_wiki_path`：normalize → `canonicalize(wiki_root)` + `canonicalize(target parent)` 并 `starts_with`，越界一律 400 `wiki_path_escape`；拒绝绝对路径/盘符/`..`/空段；上传文件仅允许 `.md` / `.markdown` / 常见图片（`.png .jpg .jpeg .gif .svg .webp`）/ `.pdf`，其他扩展名拒绝（与只读接口一致，防止把可执行物丢进 wiki）。
- 【后端·相对资源接口】按 020 next_step 补齐 `GET /api/projects/{project}/wiki/asset?path=`：用于渲染 wiki 内相对图片/svg，按扩展名嗅探 `Content-Type`（png/jpg/svg/gif/webp/pdf → 正确 mime，其他 → `application/octet-stream`），路径安全复用同一函数。
- 【后端·content 端点语义】现在 content handler 从 path 参数取文件、同时也会命中 `tree`/`upload` 等子路径；切到 `GET /wiki/file?path=` 后，handler 改走 axum `Query<{ path: String }>`，并在 handler 入口统一 `validate_wiki_path`。
- 【后端·错误码映射】`InboxError::NotFound` 当前给的是 wiki 目录不存在的场景，混在 404 里看不出语义；新增一个语义清晰的 `InboxError::InvalidInput("wiki path escapes...")` → 400，并在 `http.rs::ApiError` 的 match 中为 wiki 越界场景也能落到 `bad_request`（目前其实已经通过 InvalidInput 映射，但需单测覆盖）。
- 【后端·测试】在 `bb_backend/crates/bb_server/src/http/tests.rs` 与 `bb_core/src/tests.rs` 追加：(a) 空 wiki 目录 → tree 为空；(b) 只有非 md 文件（如 `a.png`）→ tree 列出 png 节点为 file，但 `file?path=a.png` 返回 400 not_readable（或改为允许也列 md/markdown，二选一并固定）；(c) 嵌套 `modules/context-management/index.md` 路径在 tree 中为相对 POSIX 路径；(d) `..`、`C:\\`、`/etc/passwd` 一律 400；(e) upload 首次自动建目录；(f) asset 端点返回正确 mime。
- 【前端·数据层拆分】从 `bb_web/src/data/tickets.ts` 把 wiki 相关的类型（`WikiTreeNode` / `WikiTreeResponse` / `WikiContentResponse` / `WikiUploadResponse`）和函数（`loadWikiTree` / `loadWikiContent` / `uploadWikiFiles`）迁出到新文件 `bb_web/src/data/wiki.ts`，仅保留跨文件必要的 re-export；调用方改走 `@/data/wiki`。理由：021 子单明文要求独立 `data/wiki.ts`，而且 tickets.ts 正在膨胀。
- 【前端·loadWikiContent URL 匹配后端】后端切到 `?path=` query 后，前端改为 `fetchJson(\`/api/projects/${p}/wiki/file?path=${encodeURIComponent(path)}\`)`；不再用 `encodeURIComponent` 把整个路径做 URL segment（原来的 `wiki/${safePath}` 在 `/` 被编码成 `%2F` 后会被路由解成单段，导致嵌套路径读取出错）。
- 【前端·树路径处理】`WikiTreeItem` 当前在目录节点被点击时既 toggle 又 emit select，造成父级点击触发 navigateToChild/路由 replace —— navigateToChild 内部只处理 file，所以路由不变，但 `selected-path` 高亮会串到父目录。规范：目录点击仅 toggle + 不 emit select；文件点击 emit select。
- 【前端·相对链接解析】`handleContentLinkClick` 的 `../` 路径解析有 bug：`href.replace(/\.\.\//g, '')` 会把中间的 `../` 也清掉，但 `basePath` 只按 `upCount` 回退，计算 `parts.length - upCount` 未防越界也未在剩余路径首段再消化二次 `..`。重写：用 URL 构造器或循环 split/解 `.`/`..`，并限制解析结果必须落在 wiki 根内。
- 【前端·图片相对资源】`MarkdownRenderer` 渲染的 `<img src="./foo.png">` 当前会按相对 URL 走当前页面路径请求，404。增加 img src 重写：在 WikiPanel 里对渲染后的 DOM（或用 `MarkdownRenderer` 提供的钩子，如无则 `onMounted` + `watch(wikiContent)` 里遍历 `.bb-wiki-document img`）把相对 src 改写成 `/api/projects/${project}/wiki/asset?path=<resolvedPath>`。
- 【前端·空目录引导】当 `wikiTree.length === 0` 时，把提示改成‘引导用户上传或把外部工作流产物拷入 projects/<project>/wiki/ 目录’，而不是复用 `inboxEmpty`。i18n 新增 `wikiEmpty` key（zh/en）。
- 【前端·布局顺序】`WikiPanel` 的模板结构是 `<main class=bb-wiki-content>` 里同时放 `<article>` 和右侧 `<aside>`，但 css 里 `.bb-wiki-content` 没有定义其子元素宽度也没有 `min-width: 0`，长代码块会把 TOC 挤出可视区。调整为 flex 布局：`.bb-wiki-document { flex: 1; min-width: 0; }`、`.bb-wiki-toc { width: 220px; flex-shrink: 0; }`；`bb-wiki-content` 用 `flex; gap: 1rem; overflow: hidden`。
- 【前端·WikiTreeItem 复用】当前 `WikiTreeItem` 用 `defineComponent` 放在单独的 `<script lang="ts">` 里，再导出 `components: { WikiTreeItem }`，属于 SFC 双 script 混用容易在 vue3 + vite 下踩坑（尤其 `defineModel` 需要宏环境）；抽为独立文件 `components/WikiTreeItem.vue` 走正常 `<script setup>` 写法。
- 【前端·路由 path 参数】`main.ts` 里 `/projects/:project/wiki/:path` 的 `:path` 是单段，不支持 `modules/foo/index.md`。改为 `/projects/:project/wiki/:path(.*)*` 通配，并在 `dashboardProps` 里把 `route.params.path` 处理成 `string | undefined`（数组时 `join('/')`）。
- 【前端·首次进入自动选中】当前只在 tree 顶层找 `index.md`；没找到就停在空白中栏。补一条：若 URL 带 `path` 则优先 selectPath(path)；否则优先 `index.md`；再否则选择 tree 里第一个文件节点。
- 【冒烟】在 `projects/blackboard/wiki/` 放一份两层的样例（如 `index.md` + `modules/context-management/index.md` + `modules/context-management/diagram.svg`），手动跑一轮：进入 Wiki → tree 正确 → 选中 index.md → 正文/TOC → 点 `./modules/context-management/` → 正确路由到子页 → 子页内图片能被 asset 端点返回。
- 【状态收尾】本单合入后，把 019/020/021/022 的 status 改为 `done`；在 019 上附一条完成记录指向本单；022 在开始修复前先改回 `review`。
