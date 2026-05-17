+++
id = "000070"
lane = "bbt"
title = "Idea Canvas Backend：canvas/sticky note 轻量持久化与 API"
created_at = "2026-05-17"
updated_at = "2026-05-17"
status = "archived"
area = "Idea Canvas"
depends_on = "000026"
kind = "backend"
parent = "000026"
+++

# 当前目标

实现 Idea Canvas 的后端最小闭环：project 级 canvas/sticky note 轻量持久化与 API。

# MVP 范围

## 数据对象

Canvas：

- `id`
- `title`
- `created_at`
- `updated_at`
- `notes`

Sticky note：

- `id`
- `text`
- `x`
- `y`
- `color`
- `created_at`
- `updated_at`

## 存储

建议文件布局：

```text
projects/<project>/
  idea_canvases/
    canvas-default.json
  __idea_canvases__.json
```

要求：

- 存储与 Inbox / Tickets 平行，属于 project 自身数据对象。
- 写入使用 atomic write。
- `__idea_canvases__.json` 可由 canvas 文件重建。
- id、文件名和路径必须做安全校验，不能逃逸 project。

## API

建议 HTTP API：

```text
GET    /api/projects/{project}/idea-canvases
POST   /api/projects/{project}/idea-canvases
GET    /api/projects/{project}/idea-canvases/{canvas_id}
PATCH  /api/projects/{project}/idea-canvases/{canvas_id}
DELETE /api/projects/{project}/idea-canvases/{canvas_id}

POST   /api/projects/{project}/idea-canvases/{canvas_id}/notes
PATCH  /api/projects/{project}/idea-canvases/{canvas_id}/notes/{note_id}
DELETE /api/projects/{project}/idea-canvases/{canvas_id}/notes/{note_id}
```

# 验收

- 可创建 canvas。
- 可列出 canvas。
- 可读取 canvas 及其 notes。
- 可创建 sticky note。
- 可更新 sticky note 文本、位置和颜色。
- 可删除 sticky note。
- 重启后数据仍存在。
- 后端测试覆盖核心 CRUD。

# 关联

- `ticket:000026`
