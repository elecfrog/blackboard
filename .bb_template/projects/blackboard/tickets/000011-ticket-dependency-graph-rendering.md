+++
id = "000011"
lane = "bbd"
title = "Ticket 依赖 Graph 渲染"
created_at = "2026-05-06"
updated_at = "2026-05-07"
status = "archived"
assignee = "codex"
depends_on = "000016"
parent = "000005"
+++

# 当前进展

- 2026-05-07：从用户夜间任务拆出，准备实现 ticket dependency graph 可视化。

- 2026-05-07：完成 Ticket 依赖 Graph 渲染，按 RDG 有向无环图展示 explicit frontmatter dependencies。

- 2026-05-07：根据用户验收反馈，重做 RDG graph 视觉表达。

- 2026-05-07：将 RDG 从静态 SVG 升级为类 canvas 交互依赖编辑器。

- 2026-05-07: 修复 graph 严重交互问题：节点拖拽和 pin 拖拽不再触发 ticket detail，ticket detail 只保留双击节点打开。

- 2026-05-06：codex 完成 6 张 ticket 联合验收（000006-000011），computer-use Chrome 验证 graph/agents/ticket detail 三页面通过

- 2026-05-06：codex 修复 graph pin drag 和 zoom 问题，browser-use 全流程验证通过

# 记录

- 范围：解析 ticket 依赖关系，在前端绘制 project scoped 依赖 graph，支持从 ticket detail 跳转。
- 验收：能渲染当前 project 的 ticket 节点与依赖边，空依赖/循环/跨状态 ticket 都有稳定显示。

- 实现：后端导出与运行时 API 均只读取 frontmatter depends_on/dependencies；新增 TicketDependencyGraph.vue，边方向为 dependency -> dependent。
- 纠正：000001 无前置依赖；000002 depends_on=000001；000003 depends_on=000001。
- 强门禁：check-ticket-ids.sh 增加 per-project DAG 校验，拒绝缺失依赖、自依赖与环。
- 验证：browser-use 与 computer-use 均打开 /projects/blackboard/graph?focus=000003，确认 000001/000002/000003 可见且无环路警告。

- 返工：RDG 改为分层列布局，增加 Roots/Step 列头、网格画布、320px 节点、两行标题截断、状态胶囊、节点阴影和从 dependency 右侧到 dependent 左侧的曲线箭头。
- 验证：browser-use 打开 /projects/blackboard/graph，确认 11 nodes、5 edges、列头和状态胶囊可见，标题不再冲出节点。

- 实现：Graph 使用 SVG world transform 承载类 canvas 交互，支持节点自由拖拽、画布平移、滚轮围绕鼠标连续缩放、localStorage 保存节点布局。
- 实现：节点右侧连接点用于创建依赖，边中点删除控件用于移除依赖；依赖修改直接 PATCH ticket frontmatter depends_on 并刷新图边。
- 后端：PATCH /api/projects/{project}/tickets/{id} 新增 depends_on 字段，保存前校验六位 ticket id、拒绝未知依赖/自依赖/成环。
- 验证：browser-use 操作图面，临时创建 000011 -> 000010 后 API 显示 000010 dependencies=000008,000011；再通过边删除控件移除，API 恢复 000010 dependencies=000008。
- 验证：live API 尝试设置 000001 depends_on=000002，被后端拒绝 dependency graph must stay acyclic，现有 000001/000002/000003 关系保持正确。

- 2026-05-07: Graph canvas 改为固定 viewBox + viewport transform，节点移动只更新 layout 坐标，不再因为自适应画布大小造成缩放变化；缩放只由滚轮/pointer 用户操作决定。
- 2026-05-07: Node pin 改为上下左右四个 edge center，可 hover 浮出；连接从 pin 拖拽到目标 node bounding/pin snap 区域后自动写入 depends_on，不在 snap 区域则不连接；补充透明命中区和 window-level pointer listener 保证拖拽稳定。
- 验证：browser-use 在 http://127.0.0.1:8071/#/projects/blackboard/graph 验证节点拖拽后 zoom 100% 不变且 detailPanels=0；滚轮后 zoom 变为 112%；从 000001 pin 拖到 000004 pin 临时写入依赖成功，随后 PATCH 恢复 000004 depends_on=[]；拖到空白区域未写入依赖。

- inbox/2026-05-06-codex-bb-mcp-frontend-rdg-验收.md 来源：codex 夜间开发验收 handoff

- inbox/2026-05-06-codex-interactive-dependency-graph-bugfix.md 来源：codex interactive dependency graph bugfix handoff

# 下一步

- 确认现有 dependencies 数据来源，补后端字段或前端解析，并实现 graph view。

- 等待用户验收。

- 等待用户复验 RDG graph 表达。

- 用户复验交互式 graph：拖节点、滚轮缩放、背景拖拽平移、连接/删除依赖。

- 继续打磨 graph 视觉密度和大图布局时，优先补节点选择态、edge label 和 minimap/fit-to-selection 等显式用户操作，不恢复自动缩放。
