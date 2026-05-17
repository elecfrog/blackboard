+++
id = "000072"
lane = "bbp"
title = "Idea Canvas 后续转化：从 sticker 到 ticket 的可选路径"
created_at = "2026-05-17"
updated_at = "2026-05-17"
status = "blocked"
area = "Idea Canvas"
blocked_reason = "post_mvp_after_canvas_core"
depends_on = "000026 000070 000071"
kind = "product-workflow"
parent = "000026"
+++

# 当前目标

定义 Idea Canvas 后续转化能力：当 canvas/sticky note MVP 稳定后，再考虑如何从一个或多个 sticky note 轻量生成 ticket。

本票是后续增强，不是 MVP 核心。

# 原则

- 转化能力不能压过随手记录和自由整理体验。
- sticky note 默认不是 ticket。
- 转成 ticket 前应由用户明确选择。
- 生成 ticket 时可以保留 sticky note 文本、canvas id、位置和颜色作为上下文。

# 待定义问题

- 单个 sticky note 转 ticket 的默认标题和正文。
- 多个 sticky note 合并转 ticket 的排序和分组。
- 转化后 sticky note 是否保留、标记或不变。
- Ticket attachment 是否需要引用 canvas/note。
- 是否需要从 ticket 回到 canvas 位置。

# 阻塞条件

等待：

- `000070` 后端 canvas/note 存储与 API 完成。
- `000071` 前端 canvas/sticky note 核心交互完成。
- 用户确认转化入口是否符合 026 的轻量气质。

# 关联

- `ticket:000026`
- `ticket:000070`
- `ticket:000071`
