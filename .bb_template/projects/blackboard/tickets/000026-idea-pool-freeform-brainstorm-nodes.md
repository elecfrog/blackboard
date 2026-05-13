+++
id = "000026"
lane = "bbp"
title = "Idea Pool：自由脑暴节点池"
created_at = "2026-05-08"
updated_at = "2026-05-08"
status = "todo"
area = "IdeaPool"
kind = "product"
+++

# 当前进展

- 需求：新增 Idea Pool 入口，用于承载还不能清晰表达为正式 ticket 的脑暴想法。
- 核心形态：用户可以随意扔 Node、自由摆放、整理关系，待想法清楚后再沉淀为正式 ticket。
- 本单先作为产品定义单，不挂根、不直接拆实现；等 Tickets 三视图入口整合完成后，再围绕 Idea Pool 开实现子单。

# 记录



# 下一步

- 定义 Idea Node 与 Ticket Node 的边界：哪些字段是草稿态，哪些关系不应立即写入 ticket depends_on。
- 设计 Idea Pool 到正式 ticket 的 promote 工作流，包括转单、合并、拆分、保留位置与关系。
- 待 Tickets 入口整合完成后，拆分前端/后端/数据模型实现子单。
