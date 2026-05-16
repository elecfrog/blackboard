# CodeBuddy GLM-5.1 Review: Pregel Topology Mutation Spec Draft

> Reviewer: CodeBuddy (GLM-5.1)
> Date: 2026-05-16
> Based on: `existing_truth.md` + `research_results.md` + `spec_draft.md`

## 总评

草案整体结构清晰，从背景事实到核心原则、模型定义、流程设计、模块拆分、测试要求、验收标准，逻辑链完整。核心决策——mutation 只在 barrier 生效、checkpoint 绑定 revision、cursor 从执行核心移除——与 Pregel 论文语义和现有代码事实一致。

以下是发现的具体问题，按严重程度排序。

---

## P0: 必须在实现前解决

### 1. Mutation apply 后 graph 编译失败未处理

Coordinator 新流程（spec_draft L456-L479）在 mutation applied 后执行：

```text
compile new revision
```

但未讨论：如果新 revision 的 `TaskGraphDefinition` 在编译阶段校验失败（例如 add_edge 引用了不存在的外部资源、新增 node config 非法），流程如何走？

当前 `compile/compiler.rs` 会返回 `Result<CompiledGraph, ...>`，编译失败意味着没有可用的 `CompiledGraph` 来 prepare 下一轮 task。

**建议**：在 mutation apply 阶段增加 compile 预检。如果 compile 失败，整个 mutation batch 应视为 rejected（`GraphMutationBatchResult::Rejected`），新增 conflict variant 表达 compile failure，不写入新 revision。

### 2. Mutation 与 pending writes / channel 一致性

spec_draft L573 提到：

> mutation request 不应该混进普通 channel_values 作为下一轮 task trigger

但未讨论：如果 superstep N 中节点 A 写入了 channel X，同时同一 superstep 的 mutation remove 了 node B（而 B 是 X 的下游消费者），下一 superstep prepare tasks 时，channel X 的 write 存在但消费方已不存在。

更危险的情况：mutation remove 了一个 node，但该 node 在当前 superstep 还有未完成的 pending writes replay。

**建议**：spec 应明确——mutation apply 发生在 normal writes apply 之后、下一 superstep prepare 之前。mutation remove node 时，下一 superstep 的 prepare 阶段基于新 compiled graph，自然不会为已删除 node 生成 PULL task。但当前 superstep 已完成的 node 的 writes 应该正常 apply（它们属于已承诺的事实），不应该因为同 batch mutation 而丢弃。spec 需要显式写出这条保证。

### 3. PatchNodeConfig patch 语义未定义

`GraphMutationOp::PatchNodeConfig` 使用 `serde_json::Value` 作为 patch（spec_draft L170），但未定义 patch 语义：

- JSON Merge Patch (RFC 7396)?
- JSON Patch (RFC 6902)?
- 自定义 shallow merge?
- 深度 merge 规则（数组替换还是 append）？

不同语义下，同样的 patch 值会产生不同结果。这是一个 P0 设计空缺。

**建议**：本轮最小可行方案是 JSON Merge Patch (RFC 7396)——null 表示删除 key，其他值浅替换。如果需要更精细操作，留到后续迭代。无论选哪种，spec 必须写明。

### 4. 持久化原子性

spec_draft L422-L449 列出了新增持久化文件和字段，但未讨论 mutation apply 期间的写入原子性。

一次 mutation apply 至少涉及：
1. 写 `graph_revisions/<new_revision>.json`
2. 写 `mutation_batches/<batch>.json`
3. 更新 `run.json` 的 `current_graph_revision`
4. 写 `SuperstepCheckpoint` 的 `graph_revision_after`
5. 写 `PregelCheckpoint` 的 `graph_revision`

如果进程在步骤 2 和 3 之间崩溃，恢复时会读到旧 revision 但磁盘上已有新 revision 文件。checkpoint 和 run state 不一致。

**建议**：spec 应增加崩溃一致性约束。方案选项：
- (a) 两阶段：先写新 revision 文件（不影响调度），再原子更新 run.json 的 `current_graph_revision`（作为 commit point）。恢复时以 run.json 为准。
- (b) 利用现有的 superstep checkpoint 机制，让 `graph_revision_after` 作为 commit point。

无论哪种，需要在 spec 中写明恢复时如何判断 revision 是否已生效。

---

## P1: 设计缺陷或歧义，建议本轮解决

### 5. remove node + 同 batch add 同 id node 未覆盖

冲突规则（spec_draft L349-L404）处理了：

- add 相同 node id 不同 spec → conflict
- remove node 后又 patch 同 node → conflict

但未讨论：remove node X + add node X（同 id、不同 spec）在同一 batch。

按 apply 顺序（remove 先于 add），remove X 后 X 不存在，add X 相当于新增。但如果 batch 中还有其他操作引用 X（例如 add edge 指向 X），此时 X 是 "remove 后重新 add" 的 X 还是 "原始" 的 X？

**建议**：显式声明——remove + add 同 id 视为 replace，add 的 spec 即为最终 spec。如果希望更保守，可以标记为 conflict 由用户拆分为两个 superstep。spec 至少需要声明一种行为。

### 6. remove node 自动删除关联 edge 与同 batch add_edge 冲突

spec_draft L369-L371：

> remove node 时，该 node 关联 edge：自动删除相关 edge

但如果同 batch 有一条 `add_edge` 指向被 remove 的 node，按 apply 顺序（remove edge → remove node → add node → add edge），add edge 在最后，此时：
- 如果 remove + add 同 id node = replace，add edge 指向的是新 node，应该成功。
- 如果 remove 后 node 确实不存在且无 add，add edge 的 endpoint 不存在 → conflict。

两种路径结果不同。spec 需要明确 cascade remove 的时机：是 apply 阶段隐式展开（把 cascade remove edge 插入到 remove edge 阶段），还是在 add edge 阶段才检查 endpoint 存在性。

**建议**：cascade remove edge 应在 mutation apply 时隐式展开并插入 request 列表（标记为 auto-generated），参与统一排序。这样 add edge 阶段检查时，如果同 batch 有 add node，endpoint 就存在。

### 7. Command.goto 目标为 mutation 新增 node

spec_draft L323-L331 讨论 Send + mutation 的交互，但未讨论 Command.goto：

> Command.goto：如果 goto 是 node name，则写对应 branch/join channel

如果当前 superstep 的节点通过 Command.goto 跳转到下一 superstep 才由 mutation 新增的 node，当前 compiled graph 中没有该 node 的 branch/join channel。Command.goto lowering 会找不到目标 channel。

**建议**：明确 Command.goto 只能指向当前 revision 已编译存在的 node。goto 到 mutation 新增 node 属于非法操作，在 lowering 阶段应报错或忽略。需要该行为的用户必须拆分为两步：先 mutation add node，下一 superstep 再 goto。

### 8. 现有运行迁移

spec 未讨论：feature 合入后，已存在的 run（处于 paused / 有 cursor state）如何迁移。

- 这些 run 没有 graph revision 文件。
- `SuperstepCheckpoint` 有 `cursor_before/cursor_after` 但没有 `graph_revision_before/after`。
- 恢复这些 run 时期望 `checkpoint.graph_revision == run.current_graph_revision`，但两者都不存在。

**建议**：增加迁移策略——run 首次加载时，如果不存在 revision 文件，从 graph snapshot 创建 revision 0，`current_graph_revision = 0`，checkpoint 补 `graph_revision = 0`。或者声明本轮不支持历史 run 恢复，需要新创建 run。

### 9. interrupt_before/after 与 mutation 的交互

existing_truth.md 确认已实现 `interrupt_before` / `interrupt_after`。spec 未讨论：

如果 mutation add 了一个 node，而该 node 在某个 interrupt 列表中，下一 superstep 会在 prepare 阶段检测到该 node 需要 interrupt_before 吗？

interrupt 配置是 run-level 还是 graph-level？如果跟着 graph revision 走，新 revision 里的新 node 是否自动继承了 interrupt 配置？

**建议**：明确 interrupt 配置的归属。如果是 compile-time 参数（当前实现中 `PregelLoop` 支持 `interrupt_before/after`），则新 revision compile 时需要重新传入。spec 应声明：interrupt 配置属于 run 级别，不受 graph revision 影响；mutation add node 后，如果该 node name 在 run 的 interrupt 列表中，下一 superstep 会在执行前 interrupt。

---

## P2: 建议改进但不阻塞

### 10. GraphRevision 生命周期管理

spec 未讨论旧 revision 的保留策略。长时间运行的 graph 可能产生大量 revision 文件。

**建议**：本轮不做 GC，但可以在 spec 中加一条注释：旧 revision 至少保留到 run 结束（因为 checkpoint replay 需要）。run 结束后可以由外部清理。不需要本轮实现。

### 11. 性能：每次 mutation 后全量 recompile

当前 `CompiledGraph` 编译是全量的。每次 mutation apply 后都需要 recompile，对于大图可能有性能问题。

**建议**：本轮不优化，但 spec 可以记录：增量编译是已知后续优化方向。当前 recompile 在 superstep barrier 中执行，不在 hot path。

### 12. GraphMutationBatchResult::Rejected 后 run 状态

spec_draft L206-L209：

> 只要出现无法自动解决的冲突，本 batch rejected，run failed 或 paused。

"failed 或 paused" 是两种截然不同的结果。failed 意味着 run 终止，paused 意味着可以外部干预后继续。

**建议**：本轮建议选择 failed（与 spec 的 "不做 Arena、不做 UI gate" 一致）。如果未来引入 Arena，再改为 paused + human review。spec 应明确选择一种，不要留 "或"。

### 13. mutation_batch_id 的生成策略

`GraphRevision.mutation_batch_id: Option<String>` 和 `GraphMutationBatch.id: String` 未定义 ID 生成策略。

**建议**：使用 `{superstep}` 或 `{superstep}-{short_uuid}` 即可。本轮不需要全局唯一，run 内唯一即可。

### 14. 测试覆盖补充建议

现有测试列表（spec_draft L647-L753）覆盖了主要路径，建议补充：

- **mutation 后 Command.goto 到已存在 node**：确保控制流在 revision 变更后仍然正确。
- **空 mutation batch**：superstep 中有 mutation channel write 但内容为空数组 → 不产生新 revision，正常继续。
- **连续多个 superstep 各自 mutation**：验证 revision 递增和链式 compile。
- **remove 当前 superstep 已执行 node 的下游**：验证 writes 仍然 apply，只是下一 superstep 不再调度。
- **并发 mutation（多节点同 superstep 各自提交 mutation）**：验证 batch 收集和 conflict 检测。

---

## 事实一致性检查

| spec 声明 | existing_truth 对照 | 结论 |
|---|---|---|
| 当前有 CompiledGraph | existing_truth §2 确认 | ✅ 一致 |
| 当前有 Pregel checkpoint/task/write | existing_truth §4-6 确认 | ✅ 一致 |
| 当前缺失 topology mutation | existing_truth §明确未实现 §1 确认 | ✅ 一致 |
| cursor 残留列表 | existing_truth §12 确认 cursor 仍存在 | ✅ 一致 |
| Send 只能到已有 process | existing_truth §明确未实现 §3 + research_results §4 确认 | ✅ 一致 |
| mutation apply 顺序引用 Pregel 3.4 | research_results §Comparison 确认 Pregel 3.4 语义 | ✅ 一致 |
| LangGraphJS 未实现 topology mutation | research_results 结论确认 | ✅ 一致 |

---

## 总结

草案的核心设计——barrier mutation + graph revision + cursor 移除——方向正确，与现有代码事实和研究结论一致。主要风险点集中在：

1. **编译失败处理**（P0）：mutation apply 后 compile 可能失败，需要回退策略。
2. **持久化原子性**（P0）：多文件写入需要崩溃一致性保证。
3. **PatchNodeConfig 语义**（P0）：patch 规则未定义，实现会有歧义。
4. **mutation 与 writes 一致性**（P0）：需要明确 normal writes 与 mutation 的 apply 时序和互影响。
5. **现有 run 迁移**（P1）：合并后历史 run 的恢复路径需要明确。

建议在实现前至少解决 P0 项，P1 项可以在实现初期迭代解决。
