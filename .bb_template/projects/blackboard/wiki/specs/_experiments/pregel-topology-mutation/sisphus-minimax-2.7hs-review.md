# Sisyphus-M2.7HS Review: Pregel Topology Mutation 后端内核草案

**审查日期**: 2026-05-16
**审查对象**: `spec_draft.md`
**并行审查**: existing_truth.md + research_results.md

---

## 1. 执行摘要

`spec_draft.md` 描述了一个雄心勃勃的重构目标：将 TaskGraph 后端从"静态 compiled graph + cursor 投影"演进到"graph revision + topology mutation barrier + Pregel checkpoint as sole scheduling truth"。

**总体判断**: 草案设计方向正确，但存在若干需要澄清的实现风险点。

---

## 2. 架构设计评估

### 2.1 核心原则 (第 59-108 行)

五条核心原则逻辑自洽，排序合理：

| # | 原则 | 评估 |
|---|------|------|
| 1 | superstep 内 graph topology 不变 | ✅ 正确，符合 Pregel 语义 |
| 2 | mutation 在 barrier 生效 | ✅ 正确 |
| 3 | checkpoint 必须绑定 graph revision | ✅ 正确，但需要澄清 rollback 场景 |
| 4 | mutation 是 write，不是直接文件修改 | ✅ 正确 |
| 5 | cursor 不能继续作为执行核心状态 | ⚠️ 激进，需确保 projection 层不遗漏 |

**风险点 1**: 原则 3 中"恢复时必须先恢复对应 revision 的 graph"——如果 `graph_revisions/` 目录损坏或缺失中间 revision，是否有完整性和一致性校验？建议草案补充 recovery 失败的处理策略。

**风险点 2**: 原则 5 对"只读 projection"的处理模糊。当前草案说"可以先从核心数据结构移除 cursor"，但没有明确 `TaskGraphRun.cursor` 字段是删除还是标记 deprecated。删除是破坏性变更，可能影响 API 兼容层。

### 2.2 新增核心模型完整性

#### GraphRevision (第 112-140 行)

```rust
pub struct GraphRevision {
    pub revision: u64,
    pub graph: TaskGraphDefinition,
    pub created_at: String,
    pub parent_revision: Option<u64>,
    pub mutation_batch_id: Option<String>,
}
```

**问题 A**: `graph: TaskGraphDefinition` 是内联完整定义还是引用？完整内联会导致大量冗余存储（每个 revision 都复制完整 graph）。建议明确是内联快照还是 reference+delta。

**问题 B**: `parent_revision` 和 `mutation_batch_id` 提供了追溯链，但草案未说明是否需要用这个链做 conflict detection（比如检测同 batch 内矛盾的多方 mutation）。

#### GraphMutationOp (第 158-173 行)

最小集合设计合理。`PatchNodeConfig` 是必要扩展。

**缺失**: 没有看到对 `ReplaceNode` 的考虑——某些场景下完整替换比 patch 更清晰。草案目前只支持 add/remove/patch，是否够用需要实际场景验证。

#### GraphMutationBatchResult (第 193-205 行)

```rust
pub enum GraphMutationBatchResult {
    Applied { new_revision: u64, summary: GraphMutationSummary },
    Rejected { conflicts: Vec<GraphMutationConflict> },
}
```

**关键问题**: 草案说"只要出现无法自动解决的冲突，本 batch rejected"。但没有定义"自动解决"的边界。dedupe 算自动解决，但 patch vs remove 同 node 呢？草案选择了 conflict，这比其他方案（remove wins）更安全，但可能导致频繁 rejected。建议明确"自动解决"的完整规则表。

### 2.3 NodeOutcome 变化 (第 249-275 行)

删除 `next_nodes`，新增 `graph_mutations`。控制流从 `next_nodes` 链切换到"compiled writers decide"。

**架构风险**: 这是一个从"节点控制流声明"到"编译时 writer 绑定"的根本性转变。当前所有节点 executor 都隐式依赖 `next_nodes` 做分支路由。这个改动影响面极广，需要逐节点审查。草案在第 639-646 行提到了节点修改范围，但没有明确说明过渡策略——是同时保留旧路径然后废弃，还是一次性切换？

---

## 3. Mutation Apply 顺序 (第 333-348 行)

草案定义的顺序：
1. remove edge
2. remove node
3. add node
4. patch node config
5. add edge

**评估**: 顺序正确，参考了 Pregel 论文 3.4。但有一个边界情况未覆盖：

**边界**: 如果 batch 内既有 `remove_node(X)` 又有 `add_node(X)` 但 spec 不同（冲突），按当前顺序会先 remove 再 add，最终图包含新的 X。但草案第 376-378 行说这是 conflict。**矛盾**：顺序逻辑会允许这种情况，但冲突规则说不行。

**建议**: 明确这个场景是"先 remove 再 add"还是"直接 conflict"。如果是后者，apply 顺序需要先做冲突检测再应用。

---

## 4. 冲突规则 (第 349-420 行)

### 4.1 可自动处理

dedupe 规则清晰。

### 4.2 必须拒绝

第 387-389 行：
```
remove node 后又 patch 同 node：
- conflict 或 remove wins
建议本轮选择 conflict，避免隐藏错误。
```

**问题**: `remove_node(X)` 后 `patch_node(X, ...)` 在 apply 顺序上是先 remove 再 patch，patch 实际上会作用在一个不存在的 node 上。这应该明确是 conflict 还是 no-op（因为 node 已不存在）。草案选择了 conflict，正确。

### 4.3 冲突错误枚举

草案建议用 `TaskGraphError::TopologyMutationConflict`，或先用 `ValidationFailed` 但 code 稳定。

**建议**: 直接新增 variant，避免后续 API 兼容性痛苦。

---

## 5. Cursor 清理评估 (第 487-528 行)

### 5.1 必须删除或停用列表

| 字段/概念 | 草案位置 | 评估 |
|-----------|----------|------|
| `TaskGraphRun.cursor` | 第 493 行 | ⚠️ 删除影响 API，标记 deprecated 更安全 |
| `update_cursor` | 第 493 行 | ✅ 删除 |
| `ReduceAction::Continue { next_cursor }` | 第 495 行 | ✅ 删除 |
| `SuperstepPlan.cursor_before` | 第 496 行 | ✅ 删除 |
| `SuperstepCheckpoint.cursor_before` | 第 497 行 | ✅ 删除 |
| `SuperstepCheckpoint.cursor_after` | 第 498 行 | ✅ 删除 |
| `NodeOutcome.next_nodes` | 第 499 行 | ✅ 删除 |

### 5.2 执行核心引用风险

**关键风险**: `coordinator.rs` 中 `update_cursor` 的所有调用点都需要找到并清除。草案第 603 行说"不调用 `update_cursor`"，但没有列出所有调用点清单。`resume_run` 清理（第 513-527 行）描述清晰，但需要源码级验证。

**HumanGate resume 清理** (第 610 行): "改为纯 Pregel write"——这个改动需要确保 resume write 触发路径完整覆盖原来的 cursor advance 逻辑。

---

## 6. 模块修改清单审查 (第 529-646 行)

### 6.1 高风险模块

| 模块 | 改动描述 | 风险级别 |
|------|----------|----------|
| `pregel/coordinator.rs` | 大改：graph revision load、prepare 不记 cursor、reduce 不算 next_cursor、commit 时 apply mutation | 🔴 高 |
| `pregel/outcome.rs` | 删除 cursor 依赖、删除 `next_nodes` | 🔴 高 |
| `nodes/*` | 所有 executor 不再调用 `resolve_next_nodes` | 🔴 高 |
| `run_state/model.rs` | 删除 cursor 或标记 deprecated | 🟡 中 |

### 6.2 中等风险模块

| 模块 | 改动描述 | 风险级别 |
|------|----------|----------|
| `pregel/writes.rs` | mutation channel write 支持、normal/mutation writes 分离 | 🟡 中 |
| `pregel/command.rs` | 移除 next_nodes 参数、compiled writers 决定 writes | 🟡 中 |
| `run_state/superstep.rs` | graph revision IO、mutation batch IO | 🟡 中 |

### 6.3 低风险模块

| 模块 | 改动描述 | 风险级别 |
|------|----------|----------|
| `pregel/runtime_channels.rs` | 新增 `GRAPH_MUTATIONS_CHANNEL` | 🟢 低 |
| `definition/types.rs` | 新增 mutation 相关类型 | 🟢 低 |
| `pregel/model.rs` | `PregelCheckpoint.graph_revision` | 🟢 低 |

**模块修改顺序建议**: 草案没有明确实现顺序。建议从低风险到高风险：
1. 先加新类型和数据结构（types, model, runtime_channels）
2. 再做 mutation apply 逻辑（writes.rs 的 mutation 分离）
3. 最后动 coordinator 和 outcome

---

## 7. 测试要求评估 (第 647-754 行)

### 7.1 测试覆盖完整性

草案要求 11 类测试，覆盖较全。关键测试评估：

| # | 测试 | 评估 |
|---|------|------|
| 1 | Graph revision 初始化 | ✅ 基础 |
| 2 | 无 mutation 旧图跑通 | ✅ 回归 |
| 3 | Branch 图跑通 | ✅ 回归 |
| 4 | Join barrier 跑通 | ✅ 回归 |
| 5 | Send 派发到已有节点 | ✅ 回归 |
| 6 | Add node mutation | ✅ 核心功能 |
| 7 | Remove node mutation | ✅ 核心功能 |
| 8 | Add edge mutation | ✅ 核心功能 |
| 9 | 冲突测试 | ✅ 核心功能 |
| 10 | Checkpoint replay | ✅ 核心功能 |
| 11 | cursor 删除测试 | ✅ 破坏性验证 |

### 7.2 缺失的测试场景

**未明确覆盖**：
- `patch_node_config` 的 apply 和冲突
- mutation batch 内跨 node 的因果依赖（比如 A add B，B remove A）
- Superstep N 提交的 mutation 在 Superstep N+1 因 conflict rejected 后，run 的状态是什么？paused？failed？
- graph revision 0 的完整性校验
- 并发 mutation 的 race condition（理论上不应该有，因为 barrier 是串行的）

### 7.3 "cursor 删除测试"实现方式

草案说"可以用编译期删除字段来强制发现残留"。这是**好主意**，但需要 Rust 语言级别的支持。建议：

```rust
// 在 build 脚本或 compile-time 检查
compile_fail_if_cursor_references_remain!();
```

---

## 8. 验收标准审查 (第 755-775 行)

### 8.1 验收命令

```powershell
cargo test -p bb_core task_graph --lib --manifest-path bb_backend/Cargo.toml
cargo test -p bb_cli --manifest-path bb_backend/Cargo.toml
```

**问题**: 验收标准只要求测试通过，没有要求：
- 特定的 mutation 行为验证
- 冲突 rejection 验证
- revision 绑定 checkpoint 验证

**建议**: 补充至少一个集成测试验证完整 mutation 流程。

### 8.2 排除范围合理性

排除前端 UI、schedule UI、graph editor——**正确**。本轮专注后端内核。

---

## 9. 与 existing_truth.md 的对齐分析

根据 `existing_truth.md`：

### 9.1 已实现 (相关部分)

- ✅ `CompiledGraph` IR - 草案依赖这个
- ✅ Pregel checkpoint - 草案需要扩展
- ✅ PULL/PUSH task scheduling - 草案需要适配 Send 到新 node
- ✅ `TASKS_CHANNEL` - 草案复用
- ✅ `Send` - 草案说"继续只能发送到已存在的 compiled process"
- ✅ superstep barrier apply writes - 草案复用
- ✅ pending writes replay - 草案需要绑定 mutation

### 9.2 草案未覆盖的 existing_truth 事实

**意外发现**: `existing_truth.md` 第 12 点提到 `cursor` 仍存在但"更像是 UI 和兼容层投影"。这与草案原则 5 有轻微矛盾——草案说 cursor 影响两套事实源，但 existing_truth 说 cursor 已经是投影性质。如果 cursor 已经是投影，清理成本可能低于草案预估。

**建议**: 核实 coordinator.rs 第 178、637 行和 run_state/model.rs 第 273 行的 cursor 实际使用程度。

---

## 10. 与 research_results.md 的对齐分析

根据 `research_results.md`：

LangGraphJS **没有**实现 Pregel 3.4 topology mutation。它的 dynamic dispatch 是 `Send` 到已存在的 node。

**草案设计是正确的差异化路线**。草案要实现的 runtime topology mutation 是 LangGraphJS 没有做到的。

**注意**: 草案第 331 行说"Send 不能发给 mutation 新增的 node，必须等下一 revision compile"。这与 LangGraphJS 的约束一致。草案设计没有引入与 LangGraphJS 模型的根本性冲突。

---

## 11. 总体风险矩阵

| 风险 | 概率 | 影响 | 优先级 |
|------|------|------|--------|
| cursor 清理影响 API 兼容性 | 高 | 高 | P0 |
| coordinator.rs 大改引入 regression | 高 | 高 | P0 |
| node executor 全面改写 | 高 | 高 | P0 |
| graph revision 存储膨胀（内联快照） | 中 | 中 | P1 |
| mutation 冲突检测边界不清 | 中 | 中 | P1 |
| apply 顺序与冲突规则矛盾 | 低 | 高 | P2 |
| 恢复时 graph revision 损坏 | 低 | 高 | P2 |

---

## 12. 建议行动

### P0 - 实施前必须澄清

1. **Cursor 字段处理决策**: 删除还是 deprecated？与 API 层协调。
2. **协调 nodes/* 所有 executor**: 确认 `resolve_next_nodes` 调用点数量，制定逐个修改计划。
3. **冲突规则与 apply 顺序一致性**: 明确 remove_node 后 patch 同 node 的处理方式。

### P1 - 实施中需要验证

4. **GraphRevision 存储格式**: 确认是完整内联还是 reference+delta，测试存储膨胀。
5. **补充集成测试**: 至少一个端到端 mutation 流程测试。
6. **resume_run 路径**: 验证 HumanGate resume 的 Pregel write 路径覆盖所有 cursor advance 场景。

### P2 - 优化项

7. **Recovery 策略**: graph_revisions 目录损坏时的处理。
8. **ReplaceNode 考虑**: 评估是否需要完整替换操作。
9. **跨 node 因果 mutation 测试**: 验证 batch 内 A→B 依赖的正确性。

---

## 13. 结论

`spec_draft.md` 的设计方向正确，与 Pregel 论文 3.4 语义对齐，且与当前分支已有的 Pregel 内核自然衔接。主要风险在于 cursor 清理和 coordinator 大改的 regression 可能性。

**实施建议**: 草案可以进入下一阶段，但需要先解决 P0 级别的澄清事项（cursor 字段决策和节点 executor 逐个修改计划）。建议实施前与 API 层协调 cursor 删除的影响范围。

---

*审查完成*
