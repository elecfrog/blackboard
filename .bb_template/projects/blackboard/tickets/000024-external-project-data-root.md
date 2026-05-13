+++
id = "000024"
lane = "bbt"
title = "Project 数据目录外置：支持用户指定项目根路径"
created_at = "2026-05-08"
updated_at = "2026-05-08"
status = "archived"
assignee = "codex"
depends_on = "000013"
parent = "000005"
+++

# 当前进展

- 2026-05-08：用户明确要求 Blackboard project 的数据目录不能固定放在 Blackboard 源码仓库内，应支持按 project 指定外部目录。

- 2026-05-08：修复 `machine_host_name()` macOS/Linux 回退：`hostname -s` 命令补防
- 2026-05-08：实现 macOS 目录选择器 via `osascript` `choose folder`， gated with `#[cfg(target_os = "macos")]`
- 2026-05-08：`resolve_registry_data_root_inner()` 增加相对路径回退 + 自动持久化当前机器位置条目

- 2026-05-08：实现 Workspace::list_projects / read_project_meta / open_project 支持外部 data_root 解析
- 2026-05-08：bb_core + bb_server 覆盖外部目录挂载、Ticket 创建、REST API（/api/projects、/api/projects/{project}/inbox/notes）
- 2026-05-08：前端 Project 元数据Typing更新 + Project下拉框Footer重构创建/打开入口，移除Settings页表单
- 2026-05-08：Windows目录选择器迁移至Win32 Common Item Dialog（支持粘贴路径、中文Unicode清洁）
- 2026-05-08：移除不安全 PATCH /api/projects/{project}/data-root 写路径，外部根只允许通过创建/打开流程注册
- 2026-05-08：FileDialog绑定当前Foreground Window Owner（非Show(None)），行为变为 Blackboard 的模态对话框
- 2026-05-08：projects/__projects__.json 作为中央Registry，替代原有per-project stub注册机制
- 2026-05-08：location解析优先本机绝对路径，回退相对路径；两者均不可用时报Registry错误而非静默落回
- 2026-05-08：修复Registry引导（machines.CURRENT 空白时填充当前主机名）、OS_VERSION记录（Win/macOS/Linux）

# 记录

- 背景：当前 Blackboard 以 `<bb-root>/projects/<project>/` 作为事实源目录，适合自带 demo/project，但会把真实项目的协作数据绑进 Blackboard 源码仓库。
- 产品目标：Blackboard 自身是工具，不应该强占所有 project 的数据归属；project 数据目录应能贴近用户真实 repo / workspace / 知识库。
- 兼容目标：现有内置 `projects/<project>/` 继续可用，作为默认存储和迁移前兼容路径。
- 2026-05-08 追加：用户要求提供完整创建 / 打开流程，而不是只填写 `data_root`。
- 创建流程：用户从 Project 下拉框底部点击“创建”，Blackboard 拉起系统目录选择框；目录选中后用路径最后一级 PureDirectoryName 生成默认 Project Name，显示名默认保留目录原名，用户可覆盖；确认后在目标目录生成完整模板（`__project__.json`、`__tickets__.json`、`__inbox__.json`、`inbox/`、`tickets/`、`wiki/`），并在中央 registry 写入 uuid 与本机 location。
- 打开流程：用户从 Project 下拉框底部点击“打开”，Blackboard 拉起系统目录选择框；目录选中后先识别并校验 `__project__.json` / `__tickets__.json` / `__inbox__.json` 模板和必要目录，确认 project 有效且未损坏；若本机尚未索引该目录，则由用户确认 Project Name 后再写入中央 registry。
- 索引策略：`projects/<project>/__project__.json` 是每台机器的 local registry stub，允许跨平台 / 跨机器存在不同本机索引；同一台机器上同一个 canonical project data root 只能挂到一个 project key，避免双入口冲突；外部 project 目录里的 `__project__.json` 不写入本机绝对路径，避免污染可移植事实源。
- 2026-05-08 修正：Windows 目录选择器必须使用 Win32 Common Item Dialog，支持地址栏输入 / 粘贴路径，并直接返回 Unicode 路径；旧 PowerShell `FolderBrowserDialog` 会把中文目录名写成 mojibake，已移除。
- 2026-05-08 复盘修正：移除未授权的 `.BB` 后缀语义和后端自动命名逻辑；目录名只作为默认 Project Name / 显示名来源，不携带隐藏业务语义。
- 2026-05-08 复盘修正：FileDialog 由后端进程拉起时必须绑定当前 foreground window owner；不能用无 owner 的 `Show(None)`，否则看起来像独立窗口而不是当前 Blackboard 操作的模态对话框。
- 2026-05-08 交互修正：创建 / 打开确认 UI 改为页面重心的大卡片；目录路径只作为副标题展示；Project Name 默认使用归一化后的最后一级文件夹名，显示名默认使用所选路径的最后一级文件夹原文，用户可覆盖。
- 2026-05-08 架构修正：`projects/__projects__.json` 成为全局 project registry，统一维护 project key、不可变 uuid、机器信息、每台机器的 project location。
- registry 结构：`projects` 记录 `{ project_key: { uuid, locations } }`；`locations` 记录 `{ machine_name: { absolute_path, relative_path } }`；`machines.CURRENT` 每次启动刷新为当前主机名，机器节点记录 OS，OS_Version 可空。
- 路径解析策略：优先使用当前机器 location 的 `absolute_path`；若绝对路径为空、不可读或不存在，则回退 `relative_path`；两者都不可用时报 project registry 错误，不再静默落回源码目录。
- 跨机器策略：普通 project location 写入真实主机名；只有 Blackboard 自带示例 project 使用 `CURRENT` + `./blackboard`，并固定 uuid 为 `00000000-0000-0000-0000-000000000000`，保证随仓库分发时默认可见。
- 命名策略：创建 / 打开时默认 project key 来自所选目录最后一级 PureDirectoryName，所有非字母数字字符（包括 `.`, `。` 等）归一化为 `-`；显示名默认仍保留最后一级目录原文，用户可覆盖；后端不再剥离 `.BB` 这类未授权语义后缀。
- uuid 策略：project registry 中 uuid 创建后不可变；打开同名已注册 project 时复用原 uuid，新 project 未显式提供 uuid 时由后端生成。

- 来源：inbox/2026-05-08-codebuddy-macos-platform-support.md
- 涉及文件：bb_backend/crates/bb_core/src/lib.rs、bb_backend/crates/bb_server/src/http.rs

- 来源：inbox/2026-05-08-codex-external-project-root.md

# 验证

- 2026-05-08：`cargo test -p bb_core` 通过。
- 2026-05-08：`cargo test -p bb_server` 通过。
- 2026-05-08：`npm run build --prefix bb_web` 通过。

# 下一步

- 用 browser-use 做创建 / 打开主流程冒烟，重点看 Project 下拉底部拆分入口、页面重心创建卡片、Win32 目录选择器回填、项目列表刷新。