# Pregel Topology Mutation Research Results

## Context

本次 research 的问题是：LangGraphJS 是否实现了 Pregel paper 3.4 `Topology Mutations` 中描述的运行时图拓扑变更能力，即在 superstep 中请求新增/删除 vertex 或 edge，并在下一 superstep 按确定性顺序应用。

研究对象：

- Distillation docs: `C:\Users\Administrator\Documents\WXWork\1688856312876561\Cache\File\2026-05\Dev\kb\projects\agentic\langgraphjs`
- Source code: `C:\Users\Administrator\Documents\WXWork\1688856312876561\Cache\File\2026-05\Dev\langgraphjs`

## Executive Finding

LangGraphJS 没有实现 Pregel paper 3.4 意义上的 true runtime topology mutation。

它采用的是：

- 静态图拓扑编译：`StateGraph.addNode/addEdge/addConditionalEdges` 后调用 `.compile()`，得到可执行的 `CompiledStateGraph`。
- 运行时动态调度：通过 `Send` / `Command.goto` 在 superstep 之间派发任务。
- 目标节点约束：`Send` 只能指向已编译进 `processes` 的已有 node，不能在运行中创造新的 node 类型或新拓扑。

因此 LangGraphJS 的实际模型更准确地说是：

```text
static topology + dynamic task dispatch
```

而不是：

```text
dynamic topology mutation
```

## Source Evidence

### 1. Pregel internal task channel

`libs/langgraph-core/src/constants.ts` 定义了内部 task channel 和 task mode：

- `TASKS = "__pregel_tasks"`
- `PUSH = "__pregel_push"`
- `PULL = "__pregel_pull"`

源码位置：

- `libs/langgraph-core/src/constants.ts:55`
- `libs/langgraph-core/src/constants.ts:56`
- `libs/langgraph-core/src/constants.ts:57`

`TASKS` 是 LangGraphJS 用于承载 `Send` packet 的内部 channel。

### 2. Send is dynamic dispatch, not topology mutation

`Send` 的语义是向指定 node 发送一个 task packet，用于 map-reduce、动态 fan-out 等场景。

源码位置：

- `libs/langgraph-core/src/constants.ts:181`

关键点：`Send` 的目标是 node name，但这个 node 必须已经存在于已编译 graph 的 `processes` 中。

### 3. Command.goto lowers into Send / branch writes

`Command` 支持 `resume`、`graph`、`update`、`goto`。

源码位置：

- `libs/langgraph-core/src/constants.ts:447`
- `libs/langgraph-core/src/pregel/io.ts:71`

`mapCommand` 会把 `Command.goto` 降解成以下写入：

- 如果是 `Send`，写入 `[NULL_TASK_ID, TASKS, send]`
- 如果是 string node name，写入 `branch:to:<node>`

源码位置：

- `libs/langgraph-core/src/pregel/io.ts:87`
- `libs/langgraph-core/src/pregel/io.ts:89`

这仍然是控制流/任务调度，不是 add/remove node/edge。

### 4. Send target is validated against compiled processes

`_localWrite` 对 `PUSH` / `TASKS` 写入做校验：

- value 必须是 `Send`
- `value.node` 必须存在于 `processes`
- 不存在则抛出 `InvalidUpdateError`

源码位置：

- `libs/langgraph-core/src/pregel/algo.ts:210`
- `libs/langgraph-core/src/pregel/algo.ts:218`
- `libs/langgraph-core/src/pregel/algo.ts:226`

这说明 runtime 不能通过 `Send` 创建一个尚未编译进 graph 的新 node。

### 5. PUSH task preparation only schedules existing processes

`_prepareNextTasks` 会准备下一轮 superstep 的任务集合，其中包括：

- PUSH tasks: 来自 `TASKS` channel 的 `Send`
- PULL tasks: 来自 channel trigger 的普通 graph node

源码位置：

- `libs/langgraph-core/src/pregel/algo.ts:482`
- `libs/langgraph-core/src/pregel/algo.ts:504`

`_prepareSingleTask` 在处理 PUSH task 时，会从 `TASKS` channel 中读取 `Send` packet，并检查：

- packet 必须是 Send-like object
- `packet.node` 必须存在于 `processes`

源码位置：

- `libs/langgraph-core/src/pregel/algo.ts:741`
- `libs/langgraph-core/src/pregel/algo.ts:745`
- `libs/langgraph-core/src/pregel/algo.ts:761`

如果目标 node 不在 `processes` 中，LangGraphJS 不会把它作为新的 topology mutation 应用，而是忽略/警告。

### 6. TASKS is a reserved internal Topic

`Pregel` 初始化时会保留 `TASKS` channel。如果用户图中存在同名 channel 且不是内部 `Topic`，会报错。

源码位置：

- `libs/langgraph-core/src/pregel/index.ts:601`
- `libs/langgraph-core/src/pregel/index.ts:606`
- `libs/langgraph-core/src/pregel/index.ts:609`
- `libs/langgraph-core/src/pregel/index.ts:610`

这说明 LangGraphJS 把动态任务派发建模为内部 channel write，而不是直接修改 graph topology。

### 7. StateGraph topology is attached at compile time

`StateGraph` 文档明确表达：添加 nodes 和 edges 后必须调用 `.compile()`。

源码位置：

- `libs/langgraph-core/src/graph/state.ts:211`

编译阶段会 attach nodes、edges、branches：

- `compiled.attachNode(START)`
- 对每个 node 调用 `compiled.attachNode(...)`
- 对每条 edge 调用 `compiled.attachEdge(...)`
- 对每个 branch 调用 attach branch

源码位置：

- `libs/langgraph-core/src/graph/state.ts:1169`
- `libs/langgraph-core/src/graph/state.ts:1170`
- `libs/langgraph-core/src/graph/state.ts:1174`
- `libs/langgraph-core/src/graph/state.ts:1189`
- `libs/langgraph-core/src/graph/state.ts:1195`

这进一步说明 LangGraphJS 的 topology 是 build/compile-time concept。

## Comparison With Pregel Paper 3.4

Pregel paper 3.4 `Topology Mutations` 的核心语义包括：

- `Compute()` 可以请求 add/remove vertices 或 edges。
- mutation 不在发起请求的 superstep 立即对全局 graph 生效，而是在下一 superstep 应用。
- 应用顺序是确定性的：
  - edge removal
  - vertex removal
  - vertex addition
  - edge addition
- 冲突通过 partial ordering 和 user-defined handlers 解决。
- local mutations 可以立即生效，因为它们不会引入全局冲突。

LangGraphJS 只采用了 Pregel/BSP 风格的 superstep、channel、message/task dispatch 思想，但没有把 Pregel 3.4 的 topology mutation 作为 runtime feature 实现。

## Research Conclusion

LangGraphJS 是一个 Pregel-style graph execution runtime。它实现了 BSP/superstep、channel write、checkpoint、PUSH/PULL task scheduling、`Send` dynamic dispatch、`Command.goto` control flow 等机制。

LangGraphJS 中的 graph topology 是 build/compile-time concept。nodes、edges、branches 在 `StateGraph` 构建阶段声明，并在 `.compile()` 阶段 attach 到 compiled graph。

LangGraphJS 的 runtime dynamic behavior 主要来自：

- `Send`: 向已存在的 node 派发 task packet。
- `Command.goto`: 指定下一步控制流目标，或通过 `Send` 发送 PUSH task。
- `TASKS`: 内部 reserved channel，用于在 superstep 之间保存待调度的 Send packet。
- `pendingWrites`: checkpoint 中记录尚未完全应用的写入，用于 resume/replay。

LangGraphJS 没有在已检查源码中实现 Pregel paper 3.4 描述的 runtime topology mutation：

- 没有发现运行中 add/remove vertex 的机制。
- 没有发现运行中 add/remove edge 的机制。
- 没有发现 topology mutation request queue。
- 没有发现 edge removal、vertex removal、vertex addition、edge addition 的 barrier ordering。
- 没有发现针对 topology mutation conflict 的 user-defined handler。

因此，对 LangGraphJS 的准确描述是：

```text
static compiled topology + Pregel-style runtime task scheduling
```

而不是：

```text
runtime mutable topology graph
```
