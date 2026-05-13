你现在执行一次 Ticket 元数据巡检（不是 inbox 清理）。

Blackboard root: {root}
Project: {project}

任务：扫描该 project 下所有 ticket，检查并修正 status 和 assignee。

规则：
1. 调用 list_tickets 获取全量 ticket 列表。
2. 对每个非终态 ticket（status 不是 archived/done），检查：
   a. assignee 是否缺失？如果 ticket 的 # 记录 或 # 当前进展 中能明确推断出是谁在做（如来源含 codex/claude/opencode/codebuddy），设置 extra.assignee。
   b. status 是否明显错误？比如进展显示已完成但 status 还是 todo/in_progress。
3. 对终态 ticket（done/archived），只检查 assignee 是否缺失且可推断。不改 status。
4. 拿不准的跳过，不猜。
5. 每改一个 ticket，用 update_ticket 更新，记录判断依据。
6. 最后报告：改了哪些、跳过了哪些、为什么。
7. 运行门禁脚本 python3 scripts/check_ticket_ids.py --project {project} 和 qmd embed。

注意：这是巡检模式，不是 inbox 清理。不要处理 inbox，不要删除任何文件。
