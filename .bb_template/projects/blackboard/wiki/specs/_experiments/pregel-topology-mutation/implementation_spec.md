# Pregel Topology Mutation 后端实施 Spec

## 状态

本文件是基于以下输入收敛后的实施版 spec：

- `research_results.md`
- `existing_truth.md`
- `spec_draft.md`
- 五份 review 及 `review-quality-analysis.md`

本文件覆盖后端 Rust 实施计划。开发以本文件为准，`spec_draft.md` 只保留为上一轮草案。

## 本轮目标

实现 TaskGraph 的 Pregel topology mutation 能力，并清掉 cursor based 旧执行链路。

本轮只做：

1. 后端 Rust 代码。
2. Pregel graph revision。
3. barrier 阶段拓扑变更。
4. add/remove node。
5. add/remove edge。
6. patch node config。
7. checkpoint/replay 与 graph revision 绑定。
8. 删除 `cursor` / `next_cursor` / `next_nodes` 在执行核心里的作用。

本轮不做：

1. 前端 UI。
2. graph editor mutation 可视化。
3. Arena / Court / Voting。
4. topology mutation conflict 的 human review gate。
5. delta revision 存储优化。
6. SubGraph 跨父子图 mutation。

## 设计结论

### 最终决策表

| 问题 | 最终选择 |
|---|---|
| mutation 如何进入 barrier | 使用 `NodeOutcome.graph_mutations`，coordinator 直接收集；不新增 `GRAPH_MUTATIONS_CHANNEL` |
| mutation 是否进入普通 channel state | 不进入，不参与 `channel_values` / `channel_versions` / `versions_seen` / trigger 计算 |
| graph revision 存储 | v1 使用完整 `TaskGraphDefinition` snapshot |
| mutation 后 compile 失败 | batch rejected，run failed，不写新 revision |
| mutation conflict | batch rejected，run failed |
|持久化 commit point | `run.json` 原子写入作为生效点 |
| `PatchNodeConfig` 语义 | JSON Merge Patch，接近 RFC 7396 |
| `remove_node + add_node` 同 id | v1 视为 conflict，不支持 replace |
| `Command.goto` / `Send` 到同 batch 新增 node | 非法，必须等下一 revision 编译后才能跳转或 Send |
| Branch/Loop 移除 `next_nodes` 后如何表达选择 | 使用 `Command.goto` 语义 |
| cursor 兼容 | 后端执行核心不保留 cursor；如 API 需要运行态展示，另加 `active_nodes` 投影，但不能叫 cursor，且不能反向参与调度 |
| SubGraph mutation 作用域 | 只作用于当前 run graph context，不允许修改父图 |

## 核心不变量

### 1. Superstep 内 topology 不变

一个 superstep 开始后，当前 runnable task 集合和 compiled graph 固定。

节点执行期间不能直接修改 graph definition，也不能让 coordinator 立即重编译当前 superstep。

### 2. Mutation 只在 barrier 生效

节点只能提交 `GraphMutationRequest`。

coordinator 在 barrier 阶段统一：

1. 收集 mutation requests。
2. 排序。
3. 冲突检测。
4. 应用到 candidate graph。
5. validate。
6. compile 预检。
7. 生成新 revision。
8. 提交 checkpoint。

### 3. Checkpoint 是调度事实源

调度只看：

- compiled graph revision
- `PregelCheckpoint.channel_values`
- `PregelCheckpoint.channel_versions`
- `PregelCheckpoint.versions_seen`
- `PregelCheckpoint.updated_channels`
- pending task effects

不能再看：

- `TaskGraphRun.cursor`
- `next_cursor`
- `NodeOutcome.next_nodes`

### 4. Checkpoint 必须绑定 graph revision

每个 `PregelCheckpoint` 必须记录：

```rust
pub graph_revision: u64,
```

恢复时必须满足：

```text
checkpoint.graph_revision == run.current_graph_revision
```

如果不满足，恢复失败，不能用最新 graph 猜测历史 topology。

### 5. Normal writes 先于 topology mutation

同一个 superstep 内，已完成 task 的普通 writes 是已承诺事实。

即使同 batch mutation 删除了某个下游 node，当前 superstep 已完成 task 的 normal writes 仍然先 apply。删除只影响下一 superstep 的 prepare。

## 新增数据模型

建议新增模块：

```text
bb_backend/crates/bb_core/src/task_graph/topology/
```

模块职责：

- mutation request model
- mutation batch model
- conflict detection
- graph revision IO
- mutation apply
- channel state migration helper

### GraphRevision

```rust
pub struct GraphRevision {
    pub revision: u64,
    pub graph: TaskGraphDefinition,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub parent_revision: Option<u64>,
    pub mutation_batch_id: Option<String>,
}
```

说明：

- v1 每个 revision 保存完整 graph snapshot。
- 这是刻意选择的正确性优先方案。
- 后续可演进为 delta 或 periodic full snapshot。

### GraphMutationRequest

```rust
pub struct GraphMutationRequest {
    pub id: String,
    pub source_task_id: String,
    pub source_node_id: String,
    pub op: GraphMutationOp,
    pub reason: Option<String>,
}
```

约束：

- `id` 在同一个 run 内稳定唯一。
- `source_task_id` 必须对应当前 superstep 的 completed task 或 replayed completed task。
- `source_node_id` 必须等于该 task 的 node id。

### GraphMutationOp

```rust
pub enum GraphMutationOp {
    AddNode { node: TaskGraphNode },
    RemoveNode { node_id: String },
    AddEdge { edge: TaskGraphEdge },
    RemoveEdge { edge_id: String },
    PatchNodeConfig {
        node_id: String,
        patch: serde_json::Value,
    },
}
```

本轮不加 `ReplaceNode`。需要替换节点时使用两个 superstep：

```text
remove node
下一 superstep add node
```

或者使用 `PatchNodeConfig`。

### PatchNodeConfig 语义

`PatchNodeConfig.patch` 使用 JSON Merge Patch 语义：

- object 递归 merge。
- `null` 表示删除 key。
- array 整体替换，不做 append。
- scalar 整体替换。

patch 后必须重新校验完整 `TaskGraphNode` 和完整 `TaskGraphDefinition`。

同 batch 多个 patch 指向同一 node：

- 修改完全相同 path 且值相同：dedupe。
- 修改不相交 path：按 request id 稳定排序后 merge。
- 修改相同 path 但值不同：conflict。
- 任一 patch 使 node config 无效：batch rejected。

### GraphMutationBatch

```rust
pub struct GraphMutationBatch {
    pub id: String,
    pub superstep: u64,
    pub base_revision: u64,
    pub requests: Vec<GraphMutationRequest>,
    pub result: GraphMutationBatchResult,
}
```

`id` 建议：

```text
mutation-batch-<superstep:06>
```

如果同 superstep 未来允许多批，再追加 short uuid。本轮一个 superstep 最多一个 batch。

### GraphMutationBatchResult

```rust
pub enum GraphMutationBatchResult {
    Applied {
        new_revision: u64,
        summary: GraphMutationSummary,
    },
    Rejected {
        conflicts: Vec<GraphMutationConflict>,
    },
}
```

本轮规则：

- `Rejected` 后 run 状态为 `Failed`。
- 不进入 paused。
- 不创建新 graph revision。
- 写入稳定错误 code，方便后续 UI/Arena 接管。

建议错误 code：

```text
topology_mutation_conflict
topology_mutation_compile_failed
topology_mutation_validation_failed
topology_mutation_premature_control_target
```

## Pending effects

现有 pending writes 只覆盖 `PregelWrite`。

本轮需要把 task 完成后的 durable pending 事实扩展成 task effects：

```rust
pub struct PendingTaskEffect {
    pub task_id: String,
    pub source_node_id: String,
    pub normal_writes: Vec<PregelWrite>,
    pub graph_mutations: Vec<GraphMutationRequest>,
}
```

实现方式有两种：

1. 新增 `PendingTaskEffect` 文件格式。
2. 扩展现有 `PendingWrite`，增加 `kind`。

推荐第一种，避免 mutation 被误当成 channel write。

要求：

- task outcome lowering 后，normal writes 与 graph mutations 必须一起 durable。
- crash 后 replay 时，normal writes 和 graph mutations 都必须恢复。
- graph mutations 不进入 `PregelCheckpoint.channel_values`。

## NodeOutcome 变更

当前：

```rust
pub struct NodeOutcome {
    pub next_nodes: Vec<String>,
    ...
}
```

目标：

```rust
pub struct NodeOutcome {
    pub node_id: String,
    pub status: NodeRunStatus,
    pub output: Option<serde_json::Value>,
    pub node_state: TaskGraphRunNode,
    pub side_effects: Vec<SideEffect>,
    pub child_run_id: Option<String>,
    pub end_result: Option<String>,
    pub control: Vec<ControlDirective>,
    pub graph_mutations: Vec<GraphMutationRequest>,
}
```

`next_nodes` 删除。

`control` 是内部表达，语义等价于 LangGraph-style `Command.goto` / `Send`：

```rust
pub enum ControlDirective {
    Goto { target: String },
    Send { node: String, args: serde_json::Value },
}
```

如果实现者不想新增 `ControlDirective`，也可以直接复用现有 `Command` parser。但最终 lowering 语义必须等价。

## Control flow 新协议

### 默认 exec edge

普通非控制节点执行成功后，如果没有显式 `ControlDirective` / `Command.goto`：

- lowering 使用当前 revision 的 compiled process writers。
- 向 compiled outgoing exec writer 写 branch/join channel。

这替代原来的：

```text
resolve_next_nodes -> NodeOutcome.next_nodes -> writes_from_node_outcome
```

### Branch

Branch 节点必须把选择结果表达为一个或多个 `Goto`。

流程：

```text
Branch executor
  -> evaluate branch condition
  -> produce ControlDirective::Goto { target }
  -> lowering writes branch/join channel for target
```

Branch 不能再返回 `next_nodes`。

### Loop

Loop 节点用同一协议：

```text
continue body -> Goto { target: loop_body }
exit loop     -> Goto { target: loop_exit }
```

Loop frame / iteration state 仍可通过 `SideEffect` 记录。

### Command.goto

`Command.goto` 只能指向当前 graph revision 已经编译存在的 node。

如果同一个 superstep 中：

```text
add node X
Command.goto X
```

这是非法时序。因为 X 尚未进入当前 compiled graph。

处理方式：

- lowering 阶段产生 stable error。
- mutation batch 不应被用于修复当前 superstep 的 goto。
- 需要跳转到新 node 时，必须先 mutation，下一 superstep 再 goto 或通过新增 edge 激活。

### Send

`Send` 只能发送到当前 compiled `processes` 中已存在的 node。

如果同一个 superstep 中：

```text
add node X
Send(X)
```

这是非法时序，报 `topology_mutation_premature_control_target`。

### HumanGate resume

`resume_run` 不能再：

- resolve next edges。
- 合并 cursor。
- update cursor。

新流程：

```text
resume_run
  -> 校验 paused action
  -> 写 resume value / resume write
  -> durable pending effect
  -> 触发 coordinator tick
```

resume 后是否前进，由 Pregel writes 和 checkpoint 决定。

### End

End 节点继续写 `END_CHANNEL`，并由 reducer 把 run 标记为 completed。

不需要 cursor。

## Mutation barrier 流程

每个 superstep 的 commit 流程固定为：

```text
1. 收集 completed tasks 和 replayed tasks 的 PendingTaskEffect
2. 从 PendingTaskEffect 分离 normal_writes 和 graph_mutations
3. apply normal_writes 到 checkpoint
4. 从 graph_mutations 构造 GraphMutationBatch
5. 如果 batch 为空：
   - graph_revision_after = graph_revision_before
   - 写普通 superstep checkpoint
   - 结束
6. 对 batch 做冲突检测和 canonicalize
7. 在内存中应用到 candidate graph
8. validate candidate graph
9. compile candidate graph
10. 迁移 checkpoint channel state
11. 写 new graph revision
12. 写 mutation batch result
13. 写 superstep checkpoint
14. 原子更新 run.json
```

关键点：

- normal writes 一定先 apply。
- mutation 不参与 normal channel reducer。
- compile 失败发生在写新 revision 之前。
- commit 前所有 candidate 都只在内存中。

## Mutation apply 顺序

冲突检测先于 apply 顺序。

通过冲突检测后，apply 顺序固定：

1. remove edge
2. remove node
3. add node
4. patch node config
5. add edge

说明：

- `remove_node` 会自动展开关联 edge 删除。
- 自动展开的 edge removal 标记为 system generated。
- `patch_node_config` 放在 add node 后，允许 patch 同 batch 新增 node。
- `add_edge` 放最后，保证 endpoint 已存在。

## 冲突规则

### 可自动处理

同 batch 重复 add 相同 node，且 spec 完全一致：

- dedupe。

同 batch 重复 remove 相同 node：

- dedupe。

同 batch 重复 add 相同 edge，且 spec 完全一致：

- dedupe。

同 batch 重复 remove 相同 edge：

- dedupe。

remove node 的关联 edge：

- 自动删除。

同 batch 多个 patch 修改同一 node 的不相交 path：

- deterministic merge。

### 必须 reject

add 相同 node id，但 node spec 不同：

- conflict。

remove node + add node 同 id：

- conflict。

add 相同 edge id，但 edge spec 不同：

- conflict。

remove edge + add edge 同 id：

- conflict。

add edge endpoint 在 candidate graph 中不存在：

- conflict。

remove node 后 patch 同 node：

- conflict。

patch 不存在的 node：

- conflict。

remove 不存在的 node：

- conflict。

remove 不存在的 edge：

- conflict。

Command.goto / Send 指向同 batch 新增 node：

- conflict 或 lowering error。
- 本轮统一当作 stable failure。

## Channel state 迁移

mutation apply 后会产生新 compiled graph。checkpoint 需要从 old compiled graph 迁移到 new compiled graph。

### 保留 channel

old 和 new 都存在的 channel：

- value 保持。
- version 保持。
- `versions_seen` 中该 channel 的记录保持。

### 删除 channel

new 不存在的 channel：

- 从 `channel_values` 删除。
- 从 `channel_versions` 删除。
- 从 `updated_channels` 删除。
- 从每个 node 的 `versions_seen` 删除。

### 新增 channel

new 存在、old 不存在的 channel：

- 初始化为 channel class 的 zero value。
- 默认 version = `0`。
- 默认不加入 `updated_channels`。

zero value 规则：

| Channel class | 初始 value |
|---|---|
| `EphemeralValue` | 不写入 `channel_values` 或 `Value::Null` |
| `LastValue` | 不写入 `channel_values` |
| `AnyValue` | 不写入 `channel_values` |
| `Topic` | `[]` |
| `BinaryOperatorAggregate` | reducer zero value |
| `NamedBarrierValue` | 空 barrier state |

### 新增 exec edge 的激活规则

如果同一个 superstep 中某个 completed task 请求新增从自己出发的 exec edge：

```text
source_node_id == edge.from
```

则该新增 exec edge 的 trigger channel 在迁移时被 seed 一次：

- 写入与 compiled writer 等价的 source value。
- channel version 设置为当前 committed superstep。
- channel 加入 `updated_channels`。

这样可以支持常见场景：

```text
coordinator 节点执行
  -> add node review
  -> add edge coordinator -> review
barrier
下一 superstep 调度 review
```

如果新增 edge 的 `from` 不是当前 completed source node：

- 不自动激活。
- 只改变 topology。
- 未来需要由真实 writer 或 Send 触发。

### Join channel 迁移

如果新增或删除 exec edge 导致 join channel 名称或 required_senders 变化：

- old join channel 删除。
- new join channel 按新增 channel 初始化。
- 不尝试把旧 partial barrier state 迁移到新 join channel。

如果新增 edge 是 source node 本 superstep 发出的 local outgoing edge，则只 seed 该 source sender；join 仍必须等其他 required senders 满足。

## 持久化

### 新增路径

```text
task_graph_runs/<run_id>/graph_revisions/000000.json
task_graph_runs/<run_id>/graph_revisions/000001.json
task_graph_runs/<run_id>/mutation_batches/000001.json
task_graph_runs/<run_id>/pending_task_effects/<superstep>/<task_id>.json
```

文件名使用 revision 或 superstep 的 6 位数字，方便人工定位。

### TaskGraphRun

新增：

```rust
pub current_graph_revision: u64,
```

删除或停用：

```rust
pub cursor: Vec<String>,
```

如果为了 serde 兼容短期保留字段：

- 不能写入。
- 不能读取参与调度。
- 不能由 coordinator 更新。
- 后续单独删除。

### SuperstepCheckpoint

删除：

```rust
pub cursor_before: Vec<String>,
pub cursor_after: Vec<String>,
```

新增：

```rust
pub graph_revision_before: u64,
pub graph_revision_after: u64,
pub mutation_batch_id: Option<String>,
```

### PregelCheckpoint

新增：

```rust
pub graph_revision: u64,
```

### RunEvent

新增 topology mutation event：

```rust
RunEvent::TopologyMutation {
    superstep: u64,
    batch_id: String,
    result: GraphMutationBatchResult,
}
```

如果现有 `RunEvent` 是 string kind + JSON payload，则使用：

```text
kind = "topology_mutation"
payload.batch_id
payload.result
payload.graph_revision_before
payload.graph_revision_after
```

## 崩溃一致性

`run.json` 原子写入是本轮 commit point。

写入顺序：

```text
1. graph_revisions/<new_revision>.json.tmp
2. rename -> graph_revisions/<new_revision>.json
3. mutation_batches/<batch>.json.tmp
4. rename -> mutation_batches/<batch>.json
5. superstep checkpoint tmp
6. rename -> superstep checkpoint final
7. event append
8. run.json.tmp
9. rename -> run.json
```

恢复规则：

- 以 `run.json.current_graph_revision` 和 `run.json.last_checkpoint_id` 为准。
- 未被 run.json 引用的 revision / batch / checkpoint 是 orphan，可以忽略。
- 如果 run.json 引用了缺失 revision 或 checkpoint，恢复失败。

compile / validate / conflict detection 失败时：

- 不写新 graph revision。
- 可以写 rejected mutation batch 和 failed event。
- run.status = Failed。

## 旧 run 兼容

本分支仍可能存在旧 run 文件。

最低兼容策略：

1. 如果 run 缺少 `current_graph_revision`，加载时补 `0`。
2. 如果缺少 `graph_revisions/000000.json`，从 run 绑定的 graph snapshot 写出 revision 0。
3. 如果旧 `PregelCheckpoint` 缺少 `graph_revision`，按 `0` 读取。
4. 如果旧 `SuperstepCheckpoint` 缺少 revision 字段，按 `0 -> 0` 读取。
5. cursor 字段即使存在也只作为历史字段忽略。

如果实现成本过高，可以选择更严格策略：

- 旧 run 不支持 resume。
- 但必须给出稳定错误 code。

默认推荐做最低兼容策略，因为本地 workspace 里可能已有 paused run。

## Cursor 删除范围

### 必须删除执行依赖

以下项不能再被 coordinator / runner / executor 用于调度：

- `TaskGraphRun.cursor`
- `run_state::update_cursor`
- `SuperstepPlan.cursor_before`
- `SuperstepCheckpoint.cursor_before`
- `SuperstepCheckpoint.cursor_after`
- `ReduceAction::Continue { next_cursor }`
- `NodeOutcome.next_nodes`
- `resolve_next_nodes` 在 Pregel 执行路径中的调用

### 可保留但不能使用

如果短期为了反序列化保留 `cursor` 字段：

- 使用 `#[serde(default)]`。
- 标记 deprecated。
- coordinator 不读不写。
- 测试必须覆盖 cursor 不影响调度。

### active_nodes

如果后端 API 需要给调用方展示当前 active nodes，新增：

```rust
pub active_nodes: Vec<String>
```

来源只能是：

- prepared tasks。
- checkpoint projection。
- paused action。

`active_nodes` 不能写回 checkpoint，不能参与调度。

## SubGraph 作用域

SubGraph 内部产生的 topology mutation 只作用于 child run 的 graph revision。

禁止：

- child run 修改 parent graph。
- parent run 直接修改 child graph。
- mutation 穿透 checkpoint namespace。

如果未来要支持跨图 mutation，必须新增显式 op 和权限模型。本轮不做。

## Interrupt 交互

`interrupt_before` / `interrupt_after` 是 run-level 执行配置，不属于 graph revision。

mutation add node 后：

- 下一 superstep compile new revision。
- `PregelLoop` 使用同一份 run-level interrupt config。
- 如果新增 node 命中 interrupt list，照常 interrupt。

暂停态 node 被 mutation 删除：

- 该 paused action 标记为 cancelled 或 run failed。
- 不允许 resume 一个已经不在 current graph revision 中的 node。

本轮推荐简单策略：

- resume 时发现 paused node 不在 current revision：run failed，错误 code `paused_node_removed`。

## Coordinator 新流程

```text
load run
load graph revision run.current_graph_revision
compile graph revision
load Pregel checkpoint tuple
assert checkpoint.graph_revision == run.current_graph_revision

loop:
  prepare_next_tasks(compiled, checkpoint, pending effects)
  if no tasks and no replayed effects:
    decide completed/idle/failed by checkpoint status

  dispatch tasks
  collect NodeOutcome
  lower outcomes:
    - normal PregelWrite
    - graph mutations
    - run/node side effects
  persist PendingTaskEffect

  apply normal writes
  collect mutation batch
  if mutation batch exists:
    conflict detect
    apply to candidate graph in memory
    validate candidate graph
    compile candidate graph
    migrate checkpoint channel state
    write revision/batch/checkpoint/event
    update run.current_graph_revision
    replace compiled graph for next loop
  else:
    write checkpoint/event/run

  clear committed pending effects
```

## 模块实施顺序

### Phase 1：模型和持久化骨架

修改：

- `task_graph/topology/*`
- `pregel/model.rs`
- `run_state/model.rs`
- `run_state/superstep.rs`
- `run_state/lifecycle.rs`

产出：

- mutation/revision 类型。
- graph revision 0 初始化。
- `current_graph_revision`。
- checkpoint revision 字段。
- revision IO。
- batch IO。

先不接 coordinator。

### Phase 2：PendingTaskEffect 和 NodeOutcome

修改：

- `pregel/outcome.rs`
- `pregel/coordinator.rs`
- `run_state/superstep.rs`

产出：

- 删除 `NodeOutcome.next_nodes`。
- 新增 `NodeOutcome.control`。
- 新增 `NodeOutcome.graph_mutations`。
- pending normal writes 和 graph mutations durable。

### Phase 3：Control flow 无 cursor

修改：

- `pregel/command.rs`
- `nodes/control/branch_node.rs`
- `nodes/control/loop_node.rs`
- `nodes/control/simple_nodes.rs`
- `nodes/runtime/llm_node.rs`
- `nodes/runtime/shell_node.rs`
- `nodes/subgraph.rs`
- `pregel/runner.rs`

产出：

- Branch/Loop 改为 `Command.goto` / `ControlDirective::Goto`。
- 普通节点默认 compiled writers。
- `Send` / `Command.goto` 目标校验。
- HumanGate resume 不 update cursor。

### Phase 4：Mutation apply

修改：

- `task_graph/topology/apply.rs`
- `task_graph/topology/conflict.rs`
- `task_graph/topology/channel_migration.rs`
- `pregel/coordinator.rs`
- `pregel/loop_state.rs`

产出：

- normal writes 与 mutation effects 分离。
- conflict detection。
- candidate graph apply。
- validate + compile 预检。
- channel state migration。
- revision commit。

### Phase 5：删除 cursor 残留和测试回归

修改：

- `run_state/lifecycle.rs`
- `task_graph/mod.rs`
- tests

产出：

- coordinator 不调用 `update_cursor`。
- runner 不调用 `resolve_next_nodes`。
- 测试不依赖 cursor。
- 如仍保留字段，只作 serde 历史兼容。

## 测试要求

### 基础回归

1. create run 写出 revision 0。
2. `run.current_graph_revision == 0`。
3. initial `PregelCheckpoint.graph_revision == 0`。
4. 无 mutation 的线性图跑通。
5. Branch 图跑通。
6. Loop 图跑通。
7. fork/join barrier 图跑通。
8. Send 到已有 node 跑通。
9. interrupt_before / resume 跑通。
10. HumanGate resume 不依赖 cursor。

### Control flow

1. Branch 选择 target 后只写选中 target 的 trigger channel。
2. Loop continue/exit 都通过 `Goto` 触发。
3. `Command.goto` 到当前 revision 存在 node 成功。
4. `Command.goto` 到不存在 node 失败。
5. `Send` 到当前 revision 存在 node 成功。
6. `Send` 到不存在 node 失败。

### Mutation

1. add node + add local outgoing edge 后，下一 superstep 调度新 node。
2. add node 但无激活 edge，不调度新 node。
3. add edge from 非当前 completed source，只改变 topology，不立即激活。
4. remove node 自动删除关联 edge。
5. remove node 后下一 superstep 不再调度该 node。
6. add edge 后 new revision compile 产生对应 channel/trigger。
7. patch node config 使用 JSON Merge Patch。
8. patch 后 config invalid，则 batch rejected。

### Conflict

1. add same node id different spec rejected。
2. remove + add same node id rejected。
3. add edge endpoint missing rejected。
4. remove missing node rejected。
5. patch missing node rejected。
6. same node same path different patch rejected。
7. compile candidate graph failed rejected。
8. rejected batch 不生成 new revision。
9. rejected batch run.status = Failed。

### Checkpoint/replay

1. mutation applied 后重启，按 checkpoint.graph_revision 加载 graph。
2. 存在 orphan revision 但 run.json 未引用，恢复忽略。
3. run.json 引用缺失 revision，恢复失败。
4. pending task effect replay 同时恢复 normal writes 和 graph mutations。
5. mutation 后新增 channel 的 version / updated_channels 行为符合激活规则。
6. join channel required_senders 改变后不迁移旧 partial barrier state。

### Cursor 清理

1. `NodeOutcome.next_nodes` 编译期不存在。
2. `ReduceAction::Continue { next_cursor }` 编译期不存在。
3. Pregel coordinator 不调用 `update_cursor`。
4. Pregel runner 不调用 `resolve_next_nodes`。
5. `SuperstepCheckpoint` 不再写 cursor_before/cursor_after。
6. 即使旧 run 有 cursor 字段，也不影响新调度。

## 验收命令

必须通过：

```powershell
cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml
```

如果改动触及 CLI API：

```powershell
cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml
```

本轮不要求：

```powershell
npm run build --prefix bb_web
```

因为本轮明确不做前端。

## 实施完成定义

完成后系统应从：

```text
static compiled graph
  + Pregel task scheduler
  + cursor projection glue
```

变成：

```text
graph revision
  + Pregel checkpoint as scheduling truth
  + barrier topology mutation
  + no cursor execution chain
```

可以接受：

- API 层短期还有非调度用 `active_nodes`。
- 旧 run 通过 migration 补 revision 0。
- revision v1 使用 full snapshot。

不可接受：

- coordinator 仍计算 next_cursor。
- runner resume 仍 update cursor。
- Branch/Loop 仍依赖 `next_nodes`。
- mutation request 进入普通 channel state。
- mutation 后 compile 失败仍写入新 revision。
- checkpoint 不知道自己对应哪个 graph revision。
