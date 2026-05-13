+++
id = "000014"
lane = "bbt"
title = "统一 Ticket 状态模型：移除 bucket 双轨状态"
created_at = "2026-05-07"
updated_at = "2026-05-08"
status = "archived"
assignee = "codex"
depends_on = "000013"
+++

# 当前进展

- 2026-05-07 Codex 开始统一 Ticket 状态模型，目标只保留 todo / in_progress / blocked / review / done / archived 一套状态，覆盖 Board / Tickets / Graph。

- 2026-05-07 Codex 已将后端、REST/MCP、Web Board/Tickets/Graph 收敛到 todo / in_progress / blocked / review / done / archived 单一状态模型。
- 2026-05-07 前端状态展示统一通过 i18n.ts 的 ticketStatusLabel() 输出，移除散落中文状态常量。
- 2026-05-07 已迁移现有 Markdown ticket 与 __tickets__.json 索引，清除旧 active / doing / bucket 字段残留。

# 记录


- 验证通过：cargo test --manifest-path bb_backend/Cargo.toml。
- 验证通过：npm run build --prefix bb_web。
- 验证通过：PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_ticket_ids.py。
- 验证通过：qmd embed。
- 验证通过：索引检查确认 projects/*/__tickets__.json 不再含 bucket，ticket 数据不再含 active / doing 状态。

- 浏览器 smoke 通过：http://127.0.0.1:8060/#/projects/blackboard 展示待办 / 进行中 / 验收 / 归档等 i18n 状态标签，未出现裸 status* key 或 console error。
- 服务已重启到新代码：bb_backend 监听 127.0.0.1:3001，bb_web 监听 127.0.0.1:8060。

# 下一步

- 移除 bucket 与 overview 双轨口径，后端、MCP/REST、前端视图和现有数据一次性扫平。

- 进入人工验收：重启 dev server 后检查 Board / Tickets / Graph 的六状态展示与拖拽更新。
