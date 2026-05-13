+++
id = "000020"
lane = "bbt"
title = "Wiki 只读后端：目录树与文件内容 REST 接口（bb_core + bb_server）"
created_at = "2026-05-08"
updated_at = "2026-05-08"
status = "archived"
depends_on = "000019"
+++

# 当前进展

- 父单 #000019 确定 wiki 作为 `projects/<project>/wiki/` 目录、与 tickets/inbox 同级、第一阶段仅需‘识别 + 展示’。本单交付这一阶段所需的最小后端能力：对 wiki 目录的只读访问。
- 设计原则沿用现有 inbox/ticket 的 HTTP 风格（见 `bb_backend/crates/bb_server/src/http.rs`）：`bb_core` 负责领域类型与文件系统访问，`bb_server` 只做薄 HTTP 包装；MCP 工具层本期不新增，留待后续单。
- 非目标（显式不做）：写入 / 上传接口、wiki 索引生成、跨项目搜索、语义检索、与 tickets 的反向链接——这些在父单已列为延后项。

# 记录

- 参考实现：`bb_core/src/inbox.rs`（`list_notes` / `read_note`、`InboxNoteEntry` / `InboxNote`）、`bb_server/src/http.rs` 中的 `list_notes` / `read_note` 路由与 `ApiError` 映射（`InboxError::NotFound` → 404、`InvalidInput` → 400 的统一错误语义）。
- 目录约束：wiki 根固定为 `projects/<project>/wiki/`；允许任意深度子目录（`architecture/` / `modules/` / `topics/` / `references/` 以及 `modules/<sub>/...` 嵌套）；仅暴露 `.md` 文件作为可读内容，其他扩展名（`.png` `.svg` 等相对资源）作为树节点列出但不经此接口读取（静态资源走独立静态映射，见下）。
- `projects/blackboard/__project__.json` 定义了 `lanes` 与 `board_view`；本单不动该文件，wiki 不产生新的元数据文件（第一阶段无需 `__wiki__.json`）。

# 下一步

- 在 `bb_core` 新增 `wiki.rs` 模块：定义 `WikiTreeNode { name, kind: "dir"|"file", path, children?: Vec<WikiTreeNode> }`（`path` 为相对 wiki 根的 POSIX 路径）与 `WikiFileContent { path, content }`；错误类型复用或扩展 `InboxError`（建议新增 `WikiPathEscape` / `WikiNotReadable` 变体，保持现有映射习惯）。
- `ProjectBoard`（`bb_core/src/project.rs`）增加两个方法：`list_wiki_tree() -> Result<WikiTreeNode>`（递归列出 `projects/<project>/wiki/`；目录不存在时返回一个 `kind=dir, children=[]` 的空根，不当作错误）、`read_wiki_file(path: &str) -> Result<WikiFileContent>`（只接受相对路径、拒绝 `..` / 绝对路径 / 符号链接越界、只读 `.md`）。
- 在 `bb_server/src/http.rs` 新增两个路由，风格与 inbox 对齐：\n  - `GET /api/projects/{project}/wiki/tree` → `WikiTreeResponse { generated_at, root: WikiTreeNode }`\n  - `GET /api/projects/{project}/wiki/file?path=<relpath>` 或 `GET /api/projects/{project}/wiki/files/{*path}` → `WikiFileContent`（建议用 query 参数形式避免 axum 通配符与中文/空格路径的转义纠结；最终实现二选一，写入 tests.rs 时同步明确）。
- 再新增 `GET /api/projects/{project}/wiki/asset?path=<relpath>`：用于 wiki 内相对图片/svg 等静态资源（`Content-Type` 按扩展名嗅探 png/jpg/svg/gif/webp/pdf，未知类型返回 `application/octet-stream`）；仍走同一套路径安全校验。若判断风险大，可本期先不做、让 wiki 作者在 markdown 里使用 data URI 或外链，但前端子单默认图片可渲染，建议本期带上。
- 路径安全：统一 normalize（拒绝 `\\`、`..`、空段、以 `/` 开头、长度 > 512）；解析后必须落在 `projects/<project>/wiki/` 实际 canonical 子树内（用 `canonicalize` + `starts_with` 双保险），越界返回 400 `wiki_path_escape`。
- 测试（追加到 `bb_backend/crates/bb_server/src/http/tests.rs` 与 `bb_core` 的 tests 模块）：空 wiki 目录返回空 tree；含 `index.md` + `modules/session.md` + `modules/context-management/index.md` 的嵌套结构返回正确的 `kind` 与相对 `path`；`..` / 绝对路径 / 非 `.md` 读取一律拒绝；文件不存在返回 404；asset 返回正确 mime。
- 文档：在 `bb_backend/README.md` 的 HTTP 接口表里追加 wiki 三条端点；保持与 `ticket/content` 同一段落风格（index 接口 + content 接口 + 只读声明）。
