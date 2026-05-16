# 草案：Pregel Topology Mutation 后端内核

## 目标

本轮目标是实现 TaskGraph 的 Pregel topology mutation 能力，并同时清理 cursor based 旧管线残留。

约束：

1. 只做后端 Rust 代码。
2. 不做前端 UI。
3. 不把 cursor 作为调度事实、恢复事实或兼容 glue 保留在执行核心中。

本草案只描述后端内核设计和验收边界。

## 背景事实

当前分支已经具备：

- `CompiledGraph`
- Pregel checkpoint
- Pregel task
- Pregel write
- PULL/PUSH task scheduling
- `TASKS_CHANNEL`
- `Send`
- superstep barrier apply writes
- pending writes replay
- superstep checkpoint 持久化

当前缺失：

- graph topology mutation request
- graph revision
- mutation barrier apply
- add/remove node
- add/remove edge
- mutation conflict handling
- mutation 与 checkpoint/replay 的绑定

当前还残留：

- `TaskGraphRun.cursor`
- `SuperstepCheckpoint.cursor_before`
- `SuperstepCheckpoint.cursor_after`
- `ReduceAction::Continue { next_cursor }`
- `NodeOutcome.next_nodes`
- coordinator 内的 `next_cursor` 计算
- HumanGate resume 里的 cursor 更新路径

这些 cursor 残留会导致两套事实源：

```text
Pregel checkpoint/channel state
cursor/next_nodes navigation state
```

本轮要消除这条旧事实链。

## 核心原则

### 1. 一个 superstep 内 graph topology 不变

superstep 开始后，当前 runnable task 集合固定。

节点执行期间不能直接修改当前 compiled graph。

### 2. mutation 在 barrier 生效

节点只能提交 topology mutation request。

mutation 在 superstep barrier 统一收集、排序、校验、应用。

应用完成后生成新的 graph revision。

下一 superstep 基于新的 graph revision 重新 compile 和 prepare tasks。

### 3. checkpoint 必须绑定 graph revision

每个 Pregel checkpoint 必须记录它对应的 graph revision。

恢复时必须先恢复对应 revision 的 graph，再恢复 checkpoint。

不能只靠当前 graph 文件推断历史 checkpoint 应该使用哪版拓扑。

### 4. mutation 是 write，不是直接文件修改

节点、coordinator、runtime node 都不能直接改 graph definition 文件。

所有拓扑变更必须表达为结构化 mutation request，并进入 barrier apply。

### 5. cursor 不能继续作为执行核心状态

执行核心不能继续依赖：

- cursor
- next cursor
- completed branches cursor 兼容路径
- HumanGate resume 的 cursor advance

可保留用于 API 兼容的只读 projection，但不能参与调度、恢复、mutation 或 checkpoint。

如果保留 projection，命名应避免继续叫 cursor，例如：

```text
active_nodes
```

如果本轮只做后端且不管前端，可以先从核心数据结构移除 cursor，再由 API 层后续单独适配。

## 新增核心模型

### GraphRevision

新增 graph revision 概念。

建议结构：

```rust
pub struct GraphRevision {
    pub revision: u64,
    pub graph: TaskGraphDefinition,
    pub created_at: String,
    pub parent_revision: Option<u64>,
    pub mutation_batch_id: Option<String>,
}
```

持久化位置建议：

```text
task_graph_runs/<run_id>/graph_revisions/000000.json
task_graph_runs/<run_id>/graph_revisions/000001.json
task_graph_runs/<run_id>/graph_revisions/000002.json
```

run 创建时：

- graph snapshot 写入 revision `0`
- run state 记录 `current_graph_revision = 0`
- initial Pregel checkpoint 绑定 `graph_revision = 0`

### GraphMutationRequest

新增 mutation request。

建议结构：

```rust
pub struct GraphMutationRequest {
    pub id: String,
    pub source_task_id: String,
    pub source_node_id: String,
    pub op: GraphMutationOp,
    pub reason: Option<String>,
}
```

### GraphMutationOp

建议支持最小集合：

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

`PatchNodeConfig` 不是 Pregel 论文原生项，但对 TaskGraph 必要。它可以用于更新 node config，而不是删除后重建。

### GraphMutationBatch

barrier 阶段把同一个 superstep 收集到的 mutation requests 组成 batch。

建议结构：

```rust
pub struct GraphMutationBatch {
    pub id: String,
    pub superstep: u64,
    pub base_revision: u64,
    pub requests: Vec<GraphMutationRequest>,
    pub result: GraphMutationBatchResult,
}
```

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

默认策略：只要出现无法自动解决的冲突，本 batch rejected，run failed 或 paused。

本轮不实现 Arena，不实现 human review UI。后端可以先把 unresolved conflict 表达成明确错误。

### PregelCheckpoint 增加 graph_revision

`PregelCheckpoint` 增加字段：

```rust
pub graph_revision: u64,
```

含义：

- 当前 checkpoint 是基于哪一版 graph topology 产生的。
- 恢复时必须用相同 revision 的 graph 编译。
- mutation apply 后生成的新 checkpoint 必须记录新 revision。

## Mutation 写入通道

新增 reserved runtime channel：

```text
__pregel_graph_mutations
```

或更短：

```text
__graph_mutations
```

建议使用：

```rust
pub const GRAPH_MUTATIONS_CHANNEL: &str = "__pregel_graph_mutations";
```

该 channel 只接受 `GraphMutationRequest` 或 request array。

它是 reserved runtime channel，不需要出现在 compiled user channels 中。

## NodeOutcome 变化

当前 `NodeOutcome` 有：

```rust
pub next_nodes: Vec<String>
```

本轮应移除该字段。

新增：

```rust
pub graph_mutations: Vec<GraphMutationRequest>
```

节点结果的职责变成：

- 返回 output
- 返回 node state
- 返回 side effects
- 返回 graph mutation requests

控制流跳转不再通过 `next_nodes` 传给 coordinator。

执行边触发应由编译后的 writer 和 Pregel write lowering 决定。

## Control flow 变化

当前控制流仍有旧链路：

```text
node execution
  -> resolve_next_nodes
  -> NodeOutcome.next_nodes
  -> writes_from_node_outcome
  -> branch channel writes
```

目标链路应改为：

```text
node execution
  -> output / command / side effects
  -> writes_from_node_outcome
  -> compiled writers decide channel writes
```

普通 exec edge：

- 非 branch/loop/human gate 节点成功后，应默认触发它的 compiled outgoing exec writers。

Branch 节点：

- 输出或 side effect 表达选中的 target。
- lowering 阶段只写选中 target 对应 channel。

Loop 节点：

- 输出或 side effect 表达 selected target。
- lowering 阶段只写 selected target 对应 channel。

HumanGate：

- pause 不触发 outgoing edge。
- resume 时通过 resume write 写入 gate output。
- resume 后由 compiled writer 触发 outgoing edge。
- 不再手动 update cursor。

End 节点：

- 写 `END_CHANNEL`。

Command.goto：

- 继续支持。
- 如果 goto 是 node name，则写对应 branch/join channel。
- 如果 goto 是 `Send`，写 `TASKS_CHANNEL`。

Send：

- 继续只能发送到已存在的 compiled process。
- 如果 mutation 新增了 node，则必须等下一 revision compile 后，下一 superstep 才能向该 node Send。

## Mutation apply 顺序

参考 Pregel 论文 3.4，barrier apply 顺序固定为：

1. remove edge
2. remove node
3. add node
4. patch node config
5. add edge

说明：

- Pregel 论文没有 `patch node config`，这是 TaskGraph 扩展。
- `patch node config` 放在 add node 后，可以 patch 同 batch 新增的 node。
- add edge 放最后，保证 edge endpoint 已经存在。

## 冲突规则

### 可自动处理

重复 add 相同 node，且 node spec 完全一致：

- dedupe

重复 remove 相同 node：

- dedupe

重复 add 相同 edge，且 edge spec 完全一致：

- dedupe

重复 remove 相同 edge：

- dedupe

remove node 时，该 node 关联 edge：

- 自动删除相关 edge

### 必须拒绝

同 batch add 相同 node id，但 node spec 不同：

- conflict

同 batch add 相同 edge id，但 edge spec 不同：

- conflict

add edge 的 endpoint 不存在：

- conflict

remove node 后又 patch 同 node：

- conflict 或 remove wins

建议本轮选择 conflict，避免隐藏错误。

patch 不存在的 node：

- conflict

remove 不存在的 node：

- conflict

remove 不存在的 edge：

- conflict

### 冲突结果

本轮不做 Arena，也不做 UI gate。

冲突时后端返回明确错误：

```rust
TaskGraphError::TopologyMutationConflict { ... }
```

如果当前错误枚举不方便新增 variant，可以先用 `ValidationFailed`，但 code 必须稳定，例如：

```text
topology_mutation_conflict
```

## 持久化文件

建议新增：

```text
task_graph_runs/<run_id>/graph_revisions/000000.json
task_graph_runs/<run_id>/graph_revisions/000001.json
task_graph_runs/<run_id>/mutation_batches/000001.json
```

`run.json` 建议新增：

```rust
pub current_graph_revision: u64,
```

`SuperstepCheckpoint` 建议新增：

```rust
pub graph_revision_before: u64,
pub graph_revision_after: u64,
pub mutation_batch_id: Option<String>,
```

`PregelCheckpoint` 新增：

```rust
pub graph_revision: u64,
```

## Coordinator 新流程

目标流程：

```text
load run
load graph revision from run.current_graph_revision
compile graph revision
load Pregel checkpoint
assert checkpoint.graph_revision == run.current_graph_revision

loop:
  superstep = checkpoint.superstep + 1
  prepare tasks from checkpoint + compiled graph
  dispatch ready tasks
  lower outcomes to Pregel writes
  include graph mutation writes
  persist pending Pregel writes
  apply normal writes at barrier
  collect mutation requests
  apply topology mutations if any
  if mutation applied:
    write new graph revision
    compile new revision
    update checkpoint.graph_revision
    update run.current_graph_revision
  write superstep checkpoint
  continue
```

关键点：

- graph revision 变化只能发生在 barrier。
- prepare tasks 时只能使用 checkpoint 绑定的 graph revision。
- commit 后如果产生新 revision，下一轮才使用新 compiled graph。

## Cursor 清理目标

### 必须删除或停用

执行核心中不能再使用：

- `TaskGraphRun.cursor`
- `update_cursor`
- `ReduceAction::Continue { next_cursor }`
- `SuperstepPlan.cursor_before`
- `SuperstepCheckpoint.cursor_before`
- `SuperstepCheckpoint.cursor_after`
- `NodeOutcome.next_nodes`

### 可选兼容方式

如果后端 API 暂时需要给前端返回 active nodes，可以新增只读 projection：

```rust
pub active_nodes: Vec<String>
```

但 `active_nodes` 只能来自当前 prepared tasks 或 checkpoint projection，不能反向参与调度。

本轮如果完全不管前端，可以先不新增 projection。

### runner resume 清理

`resume_run` 不能再：

- resolve next edges
- 合并 cursor
- update cursor

它应该：

1. 校验 paused action。
2. 写 resume output。
3. 通过 Pregel write 触发后续 channel。
4. commit 或保存 pending write。
5. 交回 `execute_run`。

## 需要修改的模块

### `definition/types.rs`

可能新增：

- `GraphMutationRequest`
- `GraphMutationOp`
- `GraphRevision`
- `GraphMutationBatch`
- `GraphMutationConflict`

也可以放在新模块：

```text
task_graph/topology/
```

建议放新模块，避免 definition types 继续膨胀。

### `pregel/model.rs`

修改：

- `PregelCheckpoint.graph_revision`

可能新增：

- mutation write shape helper

### `pregel/runtime_channels.rs`

新增：

- `GRAPH_MUTATIONS_CHANNEL`

并加入 reserved runtime channels。

### `pregel/writes.rs`

修改：

- 支持 mutation channel write。
- normal channel writes 与 graph mutation writes 分离。

注意：mutation request 不应该混进普通 `channel_values` 作为下一轮 task trigger。

### `pregel/command.rs`

修改：

- 移除 `next_nodes` 参数依赖。
- 根据 compiled writers 和 node output/command 生成 channel writes。
- 支持从 node output 中提取 graph mutation requests。

### `pregel/outcome.rs`

修改：

- 删除 `ReadyNode` 对 cursor 的依赖。
- 删除 `NodeOutcome.next_nodes`。
- 删除 `ReduceAction::Continue { next_cursor }`。
- 新增 `NodeOutcome.graph_mutations`。
- `SuperstepPlan` 删除 `cursor_before`。

### `pregel/coordinator.rs`

大改：

- load graph revision，而不是只用 graph snapshot。
- prepare plan 不记录 cursor。
- reduce 不计算 next_cursor。
- commit 时应用 graph mutations。
- checkpoint 记录 graph revision。
- 不调用 `update_cursor`。

### `pregel/runner.rs`

修改：

- resume 逻辑删除 cursor 更新。
- HumanGate resume 改为纯 Pregel write。

### `run_state/model.rs`

修改：

- `TaskGraphRun` 删除 `cursor` 或至少执行核心不再引用。
- 新增 `current_graph_revision`。
- `SuperstepCheckpoint` 删除 cursor 字段，新增 graph revision 字段。

### `run_state/lifecycle.rs`

修改：

- `create_run` 写 graph revision 0。
- 不初始化 cursor。
- 删除或停用 `update_cursor`。

### `run_state/superstep.rs`

新增：

- graph revision IO
- mutation batch IO

修改：

- checkpoint tuple recovery 按 graph revision 校验。

### `nodes/*`

修改：

- 所有 node executor 不再调用 `resolve_next_nodes`。
- `NodeOutcome` 不再填写 `next_nodes`。
- Branch/Loop/HumanGate 的选择结果通过 output 或 side effect 表达。

## 测试要求

### 1. Graph revision 初始化

创建 run 后：

- 存在 graph revision 0。
- run.current_graph_revision == 0。
- initial Pregel checkpoint.graph_revision == 0。

### 2. 无 mutation 的旧图仍能跑通

简单线性图：

```text
start -> node -> end
```

应成功。

验证不依赖 cursor。

### 3. Branch 图仍能跑通

Branch 选择某一路时：

- 只触发选中 branch channel。
- 未选中节点不执行。

### 4. Join barrier 仍能跑通

并行 fork/join：

```text
start -> a -> join
start -> b -> join
join -> end
```

join 只能在 a 和 b 都到达后执行。

### 5. Send 仍能派发到已有节点

节点输出 `Send { node, args }` 后：

- 写入 `TASKS_CHANNEL`
- 下一 superstep 准备 PUSH task
- PUSH task 使用 args 作为 task-local input

### 6. Add node mutation

superstep N 中节点提交：

```text
add node review
add edge current -> review
add edge review -> end
```

barrier 后：

- 生成 graph revision N+1。
- checkpoint.graph_revision 更新。
- 下一 superstep 使用新 revision。
- 新 node 能被调度。

### 7. Remove node mutation

删除 node 时：

- 相关 edge 被删除。
- 下一 revision 不包含该 node。
- 不再调度该 node。

### 8. Add edge mutation

新增 edge 后：

- 下一 revision 包含 edge。
- 对应 channel/trigger 在 compile 后存在。

### 9. 冲突测试

同 batch 两个不同 spec 的 `add_node` 同 id：

- batch rejected
- run failed 或返回稳定 conflict error
- 不生成新 graph revision

### 10. Checkpoint replay

mutation 已应用后重启：

- 从 checkpoint 读到 graph_revision。
- 加载对应 graph revision。
- replay 不使用当前最新 graph 猜测历史 topology。

### 11. cursor 删除测试

后端核心测试中不应再依赖：

- `run.cursor`
- `update_cursor`
- `next_cursor`
- `NodeOutcome.next_nodes`

可以用编译期删除字段来强制发现残留。

## 验收标准

本轮完成后，以下命令必须通过：

```powershell
cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml
```

如果改动影响 `bb_cli` 后端 API，也需要通过：

```powershell
cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml
```

本轮不要求：

- `npm run build --prefix bb_web`
- 前端 UI 可用
- schedule UI 适配
- graph editor 支持 mutation 展示

## 本草案的一句话版本

本轮要把 TaskGraph 后端从：

```text
静态 compiled graph + Pregel task dispatch + cursor projection
```

推进到：

```text
graph revision + topology mutation barrier + Pregel checkpoint as sole scheduling truth
```

并删除 cursor based 旧执行链路。
