# TaskGraph 060 Mapping: LangGraph Compile + Pregel/Superstep

> 目标：060 不再只是“借鉴” LangGraph，而是在 Blackboard TaskGraph 的图系统里复刻 LangGraph 的底层图编译与 Pregel/superstep 执行模型。命名可以保留 Blackboard 风格，但语义必须对齐。

## Source Scope

LangGraphJS 源码对齐范围：

- `C:\Dev\langgraphjs\libs\langgraph-core\src\graph\graph.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\graph\state.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\pregel\index.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\pregel\read.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\pregel\write.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\pregel\algo.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\pregel\loop.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\base.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\any_value.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\binop.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\ephemeral_value.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\last_value.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\named_barrier_value.ts`
- `C:\Dev\langgraphjs\libs\langgraph-core\src\channels\topic.ts`

已蒸馏文档对齐范围：

- `C:\Dev\kb\projects\agentic\langgraphjs\topics\pregel-execution-model.md`
- `C:\Dev\kb\projects\agentic\langgraphjs\topics\checkpoint-recovery.md`
- `C:\Dev\kb\projects\agentic\langgraphjs\topics\channel-mechanism.md`
- `C:\Dev\kb\projects\agentic\langgraphjs\modules\pregel\index.md`

## Blackboard Module Ownership

2026-05-15 cleanup 后，060 的 Pregel 内核不再允许继续堆进一个 monolith。新增 LangGraph parity 时先落到下面的 owner，再补测试。

| Blackboard module | Owner responsibility |
| --- | --- |
| `task_graph/compiler.rs` | raw TaskGraph -> compiled Pregel IR：processes、channels、triggers、writers、branch/join/state lowering。 |
| `task_graph/pregel/model.rs` | Pregel checkpoint、task、write、prepared step、send packet 数据结构。 |
| `task_graph/pregel/checkpoint.rs` | checkpoint 初始化、checkpoint tuple/config/metadata、channel version diff。 |
| `task_graph/pregel/prepare.rs` | `_prepareNextTasks` 对齐：PULL/PUSH task 生成、pending writes replay、deterministic task path/id。 |
| `task_graph/pregel/writes.rs` | `_applyWrites` 对齐：barrier apply、channel update、consume、channel availability、version bump。 |
| `task_graph/pregel/command.rs` | 节点输出 lowering：StateGraph update mapper、`Command.goto`、`Send`、branch/task writes。 |
| `task_graph/pregel/interrupt.rs` | interruptBefore/interruptAfter 判断、`__interrupt__` / `__resume__` writes、seen marker。 |
| `task_graph/pregel/namespace.rs` | checkpoint namespace 规则：parent/child/task namespace helper。 |
| `task_graph/pregel/runtime_channels.rs` | Pregel reserved runtime channels。 |
| `task_graph/pregel/loop_state.rs` | 长生命周期 loop state：prepare、put_writes、commit_step、recursion limit、loop status。 |
| `task_graph/coordinator.rs` | IO/orchestration adapter：runtime dispatch、run_state persistence、event projection；不应沉淀 Pregel 算法细节。 |

## LangGraph Model

### Compile

LangGraph 的 `Graph.compile()` / `StateGraph.compile()` 做的不是 adjacency list。它把 builder graph 降成 Pregel 可执行结构：

- `nodes/processes`: 每个节点变成 `PregelNode`。
- `channels`: `START`、`END`、state channels、branch channels、join barrier channels。
- `triggers`: 节点订阅哪些 channel；channel 新版本触发节点。
- `writers`: 节点执行后向哪些 channel 写入。
- `inputChannels/outputChannels/streamChannels`: 运行输入、输出、流式输出边界。
- `triggerToNodes`: channel 到候选节点的反向索引，用于下一轮调度优化。

StateGraph 的关键区别：

- `attachNode(START)` 把输入写入 state channels。
- 普通节点订阅 `branch:to:<node>`。
- 普通 edge 写 `branch:to:<end>`。
- waiting edge / join 写 `join:<starts>:<end>`，目标节点等 barrier channel ready 后触发。
- 节点返回值通过 writer 映射成 state channel writes。

### Pregel Superstep

Pregel loop 每轮是：

1. 初始输入写入 `START` channel。
2. `_prepareNextTasks` 准备下一轮任务。
3. 任务按 `PUSH` 和 `PULL` 两类生成。
4. 节点并发执行，只在 task-local writes 里记录输出。
5. 所有任务完成后进入 barrier。
6. `_applyWrites` 统一应用 writes。
7. 更新 `channel_versions` 和 `versions_seen`。
8. checkpoint-first 持久化。
9. interruptAfter / interruptBefore 在 superstep 边界生效。

### Scheduling Truth

LangGraph 的调度事实不是 cursor，而是：

- `checkpoint.channel_values`
- `checkpoint.channel_versions`
- `checkpoint.versions_seen`
- `process.triggers`
- `channel.isAvailable()`

PULL task 的核心条件：

```text
channel is available
and channel_versions[channel] > versions_seen[node][channel]
```

PUSH task 来自 `TASKS` channel 中的 `Send` packet，060 可以先建模，完整动态派发留到后续。

## Parity Milestones

这是一张长程对齐表。任何阶段都不能用“测试过了”冒充“全抄完了”；测试通过只说明当前切片没有破坏现有 TaskGraph。

| Milestone | Scope | Status | Notes |
| --- | --- | --- | --- |
| L0 Compile IR | raw graph -> processes/channels/triggers/writers | in progress | 已有 `CompiledGraph`、`CompiledProcess`、`CompiledWriter`、`trigger_to_nodes`。channel class 已推进到 `EphemeralValue`、`LastValue`、`AnyValue`、`Topic`、`BinaryOperatorAggregate`、`NamedBarrierValue`。 |
| L1 Pregel Checkpoint | `channel_values/channel_versions/versions_seen` | in progress | run 与 superstep checkpoint 已保存 `PregelCheckpoint`。本轮补了 Blackboard `PregelCheckpointTuple`、metadata/source/parents、parentConfig、pendingWrites 组合读取；run.json 若比 checkpoint 文件更新，会优先作为恢复真相。 |
| L2 PULL Superstep | channel version 触发 task | in progress | 已抽出 Blackboard `PregelLoop`，loop 持有 compiled graph、checkpoint、pending writes、step、stop、status，并负责 prepare next step；coordinator 持有长期 loop state，cursor 退为 UI projection。 |
| L3 Apply Writes | barrier apply + channel update | in progress | 已按 channel 分组 writes，更新 versions，返回 updated_channels。`PregelLoop.put_writes` 持有 task-local pending writes，`commit_step` 负责合并 replayed writes + executed writes 并提交 barrier。`LastValue`、`AnyValue`、`Topic`、`BinaryOperatorAggregate`、barrier channel 已有对应 update 语义。 |
| L4 PUSH / Send | `__pregel_tasks` dynamic task | partial | 已能从 `__pregel_tasks` 解析 `{node,args}` 生成 PUSH task；本轮补了 LangGraph-style `Command.goto` / `Send` 输出到 branch / `__pregel_tasks` writes 的映射，并让 PUSH task 的 args 进入本次 node executor 输入。Command graph/subgraph 语义仍未完整。 |
| L5 Interrupt / Resume | interruptBefore/After + resume | partial | HumanGate pause 会写 `__interrupt__`，resume 会写 `__resume__`；graph metadata 可配 `interrupt_before` / `interrupt_after`，`PregelLoop` 用 `versions_seen[__interrupt__]` 避免重复中断。 |
| L6 Recovery | crash-safe pending writes / replay | in progress | 已补 LangGraph-style pending task writes replay MVP；本轮补 checkpoint tuple loader，从 latest superstep checkpoint 或更新的 run.json Pregel checkpoint 恢复，并携带 pendingWrites。成功 task cache / retry 仍未完整。 |
| L7 StateGraph Parity | reducers / Annotation state channels | partial | 已把 graph inputs 编译成 `state:<input_id>` channels；graph input 可显式声明 `reducer` 或 `channel_class`，生成 `BinaryOperatorAggregate` / `Topic` / `AnyValue` / `LastValue`。完整 Annotation/Zod schema factory 仍未复制。 |
| L8 Subgraph / Namespace | nested graph checkpoint namespace | partial | 已引入 LangGraph-style namespace helper：`parent|child` 与 `namespace:taskId`；SubGraph child run 会持久化 `checkpoint_ns`。子图 checkpoint ID 与父 checkpoint 原子绑定仍未做。 |

## Current Blackboard State

已经完成的对齐：

- 2026-05-15 cleanup pass 已把原 `pregel.rs` monolith 拆成 `pregel/model.rs`、`checkpoint.rs`、`prepare.rs`、`writes.rs`、`command.rs`、`interrupt.rs`、`namespace.rs`、`runtime_channels.rs`、`loop_state.rs`、`tests.rs`，公开 API 由 `pregel/mod.rs` 统一出口控制，`task_graph` 顶层不再复制 Pregel re-export。
- `compiler.rs` 生成 `processes/channels/triggers/writers`，不是 adjacency-only。
- compile 生成 `__start__`、`__end__`、`branch:to:<node>`、`join:<starts>:<end>`、`node_outputs.<node>`。
- reserved channels 包含 `__pregel_tasks`、`__pregel_push`、`__pregel_pull`、`__interrupt__`、`__resume__`、`__error__`、`__return__`、`__previous__`。
- `CompiledChannel` 开始记录 LangGraph-style channel class：`EphemeralValue`、`LastValue`、`AnyValue`、`NamedBarrierValue`、`Topic`、`BinaryOperatorAggregate`。
- graph `inputs` 会编译成 `state:<input_id>` channels；`InputVar` 节点读取对应 state channel。
- 非 End process 会带隐藏 state writers，表达 LangGraph `StateGraph.attachNode` 的 `ChannelWriteTupleEntry` / `_getUpdates` 语义。
- `initial_checkpoint` 会把对象输入投影到已有 state channels，让 state 成为 checkpoint truth 的一部分。
- `writes_from_node_outcome` 会把节点返回对象或 `Command.update` 中匹配 state key 的字段写入对应 `state:<key>` channel。
- `writes_from_node_outcome` 会把 `Command.goto` 写成 branch channel，把 `Send` 写成 `__pregel_tasks` packet。
- `apply_writes` 已支持 `AnyValue` last-write-wins，以及 `BinaryOperatorAggregate` 的 append、object merge、sum MVP。
- task writes 会在 barrier checkpoint 前持久化为 `pending_pregel_writes.json`；恢复时 `prepare_next_tasks_with_pending_writes` 会跳过已有成功 writes 的 task，并把 writes 回放进下一次 barrier。
- `pregel/loop_state.rs` 已引入 Blackboard `PregelLoop`，封装 compiled graph、active checkpoint、recovered/current pending writes、step、stop、loop status、prepare next 与 barrier commit。
- graph metadata 可选 `recursion_limit` 已接到 `PregelLoop.stop`，超过上限时 prepare 阶段返回 `PregelLoopStatus::OutOfSteps`。
- graph metadata 可选 `interrupt_before` / `interrupt_after` 已接到 `PregelLoop`；中断判断基于 channel version 是否超过 `versions_seen[__interrupt__]`。
- `PregelCheckpointTuple` 已包含 config、checkpoint、metadata、parentConfig、pendingWrites；coordinator load 使用 tuple 恢复。
- graph inputs 支持显式 `reducer` / `channel_class`，作为 Blackboard 版 Annotation/channel factory 入口。
- `PregelLoopCommit` 已计算 `new_versions`，`writes_committed` event 可看到本轮 channel version 增量。
- SubGraph child run 会记录 LangGraph-style `checkpoint_ns`，命名形态为 `parent|child`。
- run 创建时落 `graph.compiled.json`，run state 保存 `PregelCheckpoint`。
- coordinator 通过 `PregelLoop.prepare_next` 从 checkpoint 推出 runnable tasks。
- node outcome 先变成 Pregel writes，通过 `PregelLoop.put_writes` 进入 loop-local pending writes，barrier 后通过 `PregelLoop.commit_step` 统一提交。
- superstep checkpoint 包含 `pregel_checkpoint`，event log 只是观察层。
- 现有 TaskGraph 后端与 CLI 测试在新调度路径下通过。

仍未完成的关键差距：

- LangGraph `StateGraph` 的 Annotation/Zod schema factory 还没有完整复刻；当前只有 graph input 级显式 reducer/channel_class。
- `DynamicBarrierValue`、`UntrackedValue` 未进入 compiled channel class。
- `Command`、`goto`、`Send` 已进入 node outcome / channel write 映射 MVP；PUSH task args 已进入本次 node executor 输入，Command graph/subgraph/PARENT 仍未完整支持。
- checkpoint recovery 已有 checkpoint tuple + pending writes replay / task id skip MVP；成功 task cache / retry 仍未完整。
- Blackboard `PregelLoop` 目前还不是 LangGraph 完整 `tick()` 状态机：durability mode、checkpointer promise、stream output、managed value、scratchpad/read/write runtime API 仍待继续推进。
- subgraph checkpoint namespace 只有 run 级 namespace 传播；子图 checkpoint ID 与父 checkpoint 原子绑定、retry/cache/stream/debug task envelope 还没对齐。

## Required Alignment Tasks

### A. Compile Layer

- [x] CompiledGraph/process/channel/writer 基本结构。
- [x] reserved Pregel runtime channels。
- [x] branch channel、join barrier channel、node output stream channel。
- [x] channel class 初步对齐：`EphemeralValue`、`LastValue`、`AnyValue`、`NamedBarrierValue`、`Topic`、`BinaryOperatorAggregate`。
- [x] graph inputs -> `state:<input_id>` channels 的 StateGraph MVP。
- [x] graph inputs 显式 `reducer` / `channel_class` -> state channel factory MVP。
- [x] `StateGraph.attachNode` state write mapper MVP：object / `Command.update` -> state channel writes。
- [x] `Command.goto` / `Send` 输出映射到 branch / `__pregel_tasks` writes 的 MVP。
- [ ] `Graph.attachBranch` / `StateGraph.attachBranch` 的 conditional branch runnable 仍未完整进入 IR。
- [ ] StateGraph Annotation/Zod schema 完整编译成 channel factory。

### B. Pregel Checkpoint

- [x] run state 持久化 `PregelCheckpoint`。
- [x] `channel_values` 保存 channel checkpoint value。
- [x] `channel_versions` 保存每个 channel 当前版本。
- [x] `versions_seen` 保存 process 已看到的 trigger channel 版本。
- [x] checkpoint tuple / metadata / parentConfig / pendingWrites 读取。
- [x] run.json Pregel checkpoint 比 superstep checkpoint 更新时，优先从 run.json 恢复。
- [x] run 级 checkpoint namespace 字段。
- [x] pending writes crash-safe replay MVP。

### C. Prepare Next Tasks

- [x] Blackboard 版 `_prepareNextTasks` MVP。
- [x] PULL 根据 channel availability + versions_seen 触发。
- [x] 候选节点由 `updated_channels + trigger_to_nodes` 缩小范围。
- [x] deterministic task path/id MVP。
- [x] PUSH 从 `__pregel_tasks` 读取 Send packets。
- [x] `Command.goto` / `Send` 输出可以生成 branch / PUSH channel writes。
- [x] PUSH task 的 `args` 进入本次 node executor 输入。
- [x] skip-done-tasks / pending successful writes MVP。
- [x] `PregelLoop.prepare_next` 外壳封装 checkpoint + pending writes + step/stop/status。
- [x] graph metadata `recursion_limit` 接入 loop `OutOfSteps` 状态。
- [x] coordinator 持有长期 `PregelLoop`，而不是每个阶段临时重建 loop。
- [x] graph metadata `interrupt_before` 接入 loop `InterruptBefore` 状态。
- [ ] task config / scratchpad / read/write runtime API。

### D. Apply Writes

- [x] task 完成后更新 `versions_seen[task.name][trigger]`。
- [x] 消费已读 trigger channel。
- [x] 按 channel 分组 writes。
- [x] 根据 channel class 应用 update。
- [x] `AnyValue` 与 `BinaryOperatorAggregate` reducer MVP。
- [x] 返回 `updated_channels`。
- [x] 返回 `new_versions`，用于后续 checkpointer put/newVersions。
- [ ] finish() 阶段与 no-next-step channel finalization。
- [ ] LangGraph error channel / no_writes channel 语义。

### E. Coordinator Integration

- [x] coordinator 的 plan 从 Pregel checkpoint 生成。
- [x] cursor 退化为 UI/projection。
- [x] node outcome 转为 channel writes 后统一提交。
- [x] coordinator 的 plan / barrier commit 已接入 `PregelLoop.prepare_next` / `PregelLoop.commit_step`。
- [x] task writes 通过 `PregelLoop.put_writes` 进入 loop-local pending writes，并由持久化层写入 `pending_pregel_writes.json`。
- [x] superstep checkpoint 保存 Pregel checkpoint。
- [x] HumanGate resume 写回 Pregel checkpoint。
- [x] recovery 从 run-local pending writes 跳过已完成 task 的 MVP。
- [x] recovery 从 checkpoint tuple 恢复 checkpoint + pendingWrites。
- [x] SubGraph child run 持久化 `checkpoint_ns`。
- [ ] 子图 checkpoint ID 与父 checkpoint 原子绑定。

### F. Interrupt/Resume

- [x] interruptBefore/interruptAfter 判断基于 channel version 和 triggered/completed tasks。
- [x] 中断时写 `versions_seen[__interrupt__]`。
- [x] HumanGate pause 写 `__interrupt__`；resume 写 `__resume__`。
- [ ] Command resume payload / GraphInterrupt error envelope 完整对齐。

## Acceptance

060 完整对齐时，应满足：

- 一个 TaskGraph run 不依赖 cursor 也能从 Pregel checkpoint 推出下一轮 runnable tasks。
- 每轮节点并发执行，本轮 writes 只在 barrier 后统一进入 channel state。
- 每轮 checkpoint 包含 `channel_values/channel_versions/versions_seen`。
- join 由 barrier channel readiness 驱动，而不是手工 `completed_branches` 计数。
- 事件流只是观察结果，不是执行真相。
- 崩溃后从最后 Pregel checkpoint 恢复，不重复提交已提交 writes。
