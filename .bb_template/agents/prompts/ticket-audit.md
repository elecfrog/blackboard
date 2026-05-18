你现在执行 Ticket Audit Repair 节点。

Blackboard root: {{env.root}}
Project: {{env.project}}
Audit summary from shell:

```json
{{inputs.audit-summary}}
```

完整报告路径：`{{inputs.report-path}}`

任务：

1. 先读取完整报告路径里的 JSON；不要只依赖上面的 stdout tail。
2. 如果报告 `status` 是 `passed`，不要改 ticket，只输出最终 JSON。
3. 如果报告有 findings：
   - 只修报告中列出的 ticket 和字段，不要顺手改其它文件。
   - 优先使用 Blackboard MCP 工具 `read_ticket_by_id` / `update_ticket` 修复 ticket。
   - 如果某张 ticket 已经坏到 MCP 读取或更新被 schema/业务校验拒绝，本 graph 是本地 ticket audit/repair 流水线，允许只对报告列出的 ticket JSON 做最小本地文件修复；不要改 `__tickets__.json`，索引由后续脚本重建。
   - 不要创建新 ticket，不要删除 ticket，不要改 lane 定义，不要处理 inbox。
4. 常见修复：
   - 缺必填字段：按现有 ticket 语义补齐；`attachments`、`risks`、`progress_record` 可以是空数组。
   - `summary`、story/risk/progress/attachment 文本有首尾空白：只 trim。
   - 空字符串可选字段：删除该字段，或改成有意义的非空文本；不要保留 `""`。
   - `extra.attachments`：迁移到顶层 `attachments`，再删除 `extra.attachments`。
   - story `id` 必须是 UUID；如果缺失或是语义 id，生成 UUID。
5. 修复后不要自己判断最终通过；后续 shell checker 会重跑 schema 与脚本校验。

最终只输出 JSON，不要 Markdown，不要解释：

```json
{
  "summary": "本轮修复摘要；如果无事可做，写 no changes needed",
  "changed": [
    {
      "id": "000001",
      "fields": ["summary", "risks[0].mitigation"],
      "reason": "根据 audit findings 修复首尾空白和空字符串可选字段"
    }
  ],
  "skipped": [
    {
      "id": "000002",
      "reason": "报告中的问题需要产品判断，未自动修改"
    }
  ],
  "ticket_validation": {
    "audit_status_before": "failed",
    "finding_count_before": 1,
    "report_path": ".bb_template/runtime/ticket-audit/blackboard-latest.json"
  },
  "verification": {
    "audit_report_consumed": "passed",
    "repair_attempted": "passed"
  }
}
```

字段要求：

- `changed` 可以为空数组。
- `skipped` 可以为空数组。
- `id` 必须是六位数字字符串。
- `verification.audit_report_consumed` 只能是 `passed` 或 `failed`。
- `verification.repair_attempted` 只能是 `passed`、`failed` 或 `not_needed`。
