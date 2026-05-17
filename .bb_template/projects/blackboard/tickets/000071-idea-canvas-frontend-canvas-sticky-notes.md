+++
id = "000071"
lane = "bbd"
title = "Idea Canvas Frontend：canvas 列表与 sticky note 白板"
created_at = "2026-05-17"
updated_at = "2026-05-17"
status = "archived"
area = "Idea Canvas"
depends_on = "000026 000070"
kind = "frontend"
parent = "000026"
+++

# 当前目标

实现 Idea Canvas 的前端 MVP：左侧 canvas 列表，右侧自由画布，双击创建 sticky note，sticky note 可直接编辑、拖动和删除。

# MVP 交互

- 左侧显示 canvas 列表。
- 右侧显示当前 canvas。
- 双击 canvas 空白处创建 sticky note。
- 新建 sticky note 立即进入编辑。
- sticky note 文本可直接在 note 上编辑。
- sticky note 可拖动摆放。
- sticky note 可删除。
- 刷新后 canvas 和 sticky note 状态保持。

# 视觉气质

- 轻松、自由、松散、低压力。
- canvas 是主角，不做表单主导。
- sticky note 可以像小纸条 / 小黄条，不需要严肃 card 语义。

# 代码边界

建议新增：

```text
bb_web/src/data/ideaCanvas.ts
bb_web/src/components/IdeaCanvasPanel.vue
```

`BoardView` 只负责：

- workspace key
- route
- sidebar tab
- async component mount

业务状态和交互逻辑必须放在 Idea Canvas 自己的模块里，不放进 `BoardView`。

# 验收

- `npm run build --prefix bb_web` 通过。
- 使用 `python3 scripts/dev.py --repo-root` 启动。
- 用 Computer Use 验证：进入 Idea Canvas 页面，双击创建 note，输入文本，拖动 note，刷新后位置和文本仍在。

# 关联

- `ticket:000026`
- `ticket:000070`
