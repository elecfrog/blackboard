# TaskGraph 模块地图

这个目录是 Blackboard 的 TaskGraph 引擎：负责图定义、图存储、静态校验、图编译、持久化运行态、Pregel/superstep 执行内核、节点执行、运行时接入和定时触发。

主链路：

```text
definition
    |
    v
compile::compiler -> pregel::{runner,coordinator,PregelLoop} -> pregel::executor -> nodes
        |                    |                    |
        |                    v                    v
        |                run_state <------ events/checkpoints/artifacts
        |
        v
API / UI projection
```

## 主执行流程

1. `definition/types.rs` 定义原始 TaskGraph JSON 模型。
2. `validation/` 作为 graph 语言诊断层，校验 JSON/schema、图结构、节点配置和运行前约束。
3. `compile/compiler.rs` 把原始图降低成 Pregel 风格可执行 IR：processes、channels、triggers、writers、branch channels、join barriers、state channels。
4. `run_state/` 创建和维护 durable run：graph snapshot、node state、checkpoint、event、artifact、pending Pregel writes。
5. `pregel/` 从 checkpoint channel versions 推导 runnable tasks，收集 task-local writes，在 superstep barrier 统一 apply writes，并产出下一份 checkpoint。
6. `pregel/coordinator.rs` 是编排适配层：加载 durable state，驱动 `PregelLoop`，dispatch task，持久化 checkpoint，写 UI/event projection。
7. `pregel/executor.rs` 执行 ready node，并返回 `NodeOutcome`。
8. `nodes/` 放具体节点行为、节点 registry、LLM/Agent 配置解析和图导航。

## 顶层模块

| 模块 | 职责 |
| --- | --- |
| `definition/` | 原始图定义层：schema、store、upgrade、pins。 |
| `validation/` | 诊断层：JSON/schema decode、图结构、配置、环、pin、pre-run runtime 约束。 |
| `compile/` | 编译层：raw graph -> executable IR，以及早期 channel helper。 |
| `pregel/` | Graph execution engine：Pregel kernel、run/resume entrypoint、coordinator、executor、outcome。 |
| `nodes/` | 节点行为：控制节点、runtime 节点、subgraph、LLM config、node registry、表达式求值、导航。 |
| `run_state/` | durable run state：生命周期、模型、node IO、context、fork/join、superstep checkpoint、event log、artifact、pending writes。 |
| `runtime/` | runtime 进程执行和输出捕获，包括 OpenCode event parsing。 |
| `schedules/` | TaskGraph 定时任务：schedule model、due time、claim、trigger、CRUD。 |
| `tests/` | store/run_state/graph fixture 单元测试，以及 `tests/runner/` 下的 runner/coordinator 端到端测试。 |

## definition 子模块

| 模块 | 职责 |
| --- | --- |
| `definition/types.rs` | 原始 TaskGraph schema：node、edge、config、metadata、input、错误类型。 |
| `definition/store.rs` | system/project graph 的 CRUD 与图文件存储。 |
| `definition/upgrade.rs` | 旧图定义升级，保证保存过的 graph 进入 compile/run 前能补齐新字段。 |
| `definition/pins.rs` | 节点默认 UI/data pins。 |

## validation 子模块

| 模块 | 职责 |
| --- | --- |
| `validation/decode.rs` | JSON/source/value decode 诊断，把 serde/schema 错误统一成 `TaskGraphValidationError`。 |
| `validation/mod.rs` | `validate_graph` 静态语义入口。 |
| `validation/config.rs` | 节点 config 和 graph input 诊断。 |
| `validation/pins.rs` | pin 连接、方向和类别诊断。 |
| `validation/cycles.rs` | 非法环检测。 |
| `validation/pre_run.rs` | 创建 run 前的 runtime 依赖诊断。 |

## compile 子模块

| 模块 | 职责 |
| --- | --- |
| `compile/compiler.rs` | Graph/StateGraph lowering，生成 process、channel、trigger、writer。 |
| `compile/channels.rs` | 较早的数据流 channel state helper；高层 channel spec 和部分测试仍使用。Pregel channel 语义在 `pregel/writes.rs`。 |

## Pregel 子模块

`pregel/` 是 060 的核心 graph engine。后续抄 LangGraph 的底层能力，优先落在这里；具体算法落到 kernel 文件，编排 glue 才放 `pregel/coordinator.rs`。

| 模块 | 职责 |
| --- | --- |
| `pregel/runner.rs` | 对外 run/resume 入口，负责调用 coordinator 并处理 resume/fast-fail。 |
| `pregel/coordinator.rs` | 单写者编排器：驱动 `PregelLoop`、dispatch task、reduce outcome、持久化 checkpoint/event projection。 |
| `pregel/executor.rs` | 把 `ReadyNode` 交给 node execution，得到 `NodeOutcome`。 |
| `pregel/outcome.rs` | 执行层结果类型：ready node、node outcome、side effect、reduce action、superstep plan。 |
| `pregel/model.rs` | `PregelCheckpoint`、`PregelTask`、`PregelWrite`、prepared step、send packet、checkpoint tuple 数据结构。 |
| `pregel/checkpoint.rs` | initial checkpoint、checkpoint config/metadata/tuple helper、channel version diff。 |
| `pregel/prepare.rs` | 对齐 `_prepareNextTasks`：PULL/PUSH task 生成、pending writes replay、deterministic task id/path。 |
| `pregel/writes.rs` | 对齐 `_applyWrites`：barrier apply、channel availability、channel consume、reducer、version bump。 |
| `pregel/command.rs` | node output -> Pregel writes：state update、`Command.goto`、`Send`、branch writes、task packet。 |
| `pregel/interrupt.rs` | `interruptBefore` / `interruptAfter` 判断，以及 `__interrupt__` / `__resume__` writes。 |
| `pregel/loop_state.rs` | 长生命周期 `PregelLoop`：active checkpoint、pending writes、loop status、prepare/put/commit。 |
| `pregel/namespace.rs` | checkpoint namespace helper：parent/child/task namespace。 |
| `pregel/runtime_channels.rs` | Pregel reserved runtime channel 常量。 |
| `pregel/tests.rs` | Pregel kernel 单元测试。 |

## run_state 子模块

| 模块 | 职责 |
| --- | --- |
| `run_state/model.rs` | durable run、node、checkpoint、event、artifact、pause、summary 数据结构。 |
| `run_state/lifecycle.rs` | create/read/list/update run 生命周期操作。 |
| `run_state/superstep.rs` | superstep checkpoint 持久化、checkpoint tuple recovery、pending Pregel writes IO。 |
| `run_state/node_io.rs` | node log、node output、artifact。 |
| `run_state/context.rs` | branch decision、loop iteration、run context mutation。 |
| `run_state/fork_join.rs` | 兼容路径里的 completed branch tracking。 |
| `run_state/mod.rs` | run_state 对外 API surface。 |

## nodes 子模块

| 模块 | 职责 |
| --- | --- |
| `nodes/mod.rs` | node dispatch 入口。 |
| `nodes/navigation.rs` | next-node selection helper。 |
| `nodes/control/` | Start/End、Branch、Loop、HumanGate 等控制节点。 |
| `nodes/runtime/` | LLM/Agent 与 Shell runtime 节点。 |
| `nodes/subgraph.rs` | 子图执行和 child checkpoint namespace 传播。 |
| `nodes/eval.rs` | Branch/Loop 表达式求值和 prompt template rendering。 |
| `nodes/llm.rs` | Codex、CodeBuddy、OpenCode 的 LLM/Agent runtime 配置生成。 |
| `nodes/registry.rs` | 内置节点目录：role、permission、runtime binding、UI/runtime spec。 |

## tests 子模块

| 模块 | 职责 |
| --- | --- |
| `tests/mod.rs` | 测试入口，挂载 store/run_state 测试和 runner 测试子模块。 |
| `tests/runner/eval.rs` | Branch/Loop 表达式测试。 |
| `tests/runner/runs.rs` | runner 主流程、superstep、recovery、interrupt、prompt rendering 测试。 |
| `tests/runner/runtime.rs` | Shell、Codex、CodeBuddy runtime node 测试。 |
| `tests/runner/fork_join.rs` | fork/join 兼容路径测试。 |
| `tests/runner/fixtures.rs` | runner 测试共享 fixture 和 graph builder。 |

## 新能力应该放哪里

- 新 graph schema 字段：先改 `definition/types.rs`，再改 `validation/`，必要时补 `definition/upgrade.rs`。
- 新 compile-time 图语义：放 `compile/compiler.rs`。
- 新 LangGraph/Pregel 算法对齐：放 `pregel/prepare.rs`、`pregel/writes.rs`、`pregel/checkpoint.rs`、`pregel/command.rs`、`pregel/interrupt.rs` 或 `pregel/loop_state.rs`。
- 新 durable run 文件或 recovery 语义：放 `run_state/`。
- 新节点行为：放 `nodes/`，节点目录/权限/runtime metadata 放 `nodes/registry.rs`。
- 新 runtime connector：放 `runtime/` 和 `nodes/llm.rs`。
- 新 schedule 语义：放 `schedules/`。
- 只有编排 glue 才放 `pregel/coordinator.rs`。

如果一个改动同时需要 durable IO 和 Pregel 算法，不要直接把逻辑写进 `pregel/coordinator.rs`；优先增加一个小 adapter 边界。

## 验证命令

- Pregel kernel：`cargo test -p bb_core task_graph::pregel --lib --manifest-path bb_backend/Cargo.toml`
- TaskGraph core：`cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml`
- HTTP/MCP/daemon 集成：`cargo test -p bb_daemon --manifest-path bb_backend/Cargo.toml`
- 如果改了 `bb_web/src`：还要跑 `npm run build --prefix bb_web`
