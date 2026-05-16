# 现状事实：TaskGraph 后端核心与节点运行时

## 范围

本文件记录当前分支相对 `main`，在 TaskGraph 后端核心和节点运行时上已经落地的事实。

对比基线：

- 当前分支：`feature/orni`
- 当前提交：`5c3c84a feat: pregel task engine`
- 对比主分支：`main`

纳入范围：

- `bb_backend/crates/bb_core/src/task_graph`
- TaskGraph 后端执行核心
- TaskGraph 节点运行时

排除范围：

- 前端 UI
- 定时任务 UI
- AgentChat UI
- 产品判断
- 后续方案建议

## 总结

当前分支已经把 TaskGraph 后端执行核心，从 `main` 上的 cursor 驱动解释器，重构成了 Pregel 风格的执行内核。

当前执行链路是：

```text
graph definition
  -> compiled graph IR
  -> Pregel checkpoint / channel / task scheduler
  -> superstep barrier apply writes
  -> durable run projection
```

但是，当前分支没有实现 Pregel 论文 3.4 描述的运行时拓扑变更。它支持把任务动态派发给已经编译存在的节点，但不支持运行中新增或删除 graph node / edge。

准确描述是：

```text
静态编译拓扑 + Pregel 风格运行时任务调度
```

不是：

```text
运行时可变图拓扑
```

## 主分支原貌

`main` 上的 TaskGraph 后端执行核心主要由这些模块组成：

- `interpreter.rs`
- `coordinator.rs`
- `executor.rs`
- `node_exec/`
- `outcome.rs`
- `run_state/`
- `runtime/`
- `types.rs`
- `validation/`

当时的执行模型是 cursor 驱动：

1. run 持有 `cursor`。
2. coordinator 从 `cursor` 计算 ready nodes。
3. executor 执行 ready nodes。
4. reducer 根据节点结果更新 `cursor`、node state、node output、run context。

`main` 上没有独立的 compiled graph IR、Pregel checkpoint、channel version、`versions_seen`、pending Pregel writes、PULL/PUSH task model。

## 已实现：后端执行核心

### 1. TaskGraph 模块边界重组

当前分支把 TaskGraph 后端拆成了更清楚的模块：

- `definition/`：graph 定义、store、upgrade、pins。
- `validation/`：graph 校验和运行前校验。
- `compile/`：graph 到可执行 IR 的编译。
- `pregel/`：Pregel 风格执行内核。
- `nodes/`：具体节点执行。
- `run_state/`：持久化运行态、checkpoint、event、artifact。
- `runtime/`：运行时进程辅助逻辑。
- `schedules/`：后端定时任务存储和触发逻辑。

证据：

- `bb_backend/crates/bb_core/src/task_graph/mod.rs`
- `bb_backend/crates/bb_core/src/task_graph/README.md`

### 2. 新增 `CompiledGraph` IR

当前分支新增 `CompiledGraph`，把原始 `TaskGraphDefinition` 降低成运行时结构。

`CompiledGraph` 包含：

- `entrypoint`
- `input_channels`
- `output_channels`
- `stream_channels`
- `reserved_channels`
- `nodes`
- `processes`
- `channels`
- `trigger_to_nodes`
- `exec_outgoing`
- `exec_incoming`
- `data_edges`
- `join_nodes`

证据：

- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:25`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:158`

### 3. 编译阶段生成 channel 和 process

当前分支的 compile step 会生成：

- start/end reserved channels
- 来自 graph inputs 的 state channels
- 普通执行边对应的 branch channels
- 多入边 join 节点对应的 named barrier channels
- data edges 对应的 data channels
- node output channels
- `trigger_to_nodes` 索引

证据：

- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:252`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:253`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:259`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:260`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:261`

### 4. 新增 Pregel 核心数据结构

当前分支新增这些 Pregel 结构：

- `PregelCheckpoint`
- `PregelCheckpointConfig`
- `PregelCheckpointMetadata`
- `PregelCheckpointTuple`
- `PregelTaskKind`
- `PregelTask`
- `PregelWrite`
- `PregelPreparedStep`
- `PregelSend`

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:9`
- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:47`
- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:52`
- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:61`
- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:77`

### 5. 调度事实变成 Pregel checkpoint

`PregelCheckpoint` 持有：

- `channel_values`
- `channel_versions`
- `updated_channels`
- `versions_seen`
- `superstep`

调度器根据 channel version 和 `versions_seen` 判断哪些 PULL task 可以运行。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/model.rs:9`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:14`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:96`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:130`

### 6. 已实现 PULL/PUSH task 准备

当前分支实现了两类 Pregel task：

- PULL task：由 channel trigger 和 version diff 激活。
- PUSH task：由 `TASKS_CHANNEL` 中的 `Send` packet 激活。

PUSH task 只能调度已经存在于 compiled `processes` 的节点。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:21`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:150`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:165`

### 7. 已实现 superstep barrier 写入

当前分支实现了 Pregel 风格的 barrier apply：

- completed tasks 更新 `versions_seen`
- 先消费触发本轮 task 的 channel
- 按 channel 分组应用 writes
- 根据 channel class 更新 channel value
- 更新 channel version
- 计算下一轮的 `updated_channels`

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:17`
- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:92`
- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:240`
- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:393`

### 8. 已实现 channel class 和 reducer

当前分支支持这些 channel class：

- `EphemeralValue`
- `LastValue`
- `AnyValue`
- `Topic`
- `BinaryOperatorAggregate`
- `NamedBarrierValue`

当前 reducer 支持：

- append
- merge object
- sum

证据：

- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:123`
- `bb_backend/crates/bb_core/src/task_graph/compile/compiler.rs:142`
- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:308`

### 9. 已实现 pending Pregel writes replay

当前分支支持保存 pending Pregel writes。恢复时可以跳过已经完成的 task，并重放该 task 的 writes。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:24`
- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:138`
- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:131`
- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:171`
- `bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs:182`

### 10. 新增 `PregelLoop`

当前分支新增 `PregelLoop`，负责持有 active checkpoint、pending writes、step preparation 和 barrier commit。

`PregelLoop` 支持：

- 准备下一轮 superstep
- 写入 task writes
- 提交 superstep
- 清理已经 commit 的 pending writes
- stop / recursion limit
- `interrupt_before` / `interrupt_after` 状态

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:45`
- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:131`
- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:164`
- `bb_backend/crates/bb_core/src/task_graph/pregel/loop_state.rs:171`

### 11. Coordinator 改为通过 `PregelLoop` 驱动

当前分支的 `GraphCoordinator` 执行过程是：

1. 读取 graph snapshot。
2. 编译 graph。
3. 初始化或恢复 Pregel checkpoint tuple。
4. 创建 `PregelLoop`。
5. 准备 superstep plan。
6. 执行 ready nodes。
7. 把 node outcome 降低成 Pregel writes。
8. 持久化 node/run projection。
9. 提交 Pregel barrier checkpoint。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:40`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:57`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:330`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:434`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:687`

### 12. Cursor 仍存在，但主要是投影

`cursor` 仍然存在于 run state，也仍由 coordinator 更新。

但是当前分支的调度路径已经主要由 Pregel checkpoint / channel state 准备 task。`cursor` 更像是 UI 和兼容层投影，同时也用于暂停态兼容。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:178`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:637`
- `bb_backend/crates/bb_core/src/task_graph/run_state/model.rs:273`

### 13. 已实现 `Command` / `Send` 降低

当前分支支持节点输出中的 LangGraph-like 结构：

- `Send`
- `Command`
- `Command.goto`
- `Command.update`

`Send` 会降低成写入 `TASKS_CHANNEL`。`Command.goto` 在目标存在于 compiled graph 时，会降低成 branch/task writes。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/command.rs:15`
- `bb_backend/crates/bb_core/src/task_graph/pregel/command.rs:21`
- `bb_backend/crates/bb_core/src/task_graph/pregel/command.rs:107`
- `bb_backend/crates/bb_core/src/task_graph/pregel/command.rs:248`
- `bb_backend/crates/bb_core/src/task_graph/pregel/command.rs:337`

### 14. 已实现 interrupt before/after

当前分支支持：

- `interrupt_before`
- `interrupt_after`
- `__interrupt__`
- `__resume__`

`interrupt_before` 可以在匹配节点执行前暂停。resume 后继续原本的 task 执行。

证据：

- `bb_backend/crates/bb_core/src/task_graph/pregel/interrupt.rs:8`
- `bb_backend/crates/bb_core/src/task_graph/pregel/interrupt.rs:41`
- `bb_backend/crates/bb_core/src/task_graph/pregel/interrupt.rs:52`
- `bb_backend/crates/bb_core/src/task_graph/pregel/coordinator.rs:385`
- `bb_backend/crates/bb_core/src/task_graph/pregel/runner.rs:115`

### 15. 持久化运行态扩展

当前分支在 run state 中增加了 Pregel/superstep 相关字段和文件：

- `current_superstep`
- `last_checkpoint_id`
- `pregel_checkpoint`
- `checkpoint_ns`
- `SuperstepCheckpoint`
- `RunEvent`
- `PendingWrite`

证据：

- `bb_backend/crates/bb_core/src/task_graph/run_state/model.rs:89`
- `bb_backend/crates/bb_core/src/task_graph/run_state/model.rs:100`
- `bb_backend/crates/bb_core/src/task_graph/run_state/model.rs:128`
- `bb_backend/crates/bb_core/src/task_graph/run_state/model.rs:253`
- `bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs:18`
- `bb_backend/crates/bb_core/src/task_graph/run_state/superstep.rs:219`

### 16. run 创建时初始化 Pregel checkpoint

`create_run` 现在会编译 graph，并根据 graph input 初始化 Pregel checkpoint。

证据：

- `bb_backend/crates/bb_core/src/task_graph/run_state/lifecycle.rs:48`
- `bb_backend/crates/bb_core/src/task_graph/run_state/lifecycle.rs:80`

## 已实现：节点运行时

### 1. 节点执行模块改为 `nodes/`

当前 dispatch 支持：

- `Start`
- `End`
- `Branch`
- `Loop`
- `InputVar`
- `HumanGate`
- `Llm`
- `Shell`
- `SubGraph`

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/mod.rs:35`

### 2. 新增 Shell 节点类型

当前分支新增 `NodeType::Shell` 和 `ShellConfig`。

`ShellConfig` 包含：

- `cwd`
- `command`
- `args`
- `env`
- `timeout_ms`
- `permission`
- `expected_exit_codes`
- `capture`

证据：

- `bb_backend/crates/bb_core/src/task_graph/definition/types.rs:205`
- `bb_backend/crates/bb_core/src/task_graph/definition/types.rs:273`
- `bb_backend/crates/bb_core/src/task_graph/definition/types.rs:293`

### 3. 已实现 Shell executor

Shell 节点执行支持：

- 相对 cwd 校验
- cwd 逃逸阻止
- 权限粗拦截
- process spawn
- stdout/stderr streaming log
- stdout/stderr 有界捕获
- 超时处理
- run cancellation 处理
- expected exit codes
- JSON artifact 输出

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:86`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:386`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:420`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:536`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:663`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/shell_node.rs:692`

### 4. LLM 节点仍支持 runtime command 和 AgentSession

LLM 节点目前支持：

- 通过 resolved invocation 渲染 prompt
- dry run
- OpenCode-style command execution path
- `opencode` / `codex` / `codebuddy` 的 AgentSession path
- artifact 写入
- cancellation 检测

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:30`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:46`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:137`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:315`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:559`

### 5. AgentSession-backed node execution 已存在

对于 AgentSession runtime，节点执行会创建一个 agent session，parent 为：

```text
TaskGraphNode { run_id, node_id }
```

然后执行一轮 agent turn，并把结果转成 `NodeOutcome`。

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:331`
- `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs:368`

### 6. SubGraph 节点传播 checkpoint namespace

SubGraph 执行会创建 child run，并根据 parent namespace 和当前 node id 派生 child checkpoint namespace。

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/subgraph.rs:21`
- `bb_backend/crates/bb_core/src/task_graph/nodes/subgraph.rs:108`
- `bb_backend/crates/bb_core/src/task_graph/nodes/subgraph.rs:110`
- `bb_backend/crates/bb_core/src/task_graph/nodes/subgraph.rs:179`

### 7. 新增业务节点 registry

当前分支新增 business role registry，包括：

- `explorer_agent`
- `implementer_agent`
- `verifier_agent`
- `reviewer_agent`
- `handoff_writer`
- `opencode_session`
- `codex_session`
- `claude_session`
- `local_shell`
- `write_wiki_doc`
- `update_ticket`
- `feishu_notify`

重要事实：这些是 registry specs 和 role metadata，不是新的独立 executor。它们仍然叠在已有可执行节点类型之上，例如 `llm`、`shell`、`human_gate`、`sub_graph`。

证据：

- `bb_backend/crates/bb_core/src/task_graph/nodes/registry.rs:3`
- `bb_backend/crates/bb_core/src/task_graph/nodes/registry.rs:24`
- `bb_backend/crates/bb_core/src/task_graph/nodes/registry.rs:45`
- `bb_backend/crates/bb_core/src/task_graph/nodes/registry.rs:105`

## 明确未实现

### 1. 未实现运行时拓扑变更

没有找到这些实现：

- `GraphMutation`
- 运行中新增 node
- 运行中删除 node
- 运行中新增 edge
- 运行中删除 edge
- topology mutation request queue
- topology mutation barrier apply
- topology mutation conflict handler

检索命令：

```powershell
rg "GraphMutation|Topology|topology|add_node|remove_node|add_edge|remove_edge|mutation|mutations" bb_backend/crates/bb_core/src/task_graph
```

结果只命中了普通 state mutation 注释，没有命中 graph topology mutation 实现。

### 2. Coordinator 不能修改当前 graph 拓扑

当前 coordinator 可以：

- 编译 graph snapshot
- 准备 tasks
- 执行 nodes
- 收集 writes
- 提交 channel checkpoint
- 更新 run/node projection

当前 coordinator 不能：

- 运行中创建 graph node
- 运行中删除 graph node
- 运行中新增 graph edge
- 运行中删除 graph edge
- 运行中创建新的 compiled graph revision

### 3. `Send` 是动态任务派发，不是拓扑变更

`Send` 可以向已有 compiled process 派发 PUSH task。

`Send` 不能创建新的 process 或 node。

证据：

- `prepare_push_tasks` 会忽略不在 `compiled.processes` 里的 send target。
- `apply_writes` 会拒绝写入未知的非 runtime channel。

源码位置：

- `bb_backend/crates/bb_core/src/task_graph/pregel/prepare.rs:165`
- `bb_backend/crates/bb_core/src/task_graph/pregel/writes.rs:49`

### 4. 未实现 Arena / plan-review 工作流

后端核心没有发现这些实现：

- arena/court planning protocol
- forced dissent voting
- multi-agent spec adjudication
- plan gate
- human gate tied to topology mutation conflicts

当前 `HumanGate` 是审批暂停节点，不是 plan-review arena。

## 验证

执行过的验证命令：

```powershell
cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml
```

结果：

```text
159 passed
0 failed
```

这个结果验证了当前 working tree 中 TaskGraph 后端库相关测试。

## 当前事实一句话

当前分支确实落地了一个有意义的后端执行内核重构：

```text
cursor interpreter
  -> compiled graph IR
  -> Pregel checkpoint/channel/task model
  -> superstep barrier write application
  -> durable checkpoint/event projection
```

也落地或稳定了这些节点运行时：

```text
LLM / AgentSession / Shell / SubGraph
```

但是它没有实现 Pregel 3.4 的运行时拓扑变更：

```text
runtime graph add/remove node/edge
```
