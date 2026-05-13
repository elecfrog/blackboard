+++
id = "000007"
lane = "bbt"
title = "多 Project MCP 工作流工具"
created_at = "2026-05-06"
updated_at = "2026-05-10"
status = "archived"
assignee = "codex"
depends_on = "000006"
parent = "000005"
+++

# 当前进展

- 2026-05-07：从用户夜间任务拆出，准备实现多 project 工作流 MCP 工具。

- 2026-05-07：完成多 project MCP 工作流工具：find_work_context、begin_ticket_work、complete_handoff。

- 2026-05-07：用户按方案 2 验收通过。三工具 A+B+C 冒烟在 008 上全链路跑通后已善后复原。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-find-work-context-mcp-返回协议修复.md`。

- 2026-05-10：修复 find_work_context 返回协议，组合工具直接返回裸 JSON 导致 rmcp bridge 收不到 content，改为统一 MCP text content tool result
- 2026-05-10：同步修复 complete_handoff 同类返回协议问题
- 2026-05-10：补齐 complete_handoff tools/list inputSchema 必填 id 字段
- 2026-05-10：更新 stdio workflow test，让 find_work_context 与 complete_handoff 都按 result.content[0].text 解析 payload

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-mcp-工具全量-content-协议回归.md`。

- 2026-05-10：通查 MCP 工具返回路径统一 content text result；修复 create_inbox_note/tools/list schema 误要求 id；补充清单驱动回归测试；rmcp bridge 增加空 content 防御；修复 HTTP test fixture 阻塞

# 记录

- 范围：新增 find_work_context / begin_ticket_work / complete_handoff 等组合工具，固化扫 active ticket、检索 inbox、写瘦 handoff、门禁记录。
- 验收：Agent 能通过 MCP 完成一次 project scoped 起止协议，不手搓路径。

- 实现：工作入口可跨 projects 搜 active tickets/inbox，开始工作可设置 in_progress 与 assignee，收尾可写 handoff 并回填 ticket。
- 验证：cargo test 通过；live MCP smoke 返回 context project=blackboard。

- 验收：find_work_context 无 project 时跨 4 projects 扫 active；project+query 返回 {filename,line,path,snippet} 匹配体。
- 验收：begin_ticket_work 置 in_progress + 写 assignee + 追加 progress bullet，maintenance.consistency=passed。
- 验收：complete_handoff 在 inbox 按 YYYY-MM-DD-<source>-<topic>.md 生成瘦 handoff，ticket → review，正文 progress/record/next_step 三节全追加，maintenance.consistency=passed。
- 已知缺陷：complete_handoff 对已 done 的 ticket 会无条件把 status 打回 review，另行开票处理。

- inbox/2026-05-07-codex-007-workflow-tools-accepted.md 来源：append_ticket_sections→maintenance.consistency.passed；update_ticket 搬桶→maintenance.consistency.passed；check-ticket-ids.sh exit=0；npm run export:tickets 导出 4 projects (19 tickets, 13 inbox notes)；qmd embed 无新内容。

- 验证：bb list_projects 通过，确认 project=blackboard。
- 验证：cargo fmt --manifest-path bb_backend/Cargo.toml --all -- --check 通过。
- 验证：cargo check --manifest-path bb_backend/Cargo.toml -p bb_cli 通过。
- 验证：git diff --check -- bb_backend/crates/bb_cli/src/mcp_tools.rs bb_backend/crates/bb_cli/src/stdio/tests.rs 通过。
- 验证：cargo test --manifest-path bb_backend/Cargo.toml -p bb_cli workflow_tools_find_begin_and_complete_ticket_work -- --nocapture 被范围外删除的 task_graphs/system/frontend-smoke-loop.json include_str 阻塞，未改动该并行任务文件。

- 来源：inbox/2026-05-10-codex-find-work-context-mcp-返回协议修复.md
- 相关路径：bb_backend/crates/bb_cli/src/mcp_tools.rs
- 相关路径：bb_backend/crates/bb_cli/src/stdio/tests.rs

- 验证：bb list_projects 通过，确认 project=blackboard。
- 验证：cargo fmt --manifest-path bb_backend/Cargo.toml --all -- --check 通过。
- 验证：cargo check --manifest-path bb_backend/Cargo.toml -p bb_cli 通过。
- 验证：cargo test --manifest-path bb_backend/Cargo.toml -p bb_cli stdio::tests:: -- --nocapture 通过，13 passed。
- 验证：cargo test --manifest-path bb_backend/Cargo.toml -p bb_cli mcp_tools_call_returns_content -- --nocapture 通过。
- 验证：cargo test --manifest-path bb_backend/Cargo.toml -p bb_cli 通过，63 passed。
- 验证：git diff --check -- bb_backend/crates/bb_cli/src/mcp_tools.rs bb_backend/crates/bb_cli/src/stdio/tests.rs bb_backend/crates/bb_cli/src/http/tests.rs bb_backend/crates/bb_cli/src/mcp_handler.rs 通过。

- 来源：inbox/2026-05-10-codex-mcp-工具全量-content-协议回归.md

# 下一步

- 在 schema 模块化后补充组合工具与测试。

- 等待用户验收。

- 用户验收通过，移入 done。

- 等待用户验收。
- 用户验收通过，移入 done。
