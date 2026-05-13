+++
id = "000045"
lane = "bbq"
title = "Rust 模块文件夹自包含清理"
created_at = "2026-05-10"
updated_at = "2026-05-10"
status = "archived"
assignee = "codex"
+++

# 当前进展

- codex 开始处理 `bb_core/src/wiki.rs` 与 `bb_core/src/wiki/` 混合布局，目标是让 wiki 模块目录自包含。

- codex 开始执行：确认本轮只做 Rust wiki 模块自包含与同类残留扫描，不动无关并发改动。

- 用户指出问题不是 wiki 单点，而是 Rust 代码大量 hybrid module 未收敛；本轮范围扩大为同名目录存在时，根模块统一迁入目录 `mod.rs`，测试跟随模块目录管理。

- 用户继续指出 `bb_core/src/inbox.rs` 仍是未拆的大模块；本轮继续扩大到 inbox 模块职责拆分，不只处理已经存在同名目录的 hybrid modules。
- 全量测试首次在移动 `task_graph/tests.rs` 后暴露 `include_str!` 相对路径失效，需要先修正 fixture 路径。

- 用户指出 `ticket/search.rs` 下包含通用 search helper，并且模块引用 `InboxError` 命名不合理；确认这是 ticket/inbox 交叉边界和历史 error 命名问题。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-rust-模块文件夹自包含清理.md`。

- 2026-05-10：清除 bb_core/src 与 bb_cli/src 下所有同名 foo.rs + foo/ hybrid module，统一迁到 foo/mod.rs；将 wiki/inbox/ticket 拆分为自包含子模块；新增 common 模块移入通用 search helper；将 core error 主类型改为 BlackboardError，保留 InboxError 作兼容 alias

# 记录


- 验证：`cargo fmt --manifest-path bb_backend/Cargo.toml --all` 通过。
- 验证：`cargo check --manifest-path bb_backend/Cargo.toml --workspace` 通过。
- 验证：`cargo test --manifest-path bb_backend/Cargo.toml --workspace` 通过：bb_cli 63 tests，bb_core 189 tests，doc-tests 0。
- 验证：`git diff --check -- bb_backend/crates/bb_core/src bb_backend/crates/bb_cli/src` 通过。
- 验证：结构扫描通过：同名 `foo.rs + foo/` 输出为空。
- 验证：结构扫描通过：`tests.rs` / `*_tests.rs` 输出为空。

- 来源：inbox/2026-05-10-codex-rust-模块文件夹自包含清理.md
- 相关路径：bb_backend/crates/bb_core/src/common/mod.rs, bb_backend/crates/bb_core/src/inbox/, bb_backend/crates/bb_core/src/ticket/mod.rs, bb_backend/crates/bb_core/src/error.rs, bb_backend/crates/bb_core/src/wiki/, bb_backend/crates/bb_cli/src/http/, bb_backend/crates/bb_cli/src/stdio/

# 下一步

- 移动 wiki 根模块/测试到目录内，修正 `lib.rs` 模块声明。
- 扫描同类 hybrid module 残留并运行 Rust fmt/check/test。

- 扫描 `bb_core/src` 与 `bb_cli/src` 下所有 `foo.rs + foo/` 同名目录组合。
- 批量迁移根模块到 `foo/mod.rs`，清理重复/外置测试，运行 fmt/check/test。

- 修正 `task_graph/tests/mod.rs` 的 system graph fixture include 路径。
- 拆分 `bb_core/src/inbox.rs` 为 `inbox/` 目录模块并保持公开 API 不变。
- 重新运行 cargo fmt/check/test 与结构扫描。

- 将通用搜索 helper 从 `ticket/search.rs` 移到 core 级别 `search` 模块，ticket/inbox 分别引用共享 helper。
- 继续完成 inbox 模块拆分，保持公开 API 不变。
- 评估 `InboxError` 是否应后续单独重命名为 core error，避免在本轮大范围移动中叠加全仓 API rename。

- 继续拆 `ticket/mod.rs`、`workspace.rs`、`daemon.rs`、`types.rs` 等仍然偏大的根实现文件，进入更细粒度的职责模块清理。
