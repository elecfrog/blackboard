# Review: Pregel Topology Mutation 后端内核草案

> Reviewer: CodeBuddy Opus 4.6
> Review round: 1
> Target: `spec_draft.md`
> Reference: `existing_truth.md`, `research_results.md`

## 总评

草案方向正确：在已有的 Pregel superstep/channel/checkpoint 基础上增加 graph revision + topology mutation barrier，同时清理 cursor 残留。设计与 Pregel 论文 3.4 对齐，并且正确吸收了 LangGraphJS 研究结论——LangGraphJS 没有实现 topology mutation，因此本轮是超越参考实现的原创扩展。

以下按优先级分类提出 review items。

---

## P0 — 必须在实现前澄清的设计缺陷

### 1. Graph revision 持有完整 `TaskGraphDefinition` 会导致存储膨胀

草案中 `GraphRevision` 包含完整 `graph: TaskGraphDefinition`。每次 mutation（即使只加一条 edge）都会生成一份完整 graph snapshot。对于长寿命 run 或频繁 mutation 场景，revision 文件会线性膨胀。

**建议**：

- 方案 A：revision 只存 delta（mutation batch），恢复时从 revision 0 顺序 replay。checkpoint 绑定 revision number，replay 到对应 revision 即可。优点是存储紧凑；缺点是恢复需要 replay 链。
- 方案 B：保持完整 snapshot，但增加 compaction 策略（例如每 N 个 revision 写一次 full snapshot，中间只存 delta）。
- 方案 C（草案当前方案）：每个 revision 都是 full snapshot。简单但膨胀。

本轮如果选方案 C，建议在草案中**显式声明这是有意的 v1 简化**，并在后续迭代中收敛到 A 或 B。否则实现者可能会自行引入 delta 逻辑，增加不确定性。

### 2. Mutation 与 normal writes 的 barrier 时序需要更精确

草案 coordinator 流程写的是：

```text
apply normal writes at barrier
collect mutation requests
apply topology mutations if any
```

问题：mutation request 是作为 `PregelWrite` 写入 `__pregel_graph_mutations` channel 的，那它是在 "apply normal writes" 时被消费、还是被跳过留给后续 "collect mutation requests" 步骤？

如果 `__pregel_graph_mutations` 和普通 channel 一起进入 `apply_writes`，那消费逻辑需要特殊处理——不能把 mutation request 当作普通 channel value 参与 version bump 和 trigger 计算。

**建议**：在草案中明确以下时序：

```text
1. barrier 收集所有 pending writes
2. 分离 normal writes 和 mutation writes
3. apply normal writes（更新 channel_values/versions/triggers）
4. extract mutation requests from mutation writes
5. apply topology mutations（排序、冲突检测、生成新 revision）
6. compile new revision（如果有 mutation）
7. commit checkpoint（记录新 graph_revision）
```

并明确 mutation channel **不参与** `channel_versions` / `versions_seen` / trigger 计算。

### 3. Mutation 后的 channel state 迁移未定义

当 mutation 新增了 node（例如 `review`）并新增了指向该 node 的 edge 时，新 compiled graph 会产生新的 channels（例如 `branch:current->review`）。这些新 channel 在当前 checkpoint 中不存在。

草案没有定义：

- 新 channel 的初始 value 是什么？
- 新 channel 的初始 version 是什么？
- 新 channel 如何参与下一 superstep 的 `versions_seen` 比较？

**建议**：定义 channel state 迁移策略：

- 新增 channel 初始 value = channel class 的 zero value（`None` / `[]` / `0`）。
- 新增 channel 初始 version = 当前 superstep（这样 `versions_seen` 中没有该 channel 的记录，PULL task 会被自然触发）。
- 删除的 channel 从 checkpoint 中移除。
- 保留 channel 的 value/version 不变。

### 4. Remove node 时正在执行或已暂停的 task 如何处理

草案定义了 remove node 时自动删除相关 edge，但没有定义以下情况：

- 如果被删除的 node 在当前 superstep 有已完成的 task（其 writes 已收集但尚未 apply），是否仍然 apply 这些 writes？
- 如果被删除的 node 处于 `interrupt_before` 暂停状态，remove 后 resume 会怎样？

**建议**：

- 同一 superstep 中，node 的 writes 先于 mutation apply，所以已完成 task 的 normal writes 应正常 apply。但如果该 node 同时请求删除自己，需要判断是否合法。
- 暂停态 node 被删除时，对应的 paused action 应标记为 cancelled，不允许 resume。

---

## P1 — 设计可改进但不阻塞实现

### 5. `GraphMutationOp::PatchNodeConfig` 的 patch 语义需要约束

草案用 `serde_json::Value` 作为 patch payload。这过于宽松：

- 是 JSON Merge Patch (RFC 7396)？
- 是 JSON Patch (RFC 6902)？
- 还是 deep merge？

不同语义对冲突检测影响很大。例如两个 task 同时 patch 同一 node 的不同字段，merge patch 可以自动合并，而 replace 式 patch 必须冲突。

**建议**：本轮选择 JSON Merge Patch (RFC 7396) 作为唯一语义，并在冲突规则中补充：同 batch 对同一 node 的多次 patch，如果 merge 后无 key 冲突则自动合并，有 key 冲突则 reject。

### 6. 冲突规则中 "remove 不存在的 node → conflict" 过于严格

在分布式/并发 agent 场景下，两个 node 可能同时请求删除同一个 node。按草案当前规则，第一个 dedupe 成功，但如果 node 在上一 superstep 已被删除（跨 superstep），则当前 batch 中的 remove 会触发 conflict 导致 run fail。

**建议**：区分两种情况：

- 同 batch 重复 remove → dedupe（草案已支持）。
- 目标 node 在当前 revision 中已不存在 → warn + skip（幂等语义），而非 conflict。

这更符合 Pregel 论文中 "local mutations are applied immediately" 的精神——删除已不存在的东西是无害的。

### 7. Cursor 清理策略需要更清晰的迁移路径

草案说 "本轮如果完全不管前端，可以先不新增 projection"。但 existing_truth 显示 cursor 仍被 coordinator 和 run_state 大量引用。一次性删除所有 cursor 引用会导致前后端 API contract 断裂。

**建议**：分两步：

1. **本轮**：在执行核心中用 feature gate 或 cfg 控制 cursor 代码路径，默认关闭。保留 `active_nodes` 作为 API 兼容 projection（从 prepared tasks 中 derive）。这样后端测试可以验证无 cursor 路径，而 API 层不会立即 break。
2. **下一轮**：前端适配 `active_nodes`，删除 cursor 相关代码。

### 8. 缺少 mutation 的审计/event 模型

草案定义了 `GraphMutationBatch` 持久化，但没有定义 mutation 事件如何进入 `RunEvent` 流。

**建议**：新增 `RunEvent` variant：

```rust
RunEvent::TopologyMutation {
    superstep: u64,
    batch_id: String,
    result: GraphMutationBatchResult,
}
```

这对 debug、UI 回放、inbox 交接都很重要。

---

## P2 — 细节建议

### 9. `GraphRevision.created_at` 建议用 `DateTime<Utc>` 而非 `String`

草案用 `String` 存时间戳。既然后端是 Rust，建议用 `chrono::DateTime<Utc>` 或 `time::OffsetDateTime`（取决于项目已有依赖），序列化时自然输出 ISO 8601。

### 10. `GraphMutationBatch.result` 在创建时是未知的

草案把 `result` 放在 `GraphMutationBatch` 中，但 batch 在收集阶段还没有 result。

**建议**：

- 收集阶段的中间表示只有 `requests`。
- apply 后生成 `AppliedMutationBatch` 或 `MutationBatchOutcome`，包含 `result`。
- 持久化的是最终结果。

或者把 `result` 改为 `Option<GraphMutationBatchResult>`，初始为 `None`。

### 11. 持久化路径中 revision/batch 编号建议用 superstep 对齐

草案用自增编号 `000000.json`。建议用 superstep number 作为文件名（或 `<superstep>_<revision>.json`），这样恢复时可以直接从 superstep number 定位对应文件，不需要额外索引。

### 12. `GRAPH_MUTATIONS_CHANNEL` vs 独立的 mutation collector

草案把 mutation 建模为 reserved channel write。这增加了 `apply_writes` 的复杂度（需要过滤 mutation channel）。

替代方案：不通过 channel 传递 mutation，而是在 `NodeOutcome` 中直接携带 `graph_mutations`（草案已定义），coordinator 在 outcome 收集阶段直接提取，不经过 channel。这样 channel 系统保持纯净——只处理数据和控制流。

草案同时定义了 `NodeOutcome.graph_mutations` 和 `__pregel_graph_mutations` channel，有冗余。**建议选一个路径**：

- **推荐**：`NodeOutcome.graph_mutations` → coordinator 直接收集 → barrier apply。不需要 mutation channel。
- 备选：mutation channel 路径，但需要从 `NodeOutcome` 中移除 `graph_mutations` 字段。

### 13. 测试用例 6 需要更精确的前置条件

测试 "Add node mutation" 描述的是 "superstep N 中节点提交 add node + add edge"。但没有说明哪个节点提交——如果是 `current` 节点自己提交，那它需要某种方式表达 mutation request。

**建议**：补充说明 mutation request 的触发路径。例如：

- LLM 节点通过 `Command` 扩展返回 mutation。
- Shell 节点通过 JSON artifact 返回 mutation。
- 或者引入新的 `GraphMutation` command variant。

这会影响 `command.rs` 和 node executor 的改动范围。

---

## 总结表

| ID | 优先级 | 类型 | 标题 |
|----|--------|------|------|
| 1 | P0 | 存储 | GraphRevision full snapshot 膨胀 |
| 2 | P0 | 时序 | Mutation vs normal writes barrier 分离 |
| 3 | P0 | 状态迁移 | Mutation 后新增 channel 的初始 state |
| 4 | P0 | 边界 | Remove node 时进行中/暂停 task 处理 |
| 5 | P1 | 语义 | PatchNodeConfig 的 patch 语义约束 |
| 6 | P1 | 冲突 | Remove 不存在 node 的幂等性 |
| 7 | P1 | 迁移 | Cursor 清理的渐进策略 |
| 8 | P1 | 审计 | Mutation event 进入 RunEvent 流 |
| 9 | P2 | 类型 | created_at 用强类型时间 |
| 10 | P2 | 模型 | MutationBatch.result 的生命周期 |
| 11 | P2 | 持久化 | Revision 文件名用 superstep 对齐 |
| 12 | P2 | 架构 | Mutation channel vs NodeOutcome 直接携带，二选一 |
| 13 | P2 | 测试 | Add node 测试的 mutation 触发路径 |

## 建议的下一步

1. 先回应 P0 items 1–4，在草案中补充明确定义。
2. 然后进入实现阶段，P1/P2 可在实现过程中逐步收敛。
3. cursor 清理建议分两轮，本轮先在执行核心禁用，下轮清理 API 层。
