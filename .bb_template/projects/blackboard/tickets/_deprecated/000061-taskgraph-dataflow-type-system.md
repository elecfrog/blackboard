+++
id = "000061"
lane = "bbt"
title = "TaskGraph 061：State / Channel / Reducer / Checkpoint Data Plane"
created_at = "2026-05-14"
updated_at = "2026-05-15"
status = "todo"
area = "TaskGraph"
assignee = "codex"
attachments = "[{\"kind\":\"wiki\",\"target\":\"proposal/taskgraph-superstep-agent-orchestration.md\",\"label\":\"Superstep TaskGraph Proposal\"}]"
depends_on = "000060"
explicitly_excludes = "business-artifact-taxonomy,explorer-implementer-role-contract,prompt-source"
kind = "langgraph-distillation-state-data-plane"
layer = "data-plane"
parent = "000055"
proposal = "wiki/proposal/taskgraph-superstep-agent-orchestration.md"
requested_by = "user"
rewrite_version = "2026-05-15-corrected-langgraph-boundary"
scope = "channels-state-reducers-versions-seen-checkpoints"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出数据流与类型系统子单。
- 2026-05-15：基于 LangGraphJS `channels`、`checkpoint`、`pregel-task-patterns` 蒸馏重写边界：本 ticket 聚焦 TaskGraph 的 channel/data/artifact 层，承接 #000060 的 superstep 执行内核。

- codex 开始执行：开始实现 dataflow channels 与 artifact contract；先落后端 channel merge/version contract，再同步前端类型。

- codex 完成阶段工作，handoff 写入 `2026-05-15-codex-taskgraph-061-dataflow-channels-and-artifact-contract.md`。

- 2026-05-15：完成 dataflow channels 与 artifact contract 阶段工作——新增 TaskGraph channels 模块（ChannelKind/ChannelValueType/ChannelSpec/ChannelState/ChannelWrite/ArtifactRef/VersionsSeen），实现 apply_channel_writes（last_value/topic/aggregate/barrier/artifact_ref 合并语义），实现 mark_versions_seen/changed_channels_for，扩展 Rust/前端 PinValueType 类型。验证：cargo test channels 5 passed，cargo test task_graph 128 passed，cargo check 与 npm build 通过。

# 背景

TaskGraph 的节点不应该直接互相传一坨字符串。Explorer、Implementer、Verifier、Reviewer、Handoff Writer 产出的内容必须被类型化、可引用、可校验、可审计。LangGraph 的 channel/version/versions_seen 给了一个稳定方向：节点读写的是 channel，系统根据 channel version 决定下一轮哪些节点可运行。

参考 proposal：`wiki/proposal/taskgraph-superstep-agent-orchestration.md`。

# 定位

061 是 TaskGraph 的数据层。它定义 NodeRun 的输入输出、channel、artifact、context 和 schema contract。

061 不负责调度 superstep；调度由 #000060 负责。061 也不负责业务节点 taxonomy；节点分类由 #000062 负责。

# 核心概念

## Typed Pins

每个 node spec 必须声明输入和输出：

- `input_pins`：节点需要读取的 channel 或 artifact ref。
- `output_pins`：节点可能写入的 channel 或 artifact ref。
- `schema`：输入输出的 JSON schema 或内置类型名。
- `required`：缺少时是否阻塞 NodeRun。

第一阶段内置类型：

- `text`
- `markdown`
- `json`
- `file_ref`
- `wiki_ref`
- `ticket_ref`
- `diff`
- `test_result`
- `review_comment`
- `handoff_summary`
- `runtime_log`
- `artifact_ref`

## Channels

建议第一阶段支持这些 channel kinds：

- `last_value`：只保留最新值，例如当前 proposal。
- `topic`：追加消息列表，例如 findings、comments、logs。
- `aggregate`：按 reducer 聚合，例如多个 reviewer 的意见。
- `barrier`：等待指定节点集合完成后释放。
- `artifact_ref`：只存 artifact id/ref，不在 channel 内塞大内容。

每个 channel 至少包含：

- `name`
- `kind`
- `value_type`
- `version`
- `updated_by_node_run_id`
- `updated_at`

## Artifacts

Artifact 是可审计产物，不应该只是日志文本。Artifact 至少包含：

- `id`
- `kind`
- `project`
- `task_run_id`
- `node_run_id`
- `uri` 或 `ref`
- `schema`
- `metadata`
- `created_at`

典型 artifact：

- Explorer findings
- implementation plan
- diff summary
- patch/diff file
- test result
- review comments
- wiki document
- ticket update
- handoff summary

# 实施计划

1. 盘点现有 TaskGraph pins、node_io、context、artifact 表达。
2. 定义 `ValueType`、`PinSpec`、`ChannelSpec`、`ChannelState`、`ArtifactRef`、`Artifact`。
3. 定义 channel version 机制，MVP 使用递增整数，后续兼容 checkpoint parent/fork。
4. 定义 `versions_seen`：每个 NodeRun/NodeSpec 记录已读取 channel version。
5. 实现 validation：编译期校验 pins/schema，运行期校验 node writes。
6. 实现 artifact 存储接口，先支持 wiki/ticket/file/diff/test_result/ref。
7. 实现 channel apply writes：按 channel kind 合并写入，返回 updated channels。
8. 实现 artifact ref 和 ticket attachments 的衔接：ticket 可以引用 wiki/doc/file/run artifact。
9. 给 UI 提供 artifact renderer registry：markdown/json/diff/test_result/link。
10. 接入 #000060：superstep barrier 后调用 dataflow apply writes。

# Task 列表

- [ ] 梳理当前 graph JSON 中 pins、inputs、outputs、context 的实际字段。
- [ ] 定义 Rust 类型：`ValueType`、`PinSpec`、`ChannelKind`、`ChannelSpec`、`ChannelState`。
- [ ] 定义 Rust 类型：`ArtifactKind`、`ArtifactRef`、`ArtifactRecord`。
- [ ] 实现 schema validation，错误结构必须包含 node id、pin id、expected、actual。
- [ ] 实现 `apply_channel_writes`，支持 last_value/topic/aggregate/barrier/artifact_ref。
- [ ] 实现 `versions_seen` 与 channel version 更新。
- [ ] 实现 artifact store MVP，先落 project data root 下的 task run/artifacts 目录或 SQLite。
- [ ] 实现 ticket attachments 与 artifact/wiki/file/ticket ref 的统一结构。
- [ ] 实现前端 artifact renderer registry。
- [ ] 增加单元测试覆盖 channel 合并、版本推进、schema 失败、artifact ref。
- [ ] 增加与 #000060 的 integration test：superstep writes 经过 channel apply 后可触发下一轮。

# 验收标准

- 每个 node spec 可以声明 typed input/output pins。
- 执行时写入错误类型会被拒绝，并形成可读 validation error。
- channel writes 统一在 superstep barrier 后提交。
- channel version 可以驱动下一轮 runnable node selection。
- 大产物不塞进 channel，而是以 artifact ref 引用。
- ticket detail 可以通过 attachments 引用 wiki 文档或其他 ref。

# 依赖关系

- 上游：#000055 TaskGraph Task Execution。
- 上游：#000060 Superstep-based Execution Kernel。
- 下游：#000062 Node Taxonomy 必须使用本 ticket 的 pin/channel/artifact contract。

# 记录

- 验证：cargo test -p bb_core task_graph::channels --lib：通过，5 passed。
- 验证：cargo test -p bb_core task_graph --lib：通过，128 passed。
- 验证：cargo check -p bb_cli：通过。
- 验证：npm run build --prefix bb_web：通过，仅保留既有 chunk size warning。

- 来源：2026-05-15-codex-taskgraph-061-dataflow-channels-and-artifact-contract.md
- 代码位置：bb_backend/crates/bb_core/src/task_graph/channels.rs, types.rs, bb_web/src/data/taskGraphs.ts, bb_web/src/data/taskGraphPins.ts

- 2026-05-15 纠偏重写：本票从“业务 artifact taxonomy / Explorer 产物契约”纠正为 LangGraph 蒸馏的数据平面票。061 只关心 state、channels、reducers、versions_seen、checkpoint payload 和 pending writes 的数据模型。
- 工程边界：artifact、ticket attachment、wiki ref 可以作为 Blackboard 上层产物接入，但不是 061 的核心目标。061 的核心目标是让执行内核有稳定的数据读写语义。
- 验收口径：不是定义 findings/diff/test_result 这些业务名词，而是 channel reducer 行为正确、版本推进可解释、节点是否需要重跑可由 versions_seen 判断。

# 下一步

- 把 coordinator/node outcomes 的 PendingWrite 正式映射到 ChannelWrite，并把 ChannelState 纳入 run checkpoint。

- 定义 channel spec：name、value_type、reducer、default、durability、visibility、schema_ref。
- 定义 channel state：value、version、updated_by、updated_at，并纳入 checkpoint payload。
- 定义 pending write：node_run_id、channel、value、write_kind、superstep、dedupe_key，所有节点输出先写 pending writes。
- 实现 reducers：last_value、topic/append、binary_operator/aggregate、barrier/join、ephemeral，并明确冲突处理策略。
- 实现 versions_seen：记录每个 node 对每个 channel 看到的 version，用于下一轮 runnable 判断和重复执行抑制。
- 实现 checkpoint serializer：保存 channel_values、channel_versions、versions_seen、pending_sends/pending_writes、run cursor。
- 补测试：同一 superstep 多写冲突、append reducer 顺序、aggregate reducer、versions_seen 触发/不触发节点、checkpoint roundtrip。
