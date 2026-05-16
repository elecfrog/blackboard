+++
id = "000068"
lane = "bbd"
title = "实现 Agents 页面真实 Agent 指标数据"
created_at = "2026-05-16"
updated_at = "2026-05-16"
status = "archived"
area = "bb_web_agents"
source = "codex"
type = "feature"
+++

# 当前进展


- 2026-05-16：隐藏占位 AgentMetricsCards 并留 TODO 等待真实指标数据接入；变更 Skill 列表新增行为为空闲时显示提示而非禁用按钮；将 Skill 删除文本按钮替换为 lucide Trash2/X 图标

# 记录

- 当前 Agents 页面存在 `AgentMetricsCards` 占位组件，但未接入真实指标来源。按产品决策，该组件已从 Agents 页面暂时隐藏。
- 后续实现时不要恢复占位 `—` 数据；需要先定义真实 Agent 指标的数据来源、后端 API、时间窗口和前端展示语义。

- 来源：inbox/2026-05-16-codex-agents-metrics-hidden-skill-icons.md
- 代码位置：bb_web/src/components/AgentWorkbench.vue, bb_web/src/components/agents/AgentSkillsTable.vue, bb_web/src/i18n.ts, bb_web/src/styles.css

# 下一步

- 定义 Agent 指标口径：运行次数、成功率、平均耗时、Token 总数对应的数据来源和统计窗口。
- 实现后端 API，返回当前 agent 的真实指标数据。
- 恢复 `bb_web/src/components/agents/AgentMetricsCards.vue` 在 Agents 页面中的渲染，并用真实 API 数据替换占位数组。
- 补充前后端测试和浏览器验证，确保无数据时显示明确空态而不是伪指标。
