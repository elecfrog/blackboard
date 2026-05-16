# Topology Mutation 与长程 Agent Workflow 实践手册

## 状态

本文是 `pregel-topology-mutation` 实施后的实践文档。

它不是后端实现 spec 的替代品。后端 Rust 语义仍以 `implementation_spec.md` 和当前代码为准。本文解决的是另一个问题：

```text
有了 topology mutation 以后，怎么把它正确用于长程 LLM / Agent workflow。
```

核心案例是 `kb-wiki-build-workflow`。

## 一句话结论

Topology mutation 不是为了让 LLM 直接“自由改图”。

它的正确使用方式是：

```text
LLM 负责提出结构化 plan
通用强门禁负责校验 plan
LLM repair 只负责按错误修 plan
确定性 mutation 节点负责把合格 plan 编译成 graph mutation
Pregel barrier 负责把 mutation 原子应用成新 graph revision
```

也就是说，我们不是在提升 LLM 投骰子的准确率，而是在阻断错误骰子继续污染下游。

## Why

### 1. 静态 graph 不适合长程调研和写作任务

`kb-wiki-build-workflow` 的目标不是跑一个固定脚本，而是从一段任务意图里完成：

1. 识别源码和输出目录。
2. 扫描已有 truth。
3. 规划 scout 工作。
4. 动态生成多个 scout 节点。
5. 合并 scout 输出。
6. 规划 wiki 页面。
7. 规划 writer 工作包。
8. 动态生成多个 writer 节点。
9. review / repair。
10. 生成最终报告。

这里有两个天然动态点：

- scout 数量取决于模块结构和调研目标。
- writer 数量取决于 wiki plan 和页面切分。

如果提前在静态 graph 里塞固定数量的 scout / writer，会出现两个问题：

- 少了不够用，多了浪费。
- graph 结构表达的是“猜测”，不是运行时事实。

Topology mutation 的意义是允许 workflow 在运行中根据已产生的 plan 扩展后续执行图。

### 2. LLM 输出不能直接当施工图

这次 `kb-wiki-build-workflow` 的失败不是模型不会写代码，而是 plan 输出在多个阶段可能跑偏：

- scout plan 可能缺少 `scouts`。
- wiki plan 可能缺少页面契约。
- writer plan 可能把页面写错目标。
- review 输出可能不是可判定的 `{ needs_repair, failures }` 结构。
- LLM 可能把之前测试 prompt 的残留内容带入新任务。

如果这些输出直接进入下游，后面的节点会认真执行错误计划。

所以 topology mutation 必须和强校验一起使用。否则 mutation 只是把错误扩散得更快。

### 3. 人类 review 速度追不上 Agent coding 速度

这个系统的目标不是让人类逐行检查每个 agent plan。

人类应该负责：

- blueprint
- direction
- vision
- goal alignment

系统应该负责：

- 结构化输出契约
- 校验
- 修复
- 阻断
- 可观测事实源

因此长程 workflow 的关键不是“多提醒用户 review”，而是把机器侧 gate 建好。

## What

### 1. Topology mutation 的底层对象

当前后端支持的 mutation request 是：

```rust
pub struct GraphMutationRequest {
    pub id: String,
    pub source_task_id: String,
    pub source_node_id: String,
    pub op: GraphMutationOp,
    pub reason: Option<String>,
}
```

支持的操作：

```rust
pub enum GraphMutationOp {
    AddNode { node: TaskGraphNode },
    RemoveNode { node_id: String },
    AddEdge { edge: TaskGraphEdge },
    RemoveEdge { edge_id: String },
    PatchNodeConfig { node_id: String, patch: Value },
}
```

这些 request 可以来自：

- `NodeOutcome.graph_mutations`
- 节点 `output.graph_mutations`
- 目前封装过的 `llm_mutation` 节点

### 2. Mutation 的生效时机

Mutation 不在节点执行中立即生效。

Pregel 语义下，一个 superstep 内 graph topology 固定。节点只能产生 mutation request。coordinator 在 barrier 阶段统一处理：

1. 收集本 superstep 的所有 mutation request。
2. 先应用普通 writes。
3. 检测 mutation conflict。
4. 在 candidate graph 上按固定顺序应用 mutation。
5. validate candidate graph。
6. compile candidate graph。
7. 写入 mutation batch。
8. 写入新的 graph revision。
9. 下一 superstep 使用新 revision 调度。

当前应用顺序是：

```text
remove edge
remove node
add node
patch node config
add edge
```

这个顺序的目的不是给 LLM 提供自由度，而是让同一批 mutation 的应用结果确定。

### 3. Graph revision 是事实

每次 mutation 成功应用都会产生新的 graph revision。

运行态观察不要再使用 cursor 思维。正确的观察对象是：

- `current_graph_revision`
- `active_nodes`
- `mutation_batches`
- graph revision snapshot
- node output artifact
- `.staging` 事实源

旧的 cursor / next cursor 思维会误导你，以为图还是一条线性解释器管线。现在的事实是：

```text
compiled graph revision + Pregel checkpoint + channel versions
```

### 4. Mutation 节点不是业务节点

在 `kb-wiki-build-workflow` 里，`llm_mutation` 虽然服务于 KB workflow，但它承担的是一个更通用的角色：

```text
把合格 plan 编译为 AddNode / AddEdge request
```

LLM 不应该直接决定最终 graph object。更稳的分层是：

```text
planner LLM -> JSON plan
schema_validate -> contract gate
repair LLM -> contract repair
final schema_validate -> hard gate
llm_mutation -> deterministic graph compiler
Pregel barrier -> graph revision
```

后续如果继续泛化，`llm_mutation` 也应该向更通用的 `plan_to_mutation` / `mutation_compile` 方向演进，而不是变成越来越多的 KB 专用节点。

## How

### 推荐流程模板

长程 Agent workflow 推荐使用这个结构：

```text
start
  -> intake / validate inputs
  -> scan existing truth
  -> planner
  -> validate plan
  -> route valid/invalid
  -> repair invalid plan
  -> final plan gate
  -> mutation compile
  -> dynamic workers
  -> collect / merge worker outputs
  -> next planner
  -> validate / repair / final gate
  -> mutation compile
  -> dynamic workers
  -> review / repair loop
  -> final report
  -> end
```

对应到 `kb-wiki-build-workflow`：

```text
start
  -> intake-validate
  -> scan-existing-truth
  -> scout-planner
  -> scout-plan-validate
  -> scout-plan-route
  -> scout-plan-repair
  -> scout-plan-final
  -> scout-mutation
  -> dynamic scout nodes
  -> scout-output-validate
  -> scout-output-repair
  -> scout-output-final
  -> merge-scans
  -> wiki-planner
  -> wiki-plan-validate
  -> wiki-plan-repair
  -> wiki-plan-final
  -> writer-planner
  -> writer-plan-validate
  -> writer-plan-repair
  -> writer-plan-final
  -> writer-mutation
  -> dynamic writer nodes
  -> review-repair
  -> review-output-validate
  -> review-output-repair
  -> review-output-final
  -> qmd-final-report
  -> end-success
```

### Gate 标准形态

一个稳定 gate 应该由四段组成：

```text
producer
  -> schema_validate(fail_on_invalid=false)
  -> branch(valid / invalid)
  -> llm repair
  -> schema_validate(fail_on_invalid=true)
  -> consumer
```

含义：

- 第一次 validate 不直接 fail，是为了给 repair 一次机会。
- branch 根据 `valid` 决定是否进入 repair。
- repair 只处理结构化错误，不重新发明任务。
- final gate 必须强失败，否则下游会被污染。

这套复杂度是有意引入的。它换来的是错误不会越过边界。

### Schema validate 应该保持通用

不要做 `kb_validate` 这种只服务一次 workflow 的专用节点。

正确方式是：

```text
schema_validate + JSON Schema subset + workflow config
```

原因：

- schema 校验本质是强门禁，不是 KB 业务逻辑。
- 同一套能力可以用于 scout plan、writer plan、review output、其他 workflow。
- 专用节点会把临时业务规则固化进 runtime。
- 通用 gate 更容易组合、测试和复用。

当前 `schema_validate` 的输出固定为：

```json
{
  "valid": true,
  "schema_name": "writer_plan",
  "errors": [],
  "data": {},
  "artifact_path": null,
  "artifact_type": "json"
}
```

实践规则：

- 下游读取 `output.data`。
- 需要落盘时读取 `output.artifact_path`。
- 非最终 gate 使用 `fail_on_invalid=false`。
- 最终 gate 使用 `fail_on_invalid=true`。
- repair 节点输入必须包含 `validation.errors` 和 `original`。

### Data pin 传文件路径，不传大上下文

`kb-wiki-build-workflow` 的关键收敛点是：

```text
.staging 是跨节点事实源
data pin 传 artifact_path 或小 JSON
LLM 节点自己读取 .staging 文件
```

不要把所有 scout 输出、wiki plan、writer plan 都塞进 prompt。

正确做法：

- scout 输出由 `merge-scans` 汇总到 `.staging/scout-outputs.json` 和 `.staging/scans/*`。
- `wiki-planner` 读取 manifest path。
- `writer-planner` 输出 `writer-plan.json`。
- `writer-mutation` 读取 `writer-plan-final.output.artifact_path` 或 `output.data`。
- 动态 writer 节点读取 `manifest_path`、`wiki_plan_path`、`writer_plan_path`。

这能避免两个问题：

- 上下文窗口压力集中到错误节点。
- LLM 靠聊天历史猜上游结果。

### Planner 输出是 artifact，不是聊天回复

Plan 节点的结果必须被当作 artifact：

```json
{
  "data": {},
  "artifact_path": "F:/.../.staging/wiki-plan.json",
  "artifact_type": "json",
  "schema_name": "wiki_plan",
  "summary": "..."
}
```

下游不要依赖这种硬引用：

```text
{{nodes.writer-planner.output}}
```

优先使用：

```text
{{data.writer-planner.output.artifact_path}}
{{nodes.writer-plan-final.output.artifact_path}}
{{nodes.writer-plan-final.output.data}}
```

尤其是 mutation 节点，应该优先支持 plan filepath。因为 mutation 编译需要的是稳定 artifact，不是 prompt 上下文里的某段文本。

### Dynamic worker 的输入要显式包含事实源

动态生成的 scout / writer 节点不能只拿到自己的 `goal`。

它至少需要：

#### Scout worker

```text
intake
truth
module_root
kb_output_dir
staging_dir
scope
goal
```

#### Writer worker

```text
manifest
wiki_plan
writer_plan
kb_output_dir
staging_dir
manifest_path
wiki_plan_path
writer_plan_path
language
page_ids
target_paths
goal
```

重点是 writer 必须读取 `.staging` 事实源，而不是凭 planner prompt 的记忆写作。

### Review / repair loop 要输出可判定 JSON

Review 节点不能只输出自然语言评价。

推荐输出：

```json
{
  "review_status": "PASS",
  "needs_repair": false,
  "failures": [],
  "warnings": [],
  "changed_files": [],
  "repair_count": 0
}
```

Loop condition 只看一个明确字段：

```text
$.needs_repair == true
```

否则 loop 会变成靠模型语气判断是否继续，稳定性很差。

### 观测方式

一次成功的 mutation workflow 至少应该能看到：

- run 的 `current_graph_revision` 从 0 增加。
- mutation 节点日志出现 `generated N topology mutation request(s)`。
- run 目录下有 `mutation_batches/*`。
- 动态节点出现在 run node 列表。
- 前端 mutation timeline 能看到 revision 变化。
- `.staging` 里保留 planner / merge / review artifacts。
- final report 成功前，所有 final gate 都通过。

对 `kb-wiki-build-workflow`，重点看：

```text
.staging/scout-plan.json
.staging/scout-outputs.json
.staging/wiki-manifest.json
.staging/wiki-plan.json
.staging/writer-plan.json
.staging/review-repair-output.json
```

如果最终文件跑偏，优先检查：

1. scout 输出是否已经跑偏。
2. manifest 是否正确合并。
3. wiki plan 是否把目标模块写错。
4. writer plan 是否把 target path 写错。
5. writer 是否读取了 `.staging`，还是只靠 prompt 猜。

## 踩坑记录

### 坑 1：把 mutation 理解成 LLM 自由改图

错误做法：

```text
让 LLM 直接输出 graph nodes / edges
然后 runtime 尽量兼容
```

问题：

- LLM 会漏字段。
- LLM 会创造不存在的 node type。
- LLM 会把 graph 结构和业务 plan 混在一起。
- 下游失败时很难判断是 plan 错、schema 错还是 graph 编译错。

最佳实践：

```text
LLM 输出业务 plan
schema_validate 校验业务 plan
mutation compiler 把业务 plan 转成 graph mutation
```

### 坑 2：专门做 `kb_validate`

错误做法：

```text
为了这条 KB workflow 做一个专用 kb_validate 节点
```

问题：

- 业务规则进入 runtime。
- 下一个 workflow 还要再写一个 validate。
- 节点系统会变成一次性节点垃圾场。

最佳实践：

```text
只加通用 schema_validate
具体 schema 写在 workflow config
```

### 坑 3：把 validation 放进 LLM 节点内部

错误做法：

```text
LLM 节点自己判断自己输出对不对
```

问题：

- 同一个 LLM 既是被审对象又是审查者。
- 错误输出可能被包装成“看起来合理”的自然语言。
- 下游没有统一 gate。

最佳实践：

```text
LLM 输出后，外部 schema_validate 独立校验
```

### 坑 4：第一次 invalid 直接 fail

错误做法：

```text
plan invalid -> run failed
```

问题：

- LLM 很容易犯格式错误。
- 很多错误可以通过明确 schema errors 修复。
- 长程任务会因为小格式问题频繁中断。

最佳实践：

```text
validate(fail_on_invalid=false)
  -> repair
  -> final validate(fail_on_invalid=true)
```

但注意：最终 gate 不能放水。否则 repair 没意义。

### 坑 5：没有 artifact 回退机制时盲目重试

当前没有完整 artifact rollback。

所以失败策略必须保守：

- 中间 gate 可以 repair 一次或有限次数。
- final gate 不过就 fail。
- 不要在同一输出路径上无限重试写文件。
- review repair loop 必须有迭代上限。

否则系统可能在错误文件上反复覆盖，最后失去可诊断现场。

### 坑 6：依赖聊天上下文传递计划

错误做法：

```text
上游 plan 在 prompt 里说过
下游 LLM 理应记得
```

问题：

- session 不一定共享。
- 共享 session 也会有 context window 压力。
- 下游可能继承错误上下文。

最佳实践：

```text
plan 落盘到 .staging
data pin 传 artifact_path
下游显式读取 artifact
```

Session 合并可以作为 token 优化，但不能替代 artifact flow。

### 坑 7：branch 的两路汇合被当成 join

这次实际踩到一个后端 bug：

```text
valid path -> final gate
invalid path -> repair -> final gate
```

编译器一开始把 `final gate` 当成多入边 join，要求 valid path 和 repair path 都触发，导致 branch 后卡死或失败。

正确语义是：

```text
branch merge 是 OR trigger
普通多上游依赖才是 join barrier
```

最佳实践：

- branch 的不同规则路径汇合时，下游应该被任一路径触发。
- 真正需要等待多个动态 worker 时，才使用 join/barrier 语义。
- workflow 编译器必须区分 branch source 和普通 source。

### 坑 8：动态节点连接到 collector 时，静态校验误判缺边

动态 scout 节点是在运行中 add node / add edge。

如果静态 graph 里 collector 的 `exec_in` 被标成必须存在静态入边，预运行校验会误判。

最佳实践：

- 动态 fanout 的 collector 要允许运行时入边。
- 对这类节点，`exec_in.required` 不能简单等同于“静态 graph 必须已有上游”。
- 更长期的做法是给节点声明 `accepts_dynamic_exec_in` 之类的能力，而不是靠手工把 required 调松。

### 坑 9：merge 逻辑硬编码旧 scout 节点

早期 `merge-scans` 依赖固定的 `scout-structure`、`scout-interfaces`、`scout-internals` 等节点。

引入 mutation 后，scout 节点是动态生成的，固定节点名不再可靠。

最佳实践：

```text
merge 节点优先读取 data pin 输入的 scout_outputs
必要时再按 prefix 收集 node outputs
```

也就是说，collector 应该面向“动态输出集合”，不是面向“固定节点清单”。

### 坑 10：模型切换被当成架构修复

DeepSeek、MiniMax、GPT 都可能输出错误结构。

模型切换可以提高一次通过率，但不能解决：

- 输出契约缺失
- 数据流不显式
- artifact 不落盘
- final gate 放水
- mutation 直接吃未校验 plan

最佳实践：

```text
模型是可替换执行资源
gate 是 workflow 架构
```

不要用更强模型掩盖缺失的工程门禁。

## kb-wiki-build-workflow 最佳实践

### 1. Intent resolver 应该在 graph 之前

用户真实操作方式应该是：

```text
chatting prompt
  -> intent resolver
  -> graph inputs
  -> start graph
```

不要把 intent 节点塞进每一条业务 graph。

原因：

- intent 是 run launcher / UI / API pre-start 层能力。
- 每条 graph 仍应保留明确 inputs。
- 这样 graph 本体不会被自然语言输入污染。

当前 KB workflow 应收敛为稳定 inputs，例如：

```text
module-name
module-root
kb-output-dir
language
request
options
```

### 2. Planner 必须拆成两类

不要让一个 planner 同时负责所有事情。

推荐分层：

```text
scout-planner: 决定要调研什么
wiki-planner: 决定文档页面结构和页面契约
writer-planner: 决定 writer fanout 和写作包
```

每个 planner 的输出都必须：

- JSON only
- schema validate
- artifact 落盘
- 下游通过 data pin 或 artifact path 读取

### 3. Mutation 只发生在 fanout 边界

不需要每一步都 mutation。

`kb-wiki-build-workflow` 里最合理的两个 mutation 点是：

```text
scout-plan-final -> scout-mutation -> dynamic scout nodes
writer-plan-final -> writer-mutation -> dynamic writer nodes
```

其他步骤用普通 exec / data edge 足够。

### 4. Dynamic worker 不负责决定流程

动态 scout / writer 是 worker，不是 coordinator。

它们应该：

- 执行分配给自己的 scope。
- 输出结构化 artifact。
- 不新增下游流程。
- 不决定是否结束 run。
- 不直接改 graph。

流程控制由 planner、validator、mutation compiler、loop condition 负责。

### 5. `.staging` 是一等事实源

`.staging` 不应该被视为临时垃圾目录。

它在长程 workflow 里承担：

- 中间事实源
- retry / repair 输入
- debug 现场
- 下游 worker 可读取上下文
- final report 的证据来源

因此 `.staging` 应该保留，至少在 run 结束前不能删。

### 6. Review 输出必须和 loop 解耦

Review 可以写得复杂，但 loop condition 必须简单。

推荐：

```text
review-repair -> review-output-validate -> review-output-final
repair-loop.condition = $.nodes.review-output-final.output.data.needs_repair == true
```

不要让 loop condition 直接读取自然语言 review。

### 7. 强门禁比 prompt 约束更重要

Prompt 可以写：

```text
只输出 JSON，不要 Markdown
```

但这只能提高概率。

真正的稳定性来自：

```text
schema_validate
branch
repair
final schema_validate
```

只靠 prompt 等于继续相信骰子。

### 8. 失败要停在正确边界

一个好的 workflow 失败时，应该能告诉你：

- 哪个 artifact 不满足哪个 schema。
- 哪个动态节点输出不合格。
- mutation batch 是 applied 还是 rejected。
- 当前 graph revision 是多少。
- `.staging` 哪个文件是最后可信事实源。

如果失败只表现为“后面写错文档了”，说明 gate 太晚。

## 设计原则

### 原则 1：Runtime 通用，业务放 config

新增 runtime node 前先问：

```text
这个节点是否能被另一个 workflow 复用？
```

如果答案是否，优先用：

- schema config
- prompt template
- data pin
- shell/tool validation
- generic transform

而不是新增业务节点。

### 原则 2：Plan 是 untrusted artifact

Plan 不是事实。

Plan 只有经过：

```text
schema validate
repair
final validate
```

才可以进入 mutation compiler。

### 原则 3：Mutation 是 compile，不是 reasoning

Mutation 阶段不应该继续做大段推理。

它应该像编译器：

```text
输入：已校验 plan
输出：确定性 GraphMutationRequest 列表
失败：明确错误码和错误位置
```

### 原则 4：下游消费 final output，不消费 raw output

不要让下游读：

```text
planner.output
repair.output
validate.output.data
```

除非这个节点就是 gate 本身。

普通下游应读：

```text
*-final.output.data
*-final.output.artifact_path
```

### 原则 5：先阻断污染，再优化体验

Graph 变复杂是可以接受的，因为这是稳定性成本。

后续要优化的是 authoring / UI 表达，比如折叠成：

```text
Plan Contract Gate
Validate Repair Gate
Fanout Mutation Block
Review Repair Loop
```

但底层不要为了 UI 简洁牺牲 gate。

## 未来演进建议

### 1. Gate block 作为 UI 组合块

底层仍保留通用节点：

- `plan`
- `schema_validate`
- `branch`
- `llm`
- `llm_mutation`

UI 层可以提供组合块：

```text
validate-repair-final
plan-with-contract
fanout-mutation
review-repair-loop
```

这能降低 graph 视觉复杂度，但不牺牲 runtime 可组合性。

### 2. 更通用的 mutation compiler

当前 `llm_mutation` 已经开始承担 plan 到 mutation 的编译职责，但还带有 `scout_fanout` / `writer_fanout` 模式。

后续应该演进为：

```text
mutation_compile
```

输入：

```json
{
  "mode": "fanout",
  "items": [],
  "node_template": {},
  "edge_template": {},
  "join_target": "merge-scans"
}
```

这样可以继续减少 KB 特定逻辑。

### 3. Artifact rollback

当前 repair 是保守的，因为没有完整 artifact rollback。

后续应支持：

- 每次写文件前生成 revision。
- repair 写入新 revision。
- review fail 可以回滚到上一个 artifact revision。
- final report 只引用通过 gate 的 artifact revision。

有了 rollback 后，retry 策略可以更激进。

### 4. Session 合并作为优化，不作为正确性前提

共享 session 可以：

- 节省 token。
- 保持 planner 上下文连续。
- 减少重复解释。

但它不能替代：

- `.staging`
- schema gate
- artifact path
- final validate

正确性必须建立在 artifact flow 上，session 只能是性能和体验优化。

### 5. Dynamic collector 能力声明

目前为了支持动态 fanout，部分 collector 的静态入边要求需要放松。

后续应显式建模：

```json
{
  "accepts_dynamic_exec_in": true,
  "dynamic_source_prefix": "scout-"
}
```

这样 pre-run validation 可以理解“这个节点的上游会在 mutation 后出现”。

## 最小检查清单

设计新的 mutation workflow 前，至少检查：

1. mutation 是否只发生在 fanout / topology 真正变化处。
2. planner 是否输出 JSON artifact。
3. planner 输出是否有 schema。
4. schema 是否在 LLM 外部校验。
5. invalid 是否进入 repair。
6. final gate 是否强失败。
7. mutation 是否只吃 final gate 输出。
8. dynamic worker 是否显式拿到 `.staging` 路径。
9. collector 是否能收集动态节点输出。
10. loop condition 是否读取可判定 JSON 字段。
11. run 是否能观测 graph revision 和 mutation batch。
12. 失败时是否能定位到具体 artifact 和 schema error。

如果其中任意一条不满足，先补 workflow 架构，不要靠换模型解决。
