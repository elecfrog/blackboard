+++
id = "000022"
lane = "bbd"
title = "Wiki 上传 UI：文件/文件夹 Web 上传，支持目录结构重建"
created_at = "2026-05-08"
updated_at = "2026-05-12"
status = "archived"
assignee = "opencode"
depends_on = "000019"
+++

# 当前进展

- Backend: `POST /api/projects/{project}/wiki/upload` multipart handler in bb_server/http.rs
- Backend: path traversal protection, directory creation, file write validation
- Frontend: upload button in WikiPanel header, `webkitdirectory` support for folder uploads
- Frontend: `uploadWikiFiles()` in data/tickets.ts
- i18n: `upload` key added (zh: 上传, en: Upload)

# 记录

- Files are stored preserving the webkitRelativePath structure
- Hidden files and `..` path traversal are blocked
- After successful upload, the wiki tree is automatically refreshed

- inbox/2026-05-08-opencode-wiki-upload.md 来源：opencode 验收交接，验证 cargo build、npm run build、check_ticket_ids.py、qmd embed 均通过

# 下一步


