# CodeBuddy-Kimi-2.6 Review: Pregel Topology Mutation 后端内核草案

## 总评

spec_draft.md 是一份目标清晰、边界明确、架构方向正确的技术草案。它准确识别了当前 `feature/orni` 分支的核心缺口——Pregel 3.4 运行时拓扑变更——并给出了超越 LangGraphJS 现状的设计方案。草案的五大核心原则（superstep 内拓扑不变、barrier 生效、checkpoint 绑定 revision、mutation 是 write、cursor 退出执行核心）构成了稳固的决策框架。

以下 review 基于 existing_truth.md 和 research_results.md，不参考其他 review 文档。

---

## 强项

1. **边界控制严格**：明确限定只做后端 Rust、不做前端、不保留 cursor 作为调度事实，避免了范围蔓延。
2. **核心原则与 Pregel 论文对齐**：barrier apply、deterministic ordering、checkpoint-revision 绑定都符合 Pregel 3.4 语义。
3. **冲突策略务实**：本轮不做 Arena/UI gate，统一 reject，降低实现复杂度。
4. **模块修改清单完整**：从 definition types 到 coordinator 到 runner resume，覆盖面广，可作为开发 checklist。
5. **测试场景覆盖主干**：11 个测试用例覆盖了初始化、旧图兼容、分支、并发、Send、增删节点边、冲突、checkpoint replay、cursor 清理。

---

## 与现有事实（existing_truth.md）的对比：需澄清的 gaps

### 1. Branch / Loop 节点移除 `next_nodes` 后的替代机制未定义

草案要求删除 `NodeOutcome.next_nodes`，并声明 Branch/Loop "通过 output 或 side effect 表达选中的 target"。但 existing_truth.md 显示当前 Branch 节点的选择结果正是通过 `next_nodes` 传给 coordinator，再由 coordinator 决定写哪个 branch channel。

**问题**：移除 `next_nodes` 后，Branch executor 具体如何向 lowering 层传递"选中了 target X"这一信息？是通过：
- `Command.goto`？
- `NodeOutcome.output` 中的某个约定字段？
- 新的 `NodeOutcome` 字段如 `selected_branch`？
- 还是 lowering 层根据 Branch 节点的 output value 直接计算？

**建议**：草案必须明确 Branch/Loop 选择结果的表达协议。推荐沿用 `Command.goto`（如果节点输出中包含 goto 指令，lowering 层将其转化为对应 branch channel write），这样与现有 `Command` 降低路径一致，改动最小。

### 2. `NodeOutcome.graph_mutations` vs `GRAPH_MUTATIONS_CHANNEL` 的写入路径存在语义分歧

草案在「Mutation 写入通道」节声明 `__pregel_graph_mutations` 是 reserved runtime channel，"只接受 `GraphMutationRequest` 或 request array"。但在「NodeOutcome 变化」节又说新增 `NodeOutcome.graph_mutations: Vec<GraphMutationRequest>`。

**问题**：节点产生 mutation request 后，是：
- (A) 直接写入 `NodeOutcome.graph_mutations`，由 coordinator 在 outcome lowering 时分离？
- (B) 节点执行器像写普通 channel 一样写入 `GRAPH_MUTATIONS_CHANNEL`，再被 barrier 收集？
- (C) 两者等价，即 outcome lowering 时把 `graph_mutations` 映射为 channel write？

如果是 (A) 或 (C)，mutation request 会经过 outcome -> writes -> pending writes -> barrier 的路径，但草案又说"mutation request 不应该混进普通 `channel_values` 作为下一轮 task trigger"。这需要在 writes lowering 阶段有明确的分流逻辑。

**建议**：在「NodeOutcome 变化」或「Mutation 写入通道」节增加一句话明确：
> `NodeOutcome.graph_mutations` 在 outcome lowering 阶段被提取出来，不进入普通 `PregelWrite` 集合，而是直接交给 barrier 的 mutation collector，避免它们被当作 channel values 参与下一轮 task trigger。

### 3. `SuperstepCheckpoint` 的字段变更与现有持久化结构的兼容性

草案建议 `SuperstepCheckpoint` 删除 cursor 字段并新增 `graph_revision_before/after` 和 `mutation_batch_id`。existing_truth.md 没有展示 `SuperstepCheckpoint` 的完整字段列表，但确认了它已存在于 `run_state/superstep.rs`。

**问题**：如果 `SuperstepCheckpoint` 已经在生产/测试环境中持久化了旧格式，删除字段是否会导致反序列化失败？Rust 项目中是否使用了 `serde(default)` 或版本化反序列化策略？

**建议**：草案应说明持久化格式的兼容性策略，例如：
- 使用 `#[serde(default)]` 让旧 checkpoint 在新字段上获得默认值；
- 或者声明本轮同时负责 migration（因为 feature/orni 尚未合并 main，可以接受 breaking change）。

### 4. `GraphRevision.graph: TaskGraphDefinition` 的存储冗余与加载性能

草案建议每个 revision 存储完整的 `TaskGraphDefinition` 副本。existing_truth.md 显示当前 graph definition 包含 nodes、edges、config 等完整结构。

**问题**：如果 graph 很大（几十个节点、复杂 config），每轮 mutation 都存全量副本可能导致存储膨胀。但草案未讨论这一点。

**建议**：本轮可以接受全量快照（与 checkpoint 设计一致，优先正确性），但建议在草案中加注一句：
> 本轮使用全量 snapshot 保证恢复正确性；后续如需要优化，可引入 delta-based revision 或 Merkle 结构。

---

## 与研究结果（research_results.md）的对齐评价

### 1. 正确区分了 LangGraphJS 的能力边界

research_results.md 的核心发现是：LangGraphJS 只有 `static topology + dynamic task dispatch`，没有 true runtime topology mutation。草案没有错误地把 LangGraphJS 的 `Send` / `Command.goto` 当作拓扑变更来模仿，而是选择直接实现 Pregel 3.4 语义。**这是正确的差异化方向。**

### 2. `Send` 到 mutation 新增节点的时序边界应更明确

research_results.md 证明 LangGraphJS 的 `Send` 目标必须存在于 `compiled.processes` 中，否则报错。草案也继承了这一限制（"mutation 新增了 node，则必须等下一 revision compile 后，下一 superstep 才能向该 node Send"）。

**问题**：如果节点在同一个 superstep 中既提交了 `add_node` mutation，又输出了指向该新节点的 `Send` 或 `Command.goto`，会发生什么？草案的 coordinator 流程说 "prepare tasks from checkpoint + compiled graph"，此时新 node 尚未进入 compiled graph，`Send` 会找不到目标。

**建议**：在「Control flow 变化」或「冲突规则」中增加：
> 同一 superstep 内产生的 `Send` / `Command.goto` 目标，若指向本 batch 新增的 node，属于时序错误，应在 barrier apply 前被检测为 conflict 或忽略。建议本轮将其视为 `GraphMutationConflict` 的一种（` premature_send_to_uncompiled_node`），因为这种行为通常意味着节点逻辑有 bug。

### 3. `PatchNodeConfig` 是合理的 TaskGraph 扩展

Pregel 论文没有 patch config，但 TaskGraph 的节点配置（如 LLM prompt、Shell command）需要运行时调整。草案引入 `PatchNodeConfig` 是务实的。但应明确 patch 语义。

---

## 具体建议（按优先级）

### P0：必须补充到草案中

| # | 建议 | 理由 |
|---|------|------|
| 1 | 明确 Branch/Loop 节点在移除 `next_nodes` 后，选择结果如何传递给 lowering 层 | 当前最大的实现缺口，没有它 Branch 图会无法运行 |
| 2 | 明确 `NodeOutcome.graph_mutations` 与 `GRAPH_MUTATIONS_CHANNEL` 的关系，以及 lowering 阶段的分流逻辑 | 避免 mutation request 被误当作普通 channel write 触发下一轮 task |
| 3 | 明确 `PatchNodeConfig.patch` 的语义：是 `serde_json::merge`（递归合并）还是 `replace`（整字段替换）？ | 影响节点 executor 的行为和测试预期 |
| 4 | 在测试列表中增加 `PatchNodeConfig` 的测试用例 | 草案支持该 op 但测试未覆盖 |

### P1：强烈建议补充

| # | 建议 | 理由 |
|---|------|------|
| 5 | 在「冲突规则」中明确：dedupe 只针对**同 batch** 的重复操作；remove 不存在的 node 在跨 batch 场景下（上一 batch 已 remove）也应 conflict | 避免实现时产生歧义 |
| 6 | 在 coordinator 流程中增加 graph revision assert 失败的处理策略 | 例如：用户手动修改了 graph definition 文件，导致 run.current_graph_revision 与 checkpoint 不匹配 |
| 7 | 增加"remove node 自动删除相关 edge"的测试用例 | 冲突规则已声明，测试应覆盖 |
| 8 | 增加"superstep 无 mutation 时 `graph_revision_before == graph_revision_after`"的测试 | 验证 checkpoint 字段的稳定性 |

### P2：可选优化

| # | 建议 | 理由 |
|---|------|------|
| 9 | `GraphRevision.created_at` 从 `String` 改为 `chrono::DateTime<Utc>` 或 `u64` timestamp | 类型安全，避免序列化格式歧义 |
| 10 | 考虑 `GraphRevision.graph` 是否使用 `Arc<TaskGraphDefinition>` 减少内存拷贝 | 运行时性能，不影响持久化 |
| 11 | 在「持久化文件」节说明 revision 存储是否可配置 retention | 长期运行可能积累大量 revision 文件 |

---

## 风险

1. **Branch/Loop 控制流断裂风险（高）**：如果移除 `next_nodes` 后的替代方案在实现时才被设计，可能导致 Branch/Loop 图在开发中期不可用。建议在开始大规模重构前，先确定 Branch/Loop 的新协议。
2. **持久化格式 breaking change 风险（中）**：`SuperstepCheckpoint` 删除 cursor 字段并新增 revision 字段，如果反序列化不兼容，会导致已有测试数据或 run state 无法加载。建议在修改 serde 结构时，先写兼容性测试。
3. **PatchNodeConfig 类型安全风险（中）**：`serde_json::Value` 的 patch 可能在运行时产生无效配置，导致节点执行 panic。建议在 patch apply 阶段增加 config schema validation（复用 `TaskGraphNode` 的验证逻辑）。
4. **cursor 清理不彻底风险（中）**：`coordinator.rs` 和 `runner.rs` 中的 cursor 代码路径较多，如果清理不彻底，可能在某些边缘 case（如 HumanGate resume、interrupt after）中重新引入 cursor 依赖。建议用编译期删除字段（即真正从 struct 中移除 cursor 字段）来强制发现残留，而不是仅标记为 deprecated。

---

## 结论

spec_draft.md 是一份**方向正确、架构合理、范围清晰**的草案，具备进入开发阶段的条件。主要缺口集中在 **Branch/Loop 控制流替代协议**、**mutation 写入路径的精确语义**、以及 **PatchNodeConfig 的 patch 语义** 三个方面。建议在正式开工前，用 1-2 段话补充这三个点，测试列表同步增加 PatchNodeConfig 覆盖，即可作为可信的实现依据。
