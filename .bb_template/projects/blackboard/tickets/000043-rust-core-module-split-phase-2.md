+++
id = "000043"
lane = "bbq"
title = "Rust 核心模块拆分 review 与第二阶段实施"
created_at = "2026-05-10"
updated_at = "2026-05-10"
status = "archived"
assignee = "codex"
bb_tools_used = "list_projects:pass; find_work_context:pass; create_ticket:pass; append_ticket_sections:pass; complete_handoff:pass; update_ticket:pass"
handoff_note = "2026-05-10-codex-rust-core-module-split-phase-2.md"
parent_ticket = "000042"
requested_by = "user"
scope = "rust-core-module-splitting-phase-2"
validation_required = "cargo fmt; cargo check --workspace; cargo test --workspace"
validation_result = "cargo fmt:pass; cargo check workspace:pass; cargo test workspace:pass (bb_cli 63, bb_core 189, doc-tests 0); git diff --check code paths:pass"
+++

# 当前进展

- 已确认 project=blackboard，lane=bbq。
- 已确认 `000042` 完成，当前继续第二阶段拆分。

- 本轮实施目标确定为拆分 `bb_core/src/task_graph/node_exec.rs`。

- `node_exec.rs` 拆分已通过 `cargo check` 和 `cargo test`。
- 继续实施第二个拆分目标：`bb_core/src/ticket.rs` helper 层拆分。

- 完成第二阶段 Rust 模块拆分：bb_core task_graph 的 node_exec、run_state、validation、runtime 继续按职责分层。
- 完成 ticket / wiki / agents_config / agents_registry 的模型、索引、路径、校验、渲染等子模块拆分，保留原 public API re-export。
- 完成 bb_cli HTTP task_graph 与 MCP tools schema 拆分；第一阶段 HTTP project_picker/tickets/wiki 拆分保持在同一验证批次内。
- 继续拆分 node_exec 下偏大的 control_nodes/runtime_nodes 子模块，将 branch/loop/simple control 与 llm/registered task 执行路径分开。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-rust-core-module-split-phase-2.md`。

- 2026-05-10：完成 bb_core task_graph 的 node_exec/run_state/validation/runtime 分层拆分；完成 ticket/wiki/agents_config/agents_registry 模型子模块拆分；完成 bb_cli HTTP task_graph 与 MCP tools schema 拆分；保留原 public API re-export 维持路径兼容

# 记录

- 承接 `000042` 的后续工作，继续 review Rust 大文件和模块边界，优先覆盖 `bb_core` 中职责更重的核心模块。
- 本阶段避免一次性大面积重构，选择可由现有测试覆盖、行为迁移风险较低的拆分点。

- Review：`bb_core/src/task_graph/node_exec.rs` 1730 行，是当前最大生产源码文件，且内部已按图导航、Start/End/Branch/Loop/Input/HumanGate、LLM/RegisteredTask、SubPipeline 分段。
- Review：`bb_core/src/ticket.rs` 1383 行也适合后续拆分，但它直接承载 ticket CRUD、frontmatter 写入和兼容逻辑，本轮先选择测试覆盖更集中、职责分段更明显的 Task Graph node execution。

- 继续 review：`bb_core/src/ticket.rs` 1383 行，职责包含 ticket CRUD、frontmatter 解析/渲染、TicketEntry 构建、搜索辅助、正文 section 追加和文件写入。
- 拆分策略：保留 `impl Blackboard` ticket CRUD 在 `ticket.rs`，只抽 helper 子模块，降低核心 API 风险。

- 验证：cargo fmt --manifest-path bb_backend/Cargo.toml --all 通过。
- 验证：cargo check --manifest-path bb_backend/Cargo.toml --workspace 通过。
- 验证：cargo test --manifest-path bb_backend/Cargo.toml --workspace 通过（bb_cli 63 tests；bb_core 189 tests；doc-tests 0）。
- 验证：git diff --check -- bb_backend/crates/bb_core/src bb_backend/crates/bb_cli/src 通过。
- 范围外状态：工作区存在既有/并发的 inbox 删除与 000007/000040/000041 ticket 修改，未回退、未纳入本次代码重构判断。

- 验证：cargo fmt --manifest-path bb_backend/Cargo.toml --all: pass
- 验证：cargo check --manifest-path bb_backend/Cargo.toml --workspace: pass
- 验证：cargo test --manifest-path bb_backend/Cargo.toml --workspace: pass（bb_cli 63，bb_core 189，doc-tests 0）
- 验证：git diff --check -- bb_backend/crates/bb_core/src bb_backend/crates/bb_cli/src: pass

- 来源：inbox/2026-05-10-codex-rust-core-module-split-phase-2.md
- 相关路径：bb_backend/crates/bb_core/src/task_graph/node_exec.rs, bb_backend/crates/bb_core/src/task_graph/run_state.rs, bb_backend/crates/bb_core/src/task_graph/validation.rs, bb_backend/crates/bb_core/src/ticket.rs, bb_backend/crates/bb_core/src/wiki.rs, bb_backend/crates/bb_core/src/agents_config.rs, bb_backend/crates/bb_core/src/agents_registry.rs, bb_backend/crates/bb_cli/src/http.rs, bb_backend/crates/bb_cli/src/mcp_tools.rs

# 下一步

- 统计 Rust 文件体量和职责分布。
- 选定一个低风险核心模块拆分目标并实施。
- 运行 cargo fmt/check/test 验证。

- 抽出图导航 helper 子模块。
- 抽出运行时节点执行子模块。
- 抽出子流水线节点执行子模块。
- 运行 rustfmt、cargo check、cargo test。

- 新增 `ticket/frontmatter.rs` 承接 frontmatter parse/render/sanitize。
- 新增 `ticket/entry.rs` 承接 TicketEntry 构建与 ID helper。
- 新增 `ticket/render.rs` 承接 ticket markdown render/section append。
- 新增 `ticket/search.rs` 承接 ticket/inbox 共享搜索 helper。
