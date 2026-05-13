+++
id = "000042"
lane = "bbq"
title = "Rust 文件模块拆分 review 与实施验证"
created_at = "2026-05-10"
updated_at = "2026-05-10"
status = "archived"
assignee = "codex"
bb_tools_used = "list_projects:pass; search_tickets:pass; board_summary:pass; create_ticket:pass; append_ticket_sections:pass; update_ticket:pass; create_inbox_note:pass"
handoff_note = "2026-05-10-codex-rust-http-module-split-review-implementation.md"
requested_by = "user"
scope = "rust-code-module-splitting"
validation_required = "cargo build/check; cargo test"
validation_result = "cargo fmt:pass; cargo check workspace:pass; cargo test workspace:pass; git diff --check code paths:pass"
+++

# 当前进展

- 已确认 project=blackboard，lane=bbq。

- 将本单实施范围收敛为 `bb_cli` HTTP 模块拆分：抽出 `project_picker`、`tickets`、`wiki` 三个资源/功能模块。

- 已完成 `bb_cli/src/http.rs` 模块拆分，主文件从 1190 行降到 568 行。
- 已新增 `bb_cli/src/http/project_picker.rs`、`bb_cli/src/http/tickets.rs`、`bb_cli/src/http/wiki.rs`。
- 验证通过：`cargo fmt --manifest-path bb_backend/Cargo.toml --all`。
- 验证通过：`cargo check --manifest-path bb_backend/Cargo.toml --workspace`。
- 验证通过：`cargo test --manifest-path bb_backend/Cargo.toml --workspace`，`bb_cli` 63 个测试和 `bb_core` 189 个测试通过。
- 验证通过：`git diff --check -- bb_backend/crates/bb_cli/src/http.rs bb_backend/crates/bb_cli/src/http/project_picker.rs bb_backend/crates/bb_cli/src/http/tickets.rs bb_backend/crates/bb_cli/src/http/wiki.rs`。

# 记录

- 应用户要求，对 Rust 代码进行文件/模块拆分 review，产出拆分任务并实施拆分。
- 范围优先覆盖 Rust 代码中体量过大、职责混杂或测试维护成本高的模块。

- Review：`bb_cli/src/http.rs` 1190 行，职责混合了路由注册、项目目录选择、ticket 读取/patch、wiki 读写上传、lane/agent 处理和错误映射；已有 `http/task_graph.rs` 作为子模块先例。
- Review：`bb_core/src/task_graph/node_exec.rs` 1730 行、`bb_core/src/ticket.rs` 1383 行也应后续拆分，但涉及执行副作用约束或核心 CRUD 公共 API，当前单优先做低风险 HTTP 模块拆分。

- 实施：保留 `http.rs` 作为 HTTP app/route table、inbox/board/lane/agent/error 层，按资源拆出项目目录选择、ticket handlers、wiki handlers。
- 边界处理：子模块 handler 使用 `pub(super)` 供父模块路由表绑定，DTO 仅放宽到 `pub(super)`，字段保持私有。

# 下一步

- 梳理 Rust 模块结构并识别拆分候选。
- 按 review 结果拆分任务并修改代码。
- 运行编译与测试验证。

- 新增 `bb_cli/src/http/project_picker.rs`，承接项目目录选择 handler 与平台目录选择实现。
- 新增 `bb_cli/src/http/tickets.rs`，承接 ticket 列表、内容读取、patch 与依赖校验。
- 新增 `bb_cli/src/http/wiki.rs`，承接 wiki tree/file/asset/upload handlers。
- 运行 rustfmt、cargo check、cargo test 进行验证。

- 后续单可继续拆 `bb_core/src/task_graph/node_exec.rs`，建议先抽 runtime-backed nodes，再处理 sub-pipeline。
- 后续单可继续拆 `bb_core/src/ticket.rs`，建议按 validation/render/index 逐步拆，避免一次触碰核心 CRUD 公共 API。
