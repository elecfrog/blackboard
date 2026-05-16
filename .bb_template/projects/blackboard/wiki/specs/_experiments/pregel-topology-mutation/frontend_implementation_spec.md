# Pregel Topology Mutation 前端实施 Spec

## 状态

本文件是 `implementation_spec.md` 的前端配套 spec。

目标是让前端和后端可以并行推进：

- 后端实现 graph revision、barrier mutation、cursor 清理。
- 前端先基于新数据合约和 mock fixture 改造 Run UI、类型、事件展示和兼容层。

本文件只覆盖 `bb_web`。

## 本轮前端目标

1. 前端不再把 `cursor` 当成运行事实。
2. Run UI 改为基于 `active_nodes`、`current_graph_revision`、`graph_snapshot` 和 run node status 展示。
3. 展示 topology mutation 的结果和失败原因。
4. 展示 graph revision before/after。
5. 支持后端新旧字段并行期的 normalize。
6. 用 mock fixture 先把 UI 做完，不阻塞后端 Rust 合并。

本轮不做：

1. graph editor 里手动编辑 runtime mutation。
2. Arena / Court / Voting UI。
3. mutation conflict 的人工审批 UI。
4. graph revision diff 的完整交互式编辑器。
5. 前端保存运行时 graph revision。

## 当前前端事实

现有相关文件：

```text
bb_web/src/data/taskGraphs.ts
bb_web/src/components/TaskGraphRunPanel.vue
bb_web/src/components/TaskGraphCatalogPanel.vue
bb_web/src/components/task-graph/TaskGraphPreviewPanel.vue
bb_web/src/components/GraphCanvas.vue
bb_web/src/i18n.ts
```

当前 `TaskGraphRunDetail` 仍有：

```ts
cursor: string[]
graph_snapshot: TaskGraphDefinition
nodes: TaskGraphRunNode[]
```

当前 `TaskGraphSuperstepCheckpoint` 仍有：

```ts
cursor_before: string[]
cursor_after: string[]
ready_nodes: string[]
waiting_nodes: string[]
```

当前 `TaskGraphRunPanel.vue` 使用 `run.cursor`：

- 给节点加 `run-cursor` class。
- 计算当前节点摘要。
- 选择默认节点。

这些地方必须迁移到 `active_nodes`。

## 并行开发原则

### 1. 前端先做 normalize 层

后端新字段完成前，前端通过 normalize 同时兼容：

```ts
raw.active_nodes ?? raw.cursor ?? running node ids
raw.current_graph_revision ?? 0
raw.graph_revision ?? raw.current_graph_revision ?? 0
```

UI 层只能读 normalized model，不能直接读 `cursor`。

### 2. UI 文案不再叫 Cursor

所有用户可见文案从：

```text
Cursor
```

改为：

```text
Active Nodes
```

中文：

```text
活跃节点
```

`cursor` 可以短期作为 legacy input 字段存在，但不能继续作为 UI 概念。

### 3. Graph snapshot 表示当前 revision

Run detail 返回的 `graph_snapshot` 在新后端里表示当前 `current_graph_revision` 对应的 graph。

前端不自己推导 mutation 后 topology，也不在浏览器重放 mutation batch。

### 4. Mutation 只展示，不编辑

前端展示：

- mutation batch 是否 applied / rejected。
- revision before / after。
- add/remove/patch summary。
- conflict / compile failure 错误。

前端不允许用户在本轮修 mutation conflict。

## API 数据合约

### TaskGraphRunSummary

新增可选字段：

```ts
export interface TaskGraphRunSummary {
  id: string
  project: string
  graph?: TaskGraphRef & { version?: number }
  graph_ref?: TaskGraphRef & { version?: number }
  status: TaskGraphRunStatus
  created_at: string
  started_at?: string
  updated_at: string
  completed_at?: string
  current_superstep?: number
  last_checkpoint_id?: string
  current_graph_revision?: number
  active_nodes?: string[]
}
```

### TaskGraphRunDetail

目标类型：

```ts
export interface TaskGraphRunDetail {
  id: string
  project: string
  graph_ref: TaskGraphRef & { version: number }
  status: TaskGraphRunStatus
  created_at: string
  started_at?: string
  updated_at: string
  completed_at?: string
  current_superstep?: number
  last_checkpoint_id?: string
  current_graph_revision: number
  active_nodes: string[]
  paused?: TaskGraphRunPaused
  context: TaskGraphRunContext
  graph_snapshot: TaskGraphDefinition
  nodes: TaskGraphRunNode[]
  parent_run_id?: string

  /** Legacy input only. UI must not use this directly. */
  cursor?: string[]
}
```

### TaskGraphSuperstepCheckpoint

目标类型：

```ts
export interface TaskGraphSuperstepCheckpoint {
  id: string
  run_id: string
  superstep: number
  status: TaskGraphSuperstepStatus
  created_at: string
  completed_at?: string
  graph_revision_before: number
  graph_revision_after: number
  mutation_batch_id?: string
  ready_nodes: string[]
  waiting_nodes: string[]
  node_statuses: Record<string, TaskGraphNodeRunStatus>
  context: TaskGraphRunContext
  pending_effects?: TaskGraphPendingTaskEffect[]
  message?: string

  /** Legacy input only. */
  cursor_before?: string[]
  cursor_after?: string[]
}
```

### Pending effect

```ts
export interface TaskGraphPendingTaskEffect {
  task_id: string
  source_node_id: string
  normal_writes: TaskGraphChannelWrite[]
  graph_mutations: GraphMutationRequest[]
}
```

### Graph mutation types

```ts
export type GraphMutationOp =
  | { type: 'add_node'; node: TaskGraphNode }
  | { type: 'remove_node'; node_id: string }
  | { type: 'add_edge'; edge: TaskGraphEdge }
  | { type: 'remove_edge'; edge_id: string }
  | { type: 'patch_node_config'; node_id: string; patch: unknown }

export interface GraphMutationRequest {
  id: string
  source_task_id: string
  source_node_id: string
  op: GraphMutationOp
  reason?: string
}

export interface GraphMutationSummary {
  added_nodes: string[]
  removed_nodes: string[]
  added_edges: string[]
  removed_edges: string[]
  patched_nodes: string[]
}

export interface GraphMutationConflict {
  code: string
  message: string
  request_ids: string[]
  node_id?: string
  edge_id?: string
}

export type GraphMutationBatchResult =
  | {
      status: 'applied'
      new_revision: number
      summary: GraphMutationSummary
    }
  | {
      status: 'rejected'
      conflicts: GraphMutationConflict[]
    }

export interface GraphMutationBatch {
  id: string
  superstep: number
  base_revision: number
  requests: GraphMutationRequest[]
  result: GraphMutationBatchResult
}
```

### RunEvent

保留现有 generic event 类型，但 topology mutation event 的 payload 约定为：

```ts
export interface TopologyMutationEventPayload {
  batch_id: string
  graph_revision_before: number
  graph_revision_after: number
  result: GraphMutationBatchResult
}
```

事件：

```text
kind = "topology_mutation"
```

### 可选新增 endpoints

前端本轮可以不依赖这些 endpoint，但类型和 data client 可以先留接口。

```text
GET /api/projects/:project/task-graph-runs/:runId/graph-revisions
GET /api/projects/:project/task-graph-runs/:runId/graph-revisions/:revision
GET /api/projects/:project/task-graph-runs/:runId/mutation-batches
GET /api/projects/:project/task-graph-runs/:runId/mutation-batches/:batchId
```

最低要求：

- `GET /task-graph-runs/:runId` 返回 current graph snapshot。
- `GET /event-log` 能返回 topology mutation event。
- `GET /checkpoints` 能返回 graph revision before/after。

## Frontend normalize 层

在 `bb_web/src/data/taskGraphs.ts` 增加：

```ts
export function normalizeTaskGraphRunDetail(raw: TaskGraphRunDetail): TaskGraphRunDetail {
  const runningNodes = raw.nodes
    .filter((node) => node.status === 'running' || node.status === 'queued')
    .map((node) => node.node_id)

  return {
    ...raw,
    current_graph_revision: raw.current_graph_revision ?? 0,
    active_nodes: raw.active_nodes ?? raw.cursor ?? runningNodes,
  }
}
```

所有入口都必须调用：

- `readTaskGraphRun`
- `watchTaskGraphRun`
- mock fixture loader

UI 不直接访问 `run.cursor`。

## Run UI 改造

### Header

`TaskGraphRunPanel.vue` header chips 改为：

- Started
- Updated
- Duration
- Superstep
- Revision
- Checkpoint
- Active Nodes

Revision 展示：

```text
Revision 3
```

如果当前 checkpoint 有 before/after：

```text
Revision 2 -> 3
```

Active Nodes 展示来自 `run.active_nodes`。

### Canvas

节点 class 改名：

```text
run-cursor -> run-active-node
```

高亮条件：

```ts
run.value?.active_nodes.includes(node.id)
```

节点状态优先级：

1. selected
2. failed
3. paused
4. running / queued
5. active node
6. succeeded
7. idle

如果 mutation 删除了当前 selected node：

- 清空 selection。
- 优先选 failed node。
- 其次 paused node。
- 其次 first active node。
- 其次 first running node。
- 其次 first visible node。

### Edge

现有 branch decision active edge 保留。

新增 topology mutation 可视规则：

- mutation event 中新增的 edge，在最新一次 mutation 后短暂使用 accent stroke。
- 被删除 edge 不在 current graph snapshot 展示。
- 如需要查看已删除 edge，只在 mutation timeline 里显示，不在 canvas 上保留幽灵边。

### Drawer

右侧 drawer 增加一个 run-level section，而不是塞进 node detail：

```text
Run Timeline
```

内容：

- latest topology mutation event。
- latest checkpoint。
- current graph revision。
- active nodes。

如果 selected node 是当前 revision 新增节点：

- 显示 `Added in revision N`。

如果 selected node 有历史状态但不在当前 `graph_snapshot.nodes`：

- 不在 canvas 展示。
- 在 timeline 中以 retired node 记录展示。

## Mutation Timeline

新增轻量组件：

```text
bb_web/src/components/task-graph/TaskGraphMutationTimeline.vue
```

输入：

```ts
events: TaskGraphRunEvent[]
checkpoints: TaskGraphSuperstepCheckpoint[]
```

展示：

- superstep
- `graph_revision_before -> graph_revision_after`
- batch id
- applied / rejected
- add/remove/patch summary
- conflicts

显示密度要紧凑，适合作为工作台运行视图的一部分。

不要做成大号营销式 card。它是运行调试工具。

## Checkpoint / Event 数据加载

现有 data client 已有：

```ts
readTaskGraphRunEventLog(project, runId)
readTaskGraphRunCheckpoints(project, runId)
```

`TaskGraphRunPanel.vue` 应加载：

- 初次打开 run 时加载 event log 和 checkpoints。
- SSE 收到 run update 后，可以 debounce 刷新 event log/checkpoints。
- terminal status 后最后刷新一次。

建议状态：

```ts
const runEvents = ref<TaskGraphRunEvent[]>([])
const checkpoints = ref<TaskGraphSuperstepCheckpoint[]>([])
```

刷新策略：

- 打开时立即拉。
- SSE run event 到达时 500ms debounce。
- 手动 Refresh 同时刷新 run、events、checkpoints。

## HumanGate resume

现有后端 endpoint：

```text
POST /api/projects/:project/task-graph-runs/:run_id/gates/:node_id/resume
body: { "action": "<action_id>" }
```

当前 UI 里 paused action button 是 disabled。本轮应启用。

新增 data client：

```ts
export async function resumeTaskGraphGate(
  project: string,
  runId: string,
  nodeId: string,
  actionId: string,
): Promise<{ run: TaskGraphRunSummary; source: 'rest' }>
```

交互：

- 点击 action 后 disable 所有 resume buttons。
- POST resume。
- 成功后立刻 reload run。
- SSE 后续继续推进。
- 失败时显示错误，不清空当前 paused panel。

注意：

- 如果后端返回 `paused_node_removed`，展示为 run-level error。
- 不允许前端自行 advance graph。

## Catalog / Run History

Run history 增加可选 revision 展示：

```text
rev 3
```

`TaskGraphRunSummary.current_graph_revision` 缺失时不显示。

Catalog 的 last run status 不需要知道 mutation 细节。

## Editor 边界

TaskGraph editor 继续编辑静态 graph definition。

本轮不允许：

- 从 editor 创建 runtime mutation。
- 保存 run 的 current graph revision。
- 把 `graph_snapshot` 当 project graph 写回。

如果用户从 run UI 点击 Edit：

- 仍打开原始 project graph。
- 不打开 run revision snapshot。

后续如果要支持从 run revision fork，需要单独 spec。

## Mock fixture

前端需要一组 mock run 来并行开发。

新增 fixture 场景：

```text
mock-pregel-mutation-run
```

内容：

1. revision 0：`start -> planner -> end`
2. superstep 1：planner succeeded，提交 add node `review`，add edge `planner -> review`，add edge `review -> end`
3. revision 1：`start -> planner -> review -> end`
4. active_nodes = [`review`]
5. topology mutation event applied
6. checkpoint graph_revision_before = 0, graph_revision_after = 1
```

再加一个 rejected 场景：

```text
mock-pregel-mutation-conflict-run
```

内容：

- add same node id with different spec。
- result rejected。
- run.status = failed。
- conflict code = `topology_mutation_conflict`。

这些 fixture 让前端在后端未完成时就能验证：

- active_nodes 高亮。
- revision chip。
- mutation timeline。
- failed conflict 展示。

## 实施阶段

### Phase 1：类型和 normalize

修改：

- `bb_web/src/data/taskGraphs.ts`
- `bb_web/src/i18n.ts`

产出：

- 新增 mutation/revision TS 类型。
- `cursor` 改 optional legacy。
- 新增 `active_nodes`。
- `readTaskGraphRun` / `watchTaskGraphRun` 调 normalize。
- 文案 `Cursor` 改 `Active Nodes` / `活跃节点`。

### Phase 2：RunPanel 去 cursor

修改：

- `bb_web/src/components/TaskGraphRunPanel.vue`

产出：

- 所有 `run.cursor` 改为 `run.active_nodes`。
- class `run-cursor` 改 `run-active-node`。
- current node summary 改 active node summary。
- default selection 改 active node fallback。
- header 增加 revision chip。

### Phase 3：mutation timeline

新增：

- `TaskGraphMutationTimeline.vue`

修改：

- `TaskGraphRunPanel.vue`
- `taskGraphs.ts`

产出：

- 加载 event log / checkpoints。
- 展示 topology mutation applied/rejected。
- 展示 revision before/after。
- 展示 conflicts。

### Phase 4：HumanGate resume

修改：

- `taskGraphs.ts`
- `TaskGraphRunPanel.vue`

产出：

- resume data client。
- paused action button 可点击。
- resume 后 reload + SSE 跟进。

### Phase 5：mock fixture 和回归

修改：

- `taskGraphs.ts` 中 mock 数据或专门 mock fixture 文件。
- 需要时补 `TaskGraphCatalogPanel.vue` 的 mock entry。

产出：

- applied mutation mock run。
- rejected mutation mock run。
- 不依赖后端即可打开 UI 验证。

## 验收标准

### 构建

必须通过：

```powershell
npm run build --prefix bb_web
```

### 代码检查

这些 grep 不应命中 UI 执行逻辑：

```powershell
rg "run\\.cursor|cursor_before|cursor_after|run-cursor|taskGraphCursor" bb_web/src
```

允许命中：

- `TaskGraphRunDetail.cursor?: string[]` legacy type。
- normalize 函数中的 fallback。
- 注释里明确标记 legacy。

### 手动验证

打开：

```text
http://localhost:8060/#/projects/blackboard/task-graphs
```

验证：

1. 静态 graph 预览不受影响。
2. 普通 run 仍能展示节点状态。
3. mutation mock run 显示 revision。
4. mutation mock run 的 active node 高亮来自 `active_nodes`。
5. topology mutation timeline 显示 applied summary。
6. conflict mock run 显示 rejected conflicts。
7. paused run 的 HumanGate actions 可以点击。
8. 点击 resume 后 UI 不本地推进，只等后端 run update。

## 完成定义

完成后前端从：

```text
run.cursor
  -> 当前节点 UI
  -> canvas 高亮
  -> checkpoint cursor before/after
```

变成：

```text
run.active_nodes
  + current_graph_revision
  + graph_snapshot current revision
  + topology mutation events
  + checkpoint graph_revision before/after
```

可以接受：

- data normalize 中短期 fallback 到 legacy `cursor`。
- 后端 endpoint 未齐全时使用 mock fixture。

不可接受：

- UI 层直接读 `run.cursor`。
- 页面继续展示 `Cursor` 概念。
- 前端试图自己重放 mutation 来生成 current graph。
- graph editor 把 run revision snapshot 写回 project graph。
