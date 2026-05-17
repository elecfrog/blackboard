+++
id = "000026"
lane = "bbp"
title = "Idea Canvas：轻量自由的想法白板"
created_at = "2026-05-08"
updated_at = "2026-05-17"
status = "archived"
area = "Idea Canvas"
kind = "product"
product_object = "idea_canvas"
+++

# 当前定义

Idea Canvas 是轻量自由的想法白板。

Tickets 承担正式工作项，Inbox 承担交接和流水。Idea Canvas 提供低压力、自由、松散的空间，让用户把暂时还不想整理成 ticket 的想法先放下来。

## 核心形态

- 左侧是 canvas / board 列表。
- 右侧是当前 canvas。
- 用户可以按当天状态、心情、主题、杂事或临时场景选择一个空间。
- 用户在 canvas 上双击创建 sticky note / sticker / 小纸条 / 小黄条。
- sticky note 可直接编辑、拖动摆放、删除。
- 位置、颜色和文本服务于自由整理，而不是把内容过早变成正式对象。

## MVP 范围

- canvas 列表。
- canvas 画布。
- 双击创建 sticky note。
- sticky note 直接编辑。
- sticky note 拖动改变位置。
- sticky note 删除。
- project 级轻量持久化。

# 子票

- `000070`：Idea Canvas Backend：canvas/sticky note 轻量持久化与 API。
- `000071`：Idea Canvas Frontend：canvas 列表与 sticky note 白板。
- `000072`：Idea Canvas 后续转化：从 sticker 到 ticket 的可选路径，后置于 MVP。

# 下一步

- 先按 `000070` 和 `000071` 完成 Idea Canvas MVP。
- `000072` 只在 canvas/sticky note 核心体验稳定后再讨论。

# 当前进展

- 2026-05-17：实现 Idea Canvas 后端 project 级存储与 `/api/projects/{project}/idea-canvases` CRUD
- 2026-05-17：实现前端独立 Idea Canvas 工作区，左侧 canvas 列表，右侧 canvas，双击创建 sticky note，直接编辑、拖动、删除

# 记录

- 来源：inbox/2026-05-17-codex-000026-idea-canvas-mvp.md
- 代码位置：bb_backend/crates/bb_core/src/idea_canvas/mod.rs, bb_cli/src/http/idea_canvas.rs, bb_web/src/components/IdeaCanvasPanel.vue, bb_web/src/data/ideaCanvas.ts, bb_web/src/views/BoardView.vue
