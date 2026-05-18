# Inbox Note

时间: 2026-05-18T11:44:50Z
来源: Codex
项目: blackboard

## 做了什么

- 创建并完成 000075：Rust code-to-schema 独立生成器。
- bb_core 增加 schema feature 和代码生成入口；bb_schema 提供 bb-schema generate/check。
- ticket schema 生成到 .bb_template/schemas/ticket.schema.json，旧 templates/ticket.schema.json 删除，cleanup prompt/test/README 改到新路径。
- 修复 000001/000018/000040 既有 JSON ticket 校验脏数据，并重建 __tickets__.json。

## 验证了什么

- cargo test --manifest-path bb_backend/Cargo.toml -p bb_core: passed
- cargo check --manifest-path bb_backend/Cargo.toml --workspace: passed
- cargo run --manifest-path bb_backend/Cargo.toml -p bb_schema -- check --root .bb_template: passed
- python3 scripts/check_ticket_ids.py --project blackboard: passed
- qmd embed: passed
- git diff --check: passed
- remote /mcp tools/call update_ticket 000075: passed; maintenance consistency passed

## 下一步

- （未填写）

## 相关位置

- tickets/000075-schema-tooling-rust-code-to-schema-独立生成器.json
- bb_backend/crates/bb_core/src/schema.rs
- bb_backend/crates/bb_schema/src/main.rs
- .bb_template/schemas/ticket.schema.json
