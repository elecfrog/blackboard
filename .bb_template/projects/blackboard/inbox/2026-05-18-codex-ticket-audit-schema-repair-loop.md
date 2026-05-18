# Ticket Audit schema 校验闭环落地

时间: 2026-05-18T12:06:47Z
来源: codex
项目: blackboard

## 做了什么

- 新增 scripts/audit_tickets.py，把生成的 ticket schema 接入脚本级 ticket 审计，并输出 .bb_template/runtime/ticket-audit/<project>-latest.json。
- 重做 system/task-ticket-audit：audit_tickets.py 先查，发现 exit_code 2 后进入 LLM repair，repair 输出先过 schema_validate，再用同一脚本复查并按 loop 重试。
- 补齐 shell node 模板渲染，让 graph shell args/cwd/env 能正确使用 {{env.project}}、{{env.root}}、{{env.scripts_dir}}。
- 把 schemas 目录纳入 dev/desktop seed 流程，并创建 000076 记录该维护闭环。

## 验证了什么

- python3 scripts/audit_tickets.py --project blackboard --repo-root . --run-schema-check --run-maintenance: passed，72 JSON tickets，0 legacy markdown，0 findings。
- cargo run --manifest-path bb_backend/Cargo.toml -p bb_cli -- --root .bb_template daemon --graph system/task-ticket-audit --project blackboard --once: passed，run-20260518-120612-258cb10c。
- cargo check --manifest-path bb_backend/Cargo.toml --workspace: passed。
- cargo test --manifest-path bb_backend/Cargo.toml -p bb_core task_graph::tests::tests::test_validate_system_graph_fixture: passed。
- python3 scripts/check_ticket_ids.py --project blackboard: passed；qmd embed: passed；git diff --check: passed。

## 下一步

- （未填写）

## 相关位置

- scripts/audit_tickets.py
- .bb_template/task_graphs/system/task-ticket-audit.json
- .bb_template/agents/prompts/ticket-audit.md
- bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs
- scripts/dev.py
- scripts/desktop.py
- .bb_template/projects/blackboard/tickets/000076-ticket-audit-schema-repair-loop.json
