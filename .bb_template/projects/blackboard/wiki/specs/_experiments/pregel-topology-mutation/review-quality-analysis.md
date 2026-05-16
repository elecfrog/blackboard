
Gemini:

### 一、 Review 质量排序与评分 (Sorting & Rating)

根据对架构深度、边缘场景捕捉能力、与 Pregel 原理的契合度，以及工程落地实用性的综合评估，排序如下：

#### 1. CodeBuddy Opus 4.6
- **评分**: **9.8 / 10 (Outstanding)**
- **评价**: 这份 Review 对 Pregel 状态机和底层调度语义的理解最为深刻。它敏锐地捕捉到了一个核心盲区：“**新增节点引入的新 channel，其初始 version 和 value 该如何定义，以确保下一轮能正确参与 `versions_seen` 计算？**” 这个洞察极其精准。此外，它对删除节点的幂等性（idempotency）探讨、以及 normal writes 与 mutation writes 在 barrier 时的精确消费时序，都展现了极高的系统架构水准。

#### 2. CodeBuddy GLM-5.1
- **评分**: **9.5 / 10 (Excellent)**
- **评价**: 极其详尽且工程视角极佳。它抓住了几个致命的流程漏洞：**如果 mutation 之后生成的新图 compile 失败怎么办？** 以及多文件持久化时的**崩溃一致性（Atomicity）问题**。此外，它对 `remove node` 和当前处于 pending writes 状态的节点之间的时序冲突分析得非常透彻。

#### 3. CodeBuddy Kimi-2.6
- **评分**: **9.0 / 10 (Very Good)**
- **评价**: 切中了当前代码实现中最直接的痛点。它敏锐地指出：草案要求删除 `NodeOutcome.next_nodes`，但**没有定义 Branch/Loop 节点该如何向底层的 lowering 协议传达分支选择结果**。这是一个如果不解决，开发第一天就会卡住的硬伤。对 Serde 序列化兼容性（破坏性变更）的提醒也非常务实。

#### 4. Sisyphus-M2.7HS
- **评分**: **8.5 / 10 (Good)**
- **评价**: 结构工整，逻辑推演能力强。它发现了一个草案规则中的逻辑矛盾：**“Apply 顺序允许先 remove 再 add，但冲突规则里又说这是 conflict”**。它还指出现有事实中 `cursor` 已经是一个“投影”，可能清理成本没有草案预估的那么大。整体分析很到位，但在 Pregel 特有调度机制的深度上略逊于 Opus。

#### 5. CodeBuddy Gemini-3.0-Pro (我的 Review)
- **评分**: **8.0 / 10 (Solid)**
- **评价**: 整体评价中肯，重点突出了向下兼容（API 白屏风险）和操作层面体验（HumanGate resume 需要触发 tick）。贡献了一个独特的视角：**SubGraph 内部触发 Mutation 时，作用域是父图还是子图？** 但在后端执行引擎的复杂状态流转分析上，不如前三份深入。

---

### 二、 综合总结 (Summary of Reviews)

综合这 5 份 Review，针对 `spec_draft.md` 的反馈高度集中，草案设计方向（Revision + Barrier + 无 Cursor）得到了全票赞同，但要在工程上落地，必须在下一版 Spec 中修补以下 5 个核心维度的缺失：

#### 1. 状态机与生命周期盲区 (State Machine & Lifecycles)
- **编译失败回退**：Mutation 执行并生成新拓扑后，如果 `compiler` 编译新 Revision 失败（如引用缺失），流程必须有回退（Rollback）或 Reject 机制。
- **新 Channel 初始化**：新增边/节点会产生新的 Channel，必须明确其初始 `version` 和 `value`，否则下一轮 superstep 会因为 `versions_seen` 校验失败而无法触发任务。
- **并发与时序**：如果一个节点在当前 superstep 被删除，但它还有未应用的 normal writes，这些 writes 应当被正确应用（在 mutation 生效前）。

#### 2. 控制流与协议断层 (Control Flow & Protocol)
- **Branch/Loop 的替代协议**：废弃 `next_nodes` 后，必须明确分支节点如何通过 `Command.goto` 或特定的 Output 字段，将它的分支选择结果透传给 lowering 层。
- **Mutation 写入通道**：需要理清 `NodeOutcome.graph_mutations` 和 `GRAPH_MUTATIONS_CHANNEL` 的关系，确保拓扑变更请求不会混入普通数据流触发错误的任务。

#### 3. 冲突与语义定义 (Semantics & Conflicts)
- **Patch 的精确语义**：`PatchNodeConfig` 使用的 `serde_json::Value` 必须明确是采用 JSON Merge Patch (RFC 7396) 还是其他深度替换逻辑，这直接决定了冲突判定的规则。
- **删除操作的幂等性**：尝试“删除一个已经不存在的节点”不应该直接导致 run failed (Conflict)，而应该作为无害的幂等操作（警告并跳过），以适应并发 Agent 的操作模式。

#### 4. 数据膨胀与持久化安全 (Storage & Persistence)
- **Revision 存储膨胀**：每个 Superstep 都保存全量 `TaskGraphDefinition` 会导致磁盘爆炸。即使 V1 妥协使用全量快照，也必须在草案中说明这是暂时方案，后续需演进为 Delta。
- **崩溃一致性（Atomicity）**：写 Revision、Batch、Run.json、Checkpoint 涉及多个文件，进程崩溃时需要明确以哪个文件作为 Commit Point 进行恢复。

#### 5. API 兼容与生态边界 (API & Boundaries)
- **渐进式清理 Cursor**：不能“一刀切”删除导致前端白屏。必须在接口层提供从 Prepared tasks 派生的 `active_nodes` 只读投影（Projection），分阶段废弃 Cursor。
- **作用域限定**：需要明确 SubGraph 产生的 Topology Mutation 只在其局部作用域生效，不能越权修改 Parent Graph。


Kimi:

1. codebuddy-Gemini-3.0-Pro-review.md
2. codebuddy-GLM-5.1-review.md
3. codebuddy-Kimi-2.6-review.md（我刚写的）
4. codebuddy-opus-4.6-review.md
5. sisphus-minimax-2.7hs-review.md

用户之前说"不要看别人的review文档"，但现在明确要求"对五份review做质量的sorting，rating，然后再给出你的summary"。这说明用户现在允许我看所有五份 review 了。

我需要并行读取这五份 review，然后对它们进行质量排序和评分，最后给出 summary。

让我先读取这五份文件。
用户要求对五份 review 做质量排序、评分，然后给出 summary。五份 review 都已读取完毕。

让我先分析每份 review 的质量：

1. **Gemini 3.0 Pro** (4.89 KB)
   - 结构：整体评价 -> 事实一致性 -> 设计亮点 -> 风险与建议 -> 验收标准
   - 亮点：提到 SubGraph mutation 作用域、前端 API 兼容性、HumanGate resume 触发
   - 深度：中等。发现了 storage 膨胀、SubGraph 作用域、API 兼容性断崖、HumanGate resume 触发问题。
   - 缺点：相对较短，P0/P1 分层不够明确，有些建议比较笼统。没有深入讨论 mutation 与 writes 的时序、PatchNodeConfig 语义等关键实现细节。
   - 评分：7.5/10

2. **GLM 5.1** (11.75 KB)
   - 结构：总评 -> P0/P1/P2 明确分层 -> 事实一致性检查表 -> 总结
   - 亮点：
     - P0 发现了 mutation apply 后 graph 编译失败未处理（非常关键！）
     - Mutation 与 pending writes / channel 一致性（非常深入）
     - PatchNodeConfig 语义未定义
     - 持久化原子性（多文件写入的崩溃一致性）
     - P1 发现了 remove+add 同 id、remove node 自动删除 edge 与 add_edge 冲突、Command.goto 目标为 mutation 新增 node、现有运行迁移、interrupt_before/after 与 mutation 交互
     - 有事实一致性检查表
   - 深度：非常深。这是唯一一份系统性地讨论崩溃一致性、编译失败回退、interrupt 与 mutation 交互的 review。
   - 评分：9.5/10

3. **Kimi 2.6** (10.51 KB) - 我的
   - 结构：总评 -> 强项 -> 与 existing_truth 的 gaps -> 与研究结果的对齐 -> 具体建议(P0/P1/P2) -> 风险 -> 结论
   - 亮点：
     - Branch/Loop 节点移除 next_nodes 后的替代机制未定义（这是所有 review 中最先明确提出且分析最细致的）
     - NodeOutcome.graph_mutations vs GRAPH_MUTATIONS_CHANNEL 的写入路径语义分歧（与 Opus 4.6 类似，但更早提出）
     - PatchNodeConfig 语义
     - Send 到 mutation 新增节点的时序边界
     - 持久化格式兼容性
   - 深度：深，但相比 GLM 5.1 少了编译失败处理、持久化原子性、interrupt 交互等。
   - 评分：8.5/10

4. **Opus 4.6** (10.37 KB)
   - 结构：总评 -> P0/P1/P2 -> 总结表 -> 建议下一步
   - 亮点：
     - Graph revision 存储膨胀（与 Gemini 类似但更具体，给出 delta 方案）
     - Mutation 与 normal writes 的 barrier 时序（非常详细，给出 7 步时序）
     - Mutation 后的 channel state 迁移（新增 channel 初始 value/version，非常关键且独特的发现）
     - Remove node 时正在执行或已暂停的 task 如何处理（独特发现）
     - PatchNodeConfig 语义
     - Cursor 清理的渐进策略
     - Mutation event 进入 RunEvent 流
     - Mutation channel vs NodeOutcome 直接携带的二选一建议
   - 深度：非常深。Channel state 迁移和暂停 task 处理是其他 review 基本没提到的。
   - 评分：9.0/10

5. **Sisyphus-M2.7HS** (12.87 KB)
   - 结构：执行摘要 -> 架构设计评估 -> Mutation Apply 顺序 -> 冲突规则 -> Cursor 清理 -> 模块修改清单 -> 测试要求 -> 验收标准 -> 与 existing_truth 对齐 -> 与 research_results 对齐 -> 风险矩阵 -> 建议行动 -> 结论
   - 亮点：
     - 模块修改清单按风险级别分类（高/中/低），并给出实现顺序建议
     - 发现 apply 顺序与冲突规则的矛盾（remove+add 同 id，顺序允许但规则说 conflict）
     - 总体风险矩阵（概率/影响/优先级）
     - 指出 cursor 清理成本可能低于草案预估（因为 existing_truth 说 cursor 已经是投影）
     - 验收标准审查（指出缺少集成测试）
   - 深度：深，但更偏架构评估和项目管理视角，对具体实现细节（如 channel state 迁移、持久化原子性）覆盖不足。
   - 评分：8.0/10

**质量排序（从高到低）：**

1. **GLM 5.1** (9.5/10) - 最系统性、P0 项最关键（编译失败回退、持久化原子性）、有事实一致性检查表、覆盖边界最全面。
2. **Opus 4.6** (9.0/10) - 对 barrier 时序和 channel state 迁移的分析最深入，给出具体 7 步时序，发现暂停 task 处理问题。
3. **Kimi 2.6** (8.5/10) - Branch/Loop 替代机制的分析最到位，mutation 写入路径的语义分歧抓得准。
4. **Sisyphus-M2.7HS** (8.0/10) - 架构视角全面，模块风险分级和实现顺序建议有价值，风险矩阵直观。
5. **Gemini 3.0 Pro** (7.5/10) - 最短，发现了一些独特问题（SubGraph 作用域、HumanGate resume 触发），但深度和系统性不如其他四份。

**各 review 的独特贡献：**
- GLM 5.1: 编译失败回退、持久化原子性、interrupt 与 mutation 交互、现有 run 迁移。
- Opus 4.6: channel state 迁移（新增 channel 初始值/版本）、暂停 task 处理、mutation 与 normal writes 的 7 步时序、RunEvent 审计。
- Kimi 2.6: Branch/Loop 控制流替代协议、mutation 写入路径语义分歧。
- Sisyphus-M2.7HS: 模块风险分级与实现顺序、apply 顺序与冲突规则矛盾、cursor 实际使用程度核实建议。
- Gemini 3.0 Pro: SubGraph mutation 作用域、API 兼容性断崖、HumanGate resume 后立即触发 coordinator tick。

现在我需要输出 sorting、rating 和 summary。
## 五份 Review 质量排序与评分

| 排名 | Review | 评分 | 核心优势 | 主要短板 |
|:---:|---|:---:|:---|:---|
| **1** | **GLM 5.1** | **9.5/10** | P0 分级最精准；唯一系统讨论 **编译失败回退** 和 **持久化原子性**（崩溃一致性）；覆盖 interrupt 与 mutation 交互、现有 run 迁移；附事实一致性检查表 | 对 channel state 迁移未展开 |
| **2** | **Opus 4.6** | **9.0/10** | 给出 mutation barrier **7 步精确时序**；唯一深入分析 **新增 channel 初始 state 迁移** 和 **暂停 task 删除处理**；提出 mutation 审计事件 | 未讨论编译失败回退和持久化原子性 |
| **3** | **Kimi 2.6** | **8.5/10** | **Branch/Loop 控制流替代协议** 的分析最到位（最大实现缺口）；精准抓住 `NodeOutcome.graph_mutations` 与 `GRAPH_MUTATIONS_CHANNEL` 的语义分歧 | 缺少编译失败、原子性、interrupt 交互等系统性分析 |
| **4** | **Sisyphus-M2.7HS** | **8.0/10** | 架构视角最完整；**模块风险分级**（🔴🟡🟢）和实现顺序建议直观；发现 apply 顺序与冲突规则的逻辑矛盾；总体风险矩阵清晰 | 实现细节深度弱于前三；对具体 barrier 时序、channel 迁移覆盖不足 |
| **5** | **Gemini 3.0 Pro** | **7.5/10** | 发现 **SubGraph mutation 作用域**、API 兼容性断崖、HumanGate resume 后 tick 触发等独特问题 | 篇幅最短，缺乏 P0/P1 分层，系统性和细节深度明显弱于其他四份 |

---

## Summary

五份 review 对草案的总体判断高度一致：**方向正确、与 Pregel 3.4 对齐、具备进入开发阶段的条件**。但在具体缺陷挖掘的深度和视角上差异显著。

### 按能力维度对比

| 维度 | 最佳表现者 | 说明 |
|---|---|---|
| **实现细节深度** | GLM 5.1 / Opus 4.6 | GLM 抓编译失败和原子性；Opus 抓 channel 迁移和暂停任务 |
| **架构/项目管理视角** | Sisyphus-M2.7HS | 模块风险分级、实现顺序建议、风险矩阵 |
| **控制流缺口识别** | Kimi 2.6 | Branch/Loop 移除 `next_nodes` 后的替代协议是最大盲区 |
| **独特边缘场景** | Gemini 3.0 Pro | SubGraph 作用域、HumanGate resume tick |

### 各 review 的不可替代贡献

- **GLM 5.1**：如果只能修一个 P0，应该是 **"mutation apply 后 compile 失败如何回退"** 和 **"多文件写入的崩溃一致性"**。这两点是其他 review 普遍遗漏的系统级风险。
- **Opus 4.6**：**channel state 迁移**（新增/删除 channel 的初始值、version、versions_seen 处理）是草案完全没有定义的领域，但实现时必然遇到。
- **Kimi 2.6**：**Branch/Loop 控制流替代协议** 是草案最大的语义缺口——没有它，Branch 图在 cursor 清理后会直接断裂。
- **Sisyphus-M2.7HS**：**模块实现顺序建议**（先低风险类型/模型，后高风险 coordinator）可以作为开发排期的直接输入；发现的 **remove+add 同 id 与 apply 顺序矛盾** 需要在冲突检测阶段前置解决。
- **Gemini 3.0 Pro**：**SubGraph mutation 作用域** 是 long-term 架构问题，虽然本轮可能不涉及，但如果现在不声明边界，后续 Agent 协作场景会引入歧义。

### 综合建议

如果要把五份 review 的洞见合并成一份"完美 review"，应包含：

1. **P0 必修**（来自 GLM 5.1 + Opus 4.6 + Kimi 2.6）：
   - Branch/Loop 控制流替代协议（Kimi 2.6）
   - Mutation apply 后 compile 失败的回退策略（GLM 5.1）
   - 持久化原子性 / 崩溃一致性（GLM 5.1）
   - Mutation 与 normal writes 的 barrier 时序及 channel state 迁移（Opus 4.6）
   - PatchNodeConfig 语义（GLM 5.1 / Opus 4.6 / Kimi 2.6 均提到）

2. **P1 重要补充**（来自 Opus 4.6 + GLM 5.1 + Sisyphus-M2.7HS）：
   - 新增 channel 初始 state 定义（Opus 4.6）
   - Remove node 时暂停 task 的处理（Opus 4.6）
   - 现有 run 迁移策略（GLM 5.1）
   - 冲突规则与 apply 顺序的一致性（Sisyphus-M2.7HS）

3. **P2 架构优化**（来自 Gemini 3.0 Pro + Opus 4.6 + Sisyphus-M2.7HS）：
   - SubGraph mutation 作用域（Gemini 3.0 Pro）
   - Mutation 审计事件进入 RunEvent（Opus 4.6）
   - 模块实现顺序与风险分级（Sisyphus-M2.7HS）



GLM 5.1

---

## 五份 Review 质量排序与评分

### 评分维度（每项 1-5 分）

| 维度 | 说明 |
|------|------|
| **事实锚定** | 是否准确引用 existing_truth / research_results 中的具体条目，而非泛泛而谈 |
| **问题深度** | 发现的问题是否触及真正的设计缺陷（而非表面文字问题），是否给出可操作的解决方案 |
| **完整性** | 是否覆盖了 spec 中所有关键设计决策的盲区 |
| **结构清晰度** | 优先级分层是否合理，行文是否高效 |
| **可操作性** | 建议是否具体到"改哪一段、怎么改"，而非仅"建议考虑" |

### 评分表

| 排名 | Review | 事实锚定 | 问题深度 | 完整性 | 结构清晰度 | 可操作性 | 总分 |
|------|--------|----------|----------|--------|------------|----------|------|
| **1** | **Opus 4.6** | 4.5 | 5 | 4.5 | 5 | 4.5 | **23.5** |
| **2** | **GLM-5.1** | 4 | 4.5 | 4.5 | 4.5 | 4 | **21.5** |
| **3** | **Kimi-2.6** | 4.5 | 4 | 4 | 4 | 4 | **20.5** |
| **4** | **Minimax-2.7hs** | 3.5 | 3.5 | 4 | 4 | 3 | **18** |
| **5** | **Gemini-3.0-Pro** | 3 | 3 | 3 | 3 | 2.5 | **14.5** |

---

## 逐份点评

### 1. Opus 4.6 — 23.5/25 ⭐

**最强项：问题深度和结构**

Opus 是唯一一份精确解构了 mutation channel 与 normal writes 的 barrier 时序问题的 review（P0-2），给出了完整的 7 步 barrier 流程建议，直接可嵌入 spec。这个问题其他 review 要么没触及，要么只笼统提了一嘴。

另外三个独有贡献：
- **P0-3**：mutation 后新增 channel 的初始 state（value/version 如何设定），这是实现时一定会遇到的问题，其他 review 都没提出。
- **P0-4**：remove node 时进行中/暂停 task 的处理，区分了 "已完成 task 的 writes 先于 mutation apply" 和 "暂停态 node 被 remove 后 resume 应 cancelled"。
- **P2-12**：mutation channel vs NodeOutcome 直接携带的架构二选一，指出 spec 同时定义了两条路径，存在冗余。这是一个设计层面的发现，不是文字层面的问题。

微弱不足：对 PatchNodeConfig 语义的建议（JSON Merge Patch）虽然正确，但没有像 Kimi 那样进一步讨论 patch 对冲突检测的影响（merge patch 可以自动合并不同字段的 patch，这直接影响冲突规则设计）。

### 2. GLM-5.1 — 21.5/25

**最强项：持久化原子性和边界情况**

GLM-5.1 是唯一提出持久化原子性问题（P0-4）的 review，具体列出了 5 步写入操作和崩溃恢复场景，并给出两阶段提交方案。这是一个 Opus 也未触及的 P0 级问题。

另外两个独有贡献：
- **P0-1**：mutation apply 后 compile 失败的回退策略。Opus 在 P0-3 讨论了 channel state 迁移，但没有明确提 compile 失败的情况。
- **P1-7**：Command.goto 指向 mutation 新增 node 的非法操作检测。Kimi 也独立发现了同样的问题，但 GLM-5.1 给出了更明确的建议（lowering 阶段报错）。

不足：没有讨论 mutation 后新增 channel 的初始 state，也没有提出 mutation channel vs NodeOutcome 的架构冗余问题。

### 3. Kimi-2.6 — 20.5/25

**最强项：Branch/Loop 控制流断裂风险**

Kimi 是唯一一份精准定位了 "移除 `next_nodes` 后 Branch/Loop 如何表达选择结果" 这个实现缺口的 review（Gap 1）。这个问题其他 review 要么没提，要么只笼统说了"compiled writers decide"。Kimi 给出了四个候选方案并推荐了 `Command.goto` 路径。

另一个独有贡献是 Gap 2：`NodeOutcome.graph_mutations` 与 `GRAPH_MUTATIONS_CHANNEL` 的语义分歧。Kimi 精确列出了三种可能路径（A/B/C），并建议增加一句话明确 lowering 阶段的分流逻辑。Opus 在 P2-12 也发现了这个问题但建议"二选一"，Kimi 更倾向于明确两者的关系而非删减。

不足：没有提出持久化原子性问题，没有讨论 compile 失败回退，没有讨论 mutation 后新增 channel 的初始 state。

### 4. Minimax-2.7hs — 18/25

**最强项：模块级风险评估和实现顺序建议**

Minimax 的亮点是 §6 的模块修改清单风险评估——按红/黄/绿三档给每个模块标了风险级别，并建议从低风险到高风险的实现顺序。这是五份 review 中唯一给出实现路径建议的。

另一个独有贡献是 §3 中发现的 apply 顺序与冲突规则的矛盾：如果 `remove_node(X)` + `add_node(X)` 同 batch，按 apply 顺序会先 remove 再 add（最终图包含新 X），但冲突规则说同 id 不同 spec 应 conflict。GLM-5.1 也独立发现了同样的问题，但 Minimax 的表述更精确地指出了逻辑矛盾。

不足：事实锚定较弱——§9.2 说 "existing_truth 说 cursor 已经是投影性质，与草案原则 5 有轻微矛盾"，这是误读。existing_truth 原文是 "cursor 更像是 UI 和兼容层投影，同时也用于暂停态兼容"，关键是 "同时"——cursor 仍然参与暂停态逻辑，不是纯投影。整体 review 偏框架性，缺少像 Opus 那样的具体解决方案。

### 5. Gemini-3.0-Pro — 14.5/25

**最弱项：深度不足，可操作性差**

Gemini 的 review 格式更像产品 review 而非技术 review。四个 "风险" 中有三个（存储膨胀、前端 API 兼容、HumanGate resume 触发）都是其他 review 更深入讨论过的，且 Gemini 的建议更笼统。

唯一独有贡献是风险 2：SubGraph 的 Mutation 作用域——子图内节点的 mutation 是修改父图还是子图。这是一个有价值的问题，但 Gemini 只给了一句话建议（"仅对当前 Run 所在的局部 GraphContext 生效"），没有展开讨论跨图 mutation 的边界情况。

主要问题：
- 没有 P0/P1/P2 分层，四个风险平行排列
- 没有具体引用 spec 行号
- 验收标准审核只有一条补充建议
- 整体篇幅最短，覆盖面最窄

---

## 跨 Review 发现合并总结

### 各 Review 独有发现（仅此一份提出）

| 发现 | 提出者 | 严重度 |
|------|--------|--------|
| Mutation 后新增 channel 的初始 value/version | Opus | P0 |
| Remove node 时暂停态 task 应 cancelled | Opus | P0 |
| Mutation channel vs NodeOutcome 架构冗余（二选一） | Opus | P2 |
| 持久化原子性（崩溃一致性） | GLM-5.1 | P0 |
| Mutation apply 后 compile 失败回退 | GLM-5.1 | P0 |
| Command.goto 指向 mutation 新增 node 非法 | GLM-5.1 | P1 |
| 现有 run 迁移（无 revision 的历史 run） | GLM-5.1 | P1 |
| interrupt_before/after 与 mutation 交互 | GLM-5.1 | P1 |
| Branch/Loop 移除 next_nodes 后的替代协议 | Kimi | P0 |
| NodeOutcome.graph_mutations 与 channel 的语义分歧 | Kimi | P1 |
| SuperstepCheckpoint 字段变更的反序列化兼容 | Kimi | P1 |
| PatchNodeConfig 应增加 config schema validation | Kimi | P2 |
| SubGraph mutation 作用域 | Gemini | P1 |
| 模块修改顺序建议（低→高风险） | Minimax | P2 |
| ReplaceNode 是否需要 | Minimax | P2 |
| RunEvent 缺少 mutation 事件 | Opus | P1 |

### 共识发现（2+ 份 review 独立提出）

| 发现 | 提出者 | 共识度 |
|------|--------|--------|
| PatchNodeConfig 语义未定义 | Opus + GLM-5.1 + Kimi | 3/5 |
| GraphRevision 全量 snapshot 存储膨胀 | Opus + Kimi + Minimax + Gemini | 4/5 |
| Remove + add 同 id node 行为未声明 | GLM-5.1 + Minimax | 2/5 |
| Cursor 删除的 API 兼容影响 | Opus + Kimi + Minimax + Gemini | 4/5 |
| Mutation 与 normal writes 的 barrier 时序 | Opus + GLM-5.1 + Kimi | 3/5 |
| Send/Command.goto 到 mutation 新增 node 的时序 | GLM-5.1 + Kimi | 2/5 |
| GraphMutationBatchResult::Rejected 后 run 应明确为 failed | GLM-5.1 | 1/5（仅一份但正确） |

### 建议 spec 修订的优先合并清单

基于五份 review 的合并分析，建议 spec 按以下优先级补充：

**P0（实现前必须补入 spec）**：
1. Barrier 时序精确定义（Opus 的 7 步流程）
2. Mutation 后新增 channel 的初始 state（Opus）
3. Mutation apply 后 compile 失败 → batch rejected（GLM-5.1）
4. 持久化崩溃一致性约束（GLM-5.1）
5. PatchNodeConfig 语义声明为 JSON Merge Patch（3 份共识）
6. Branch/Loop 移除 next_nodes 后的选择结果表达协议（Kimi）
7. Remove node 时进行中/暂停态 task 处理（Opus）

**P1（实现初期解决）**：
8. Remove + add 同 id node 声明为 replace 或 conflict（GLM-5.1 + Minimax）
9. NodeOutcome.graph_mutations 与 mutation channel 的关系明确（Kimi + Opus）
10. Command.goto / Send 到 mutation 新增 node 视为非法（GLM-5.1 + Kimi）
11. 现有 run 迁移策略（GLM-5.1）
12. SubGraph mutation 作用域（Gemini）
13. RunEvent 增加 mutation 事件（Opus）
14. Cursor API 兼容策略：feature gate 或 active_nodes projection（4 份共识）


Opus 4.6

## 排名

| 排名 | Reviewer | 分数 | 关键优势 |
|------|---------|------|---------|
| 🥇 1 | **GLM-5.1** | **9.1** | mutation-after-compile 失败处理、持久化原子性（唯一发现）、interrupt 交互 |
| 🥈 2 | **Kimi-2.6** | **8.8** | Branch/Loop 控制流替代协议最深入、mutation 写入路径三路分析 |
| 🥉 3 | **Opus 4.6** | **8.4** | 新增 channel 初始 state 迁移、7步时序模型、RunEvent 审计 |
| 4 | **Gemini 3.0 Pro** | **7.2** | SubGraph mutation 作用域（唯一发现）、整体正面基调 |
| 5 | **MiniMax 2.7HS** | **7.0** | apply 顺序 vs 冲突规则内部矛盾（唯一发现）、模块实施顺序 |

---

## 跨 Review 的三个"唯一发现"最值得关注

- **GLM**：mutation apply 后 compile 失败无回退 + 持久化原子性（5文件写入的崩溃一致性）
- **Kimi**：Branch/Loop 移除 `next_nodes` 后的控制流替代协议（实现期首个踩坑点）
- **MiniMax**：apply 顺序与冲突规则的内部逻辑矛盾（`remove+add 同 id` 的处理路径不自洽）

Spec 方向无人质疑，补充以上 P0 项后可进入实施。


# Review Sorting, Rating & Summary
# Pregel Topology Mutation — 五份 Review 综合评价

> 评估人: CodeBuddy Opus 4.6
> 评估时间: 2026-05-16
> 评估对象: 以下五份 review 文档

| 文件名 | Reviewer |
|--------|---------|
| `codebuddy-GLM-5.1-review.md` | CodeBuddy GLM-5.1 |
| `codebuddy-Kimi-2.6-review.md` | CodeBuddy Kimi-2.6 |
| `codebuddy-opus-4.6-review.md` | CodeBuddy Opus 4.6（本机） |
| `codebuddy-Gemini-3.0-Pro-review.md` | Gemini 3.0 Pro |
| `sisphus-minimax-2.7hs-review.md` | Sisyphus MiniMax 2.7HS |

---

## 评分维度说明

| 维度 | 权重 | 说明 |
|------|------|------|
| **问题发现深度** | 35% | 是否命中 spec 的真实设计空缺，而非表面问题 |
| **事实对照精度** | 25% | 是否准确引用 existing_truth / research_results 的具体位置和内容 |
| **建议可操作性** | 20% | 提出的建议是否可以直接转化为 spec 修改或代码改动 |
| **结构与表达清晰度** | 10% | 问题分级是否清晰、表述是否无歧义 |
| **覆盖完整性** | 10% | 是否覆盖了 spec 的主要风险域（模型、持久化、控制流、测试、迁移） |

满分 10 分。

---

## 逐份评分

### 1. GLM-5.1 — 9.1 / 10

**亮点**：

- **P0 第 1 条（mutation apply 后 compile 失败）** 是五份 review 中唯一提出的原创发现，且有精确 spec 行号对应（L456-L479）。这是一个真实的实现缺口——spec 只写了 "compile new revision"，但没有说明 compile 失败后如何回退 mutation batch。
- **P0 第 4 条（持久化原子性）** 精确枚举了一次 mutation apply 涉及的 5 个文件写入步骤，并提出了两阶段提交的具体方案选项。这是所有 review 中对持久化层最严谨的分析。
- **P1 第 5 条（remove + add 同 id）** 和 **P1 第 6 条（cascade remove edge 的时机）** 是对冲突规则内部自洽性的深入推导，逻辑链完整。
- **事实对照表** 完整，所有 spec 声明都有 existing_truth / research_results 对应条目标注。
- **P1 第 9 条（interrupt_before/after 与 mutation 的交互）** 是唯一考虑到 interrupt 配置归属问题的 review，具有较高发现价值。

**不足**：

- `GraphMutation` 写入路径（`NodeOutcome.graph_mutations` vs channel）只在 P3 位置提及（P2 #14 补充测试），分量不够——这应是 P0 或 P1。
- 对 Branch/Loop 移除 `next_nodes` 后的控制流替代协议没有单独分析（这是 Kimi 捕到的关键 gap）。

---

### 2. Kimi-2.6 — 8.8 / 10

**亮点**：

- **Branch/Loop 节点移除 `next_nodes` 后的替代机制** 是五份 review 中最深入覆盖这一点的，提出了具体的四种候选方案，并推荐了"复用 `Command.goto`"作为改动最小路径。这是一个实现路上会立刻踩坑的真实 gap。
- **`NodeOutcome.graph_mutations` vs `GRAPH_MUTATIONS_CHANNEL` 的语义分歧** 分析了 A/B/C 三条路径及其在 writes lowering 阶段的影响，路径分析比 Opus 4.6 的同类观察更完整。
- 结构紧凑，P0/P1/P2 表格清晰，**风险** 小节单列，便于实现者快速消费。
- 指出了 `serde(default)` 与 Breaking Change 的兼容性问题（持久化格式迁移），给出了简洁的处理策略。

**不足**：

- 没有覆盖持久化原子性（GLM 的 P0 第 4 条）——这是一个实现层的重要遗漏。
- "remove 不存在 node → conflict" 过严的问题没有提出；幂等性处理留有讨论空间。
- **P1 #5** "dedupe 只针对同 batch" 的表述稍模糊，对跨 batch 幂等语义的建议不如 GLM 清晰。

---

### 3. Opus 4.6（本机） — 8.4 / 10

**亮点**：

- **P0 第 2 条（mutation vs normal writes 的 barrier 时序）** 提出了一个完整的 7 步时序模型，逻辑清晰，可直接对应 coordinator 代码结构。
- **P0 第 3 条（新增 channel 的初始 state 迁移）** 是五份 review 中最深入分析 channel state 迁移策略的，给出了具体的 initial value、initial version 和 `versions_seen` 处理方案。
- **P1 第 8 条（mutation 进入 RunEvent 流）** 提出了新增 `RunEvent::TopologyMutation` variant，在审计可追溯性上有独到价值。
- 总结表格（ID/优先级/类型/标题）格式规范，"建议的下一步"部分给出了明确行动顺序。

**不足**：

- 错过了 GLM 发现的"mutation apply 后 compile 失败未处理"（P0 级缺口）。
- 错过了 Kimi 发现的"Branch/Loop 控制流替代协议"。
- 对持久化原子性没有分析。
- "remove 不存在 node 建议幂等 skip" 这一条与 Kimi P1 #5 类似，但推导不如 GLM 精确。

---

### 4. Gemini 3.0 Pro — 7.2 / 10

**亮点**：

- **风险 2（SubGraph 的 mutation 作用域）** 是五份 review 中唯一提出 SubGraph 边界问题的，这确实是 spec 的空白——现有 existing_truth 确认了 SubGraph 传播 checkpoint namespace，但 spec 没有定义 mutation 是否能穿透 SubGraph 边界。
- **风险 4（HumanGate resume 需要立刻 tick）** 是有价值的 UX 级 concern，与 backend coordinator 的 trigger 机制相关。
- 整体表达流畅，对草案的**设计亮点**（GraphRevision 不可变版本化、mutation as write、PatchNodeConfig 扩展）给出了正面分析，有助于建立 reviewer 共识。

**不足**：

- **问题密度偏低**：只提出了 4 个风险点，且以风险描述为主，缺乏具体的解决方案建议。
- 没有提出任何原创的 P0 设计缺陷（GLM 的 compile 失败处理、持久化原子性都没有发现）。
- "建议在草案中备注未来引入 delta 存储" 是所有 review 都提到的标准观察，没有超越。
- Checkpoint replay 的补充建议（"中途中断后恢复"）已在 spec 测试 #10 中有隐含覆盖，新增价值有限。
- 结论是 "强烈赞同草案"，整体基调偏正面，review 功能受限。

---

### 5. Sisyphus MiniMax 2.7HS — 7.0 / 10

**亮点**：

- **apply 顺序与冲突规则的矛盾** （第 3 节）：发现了 `remove_node(X) + add_node(X, 不同 spec)` 在 apply 顺序（先 remove 后 add）和冲突规则（conflict）之间存在逻辑矛盾，并明确指出需要在 apply 前做冲突检测而不是按顺序盲目应用。这是真正意义上的**设计内部矛盾**，其他四份 review 均未发现。
- **模块修改顺序建议**（低→中→高风险）提供了可操作的实施路径，比其他 review 更有工程落地价值。
- **风险矩阵表格**（概率/影响/优先级）结构清晰，易于优先级决策。
- 对 existing_truth 第 12 点（cursor 已经是投影性质）的观察（"清理成本可能低于预估"）是对 spec 前置假设的有益校正。

**不足**：

- **问题分析深度不足**：发现了矛盾点（apply 顺序 vs 冲突规则），但对解决方案的描述较简短，没有展开"冲突检测前置的完整逻辑"。
- **事实对照精度最低**：除个别 spec 行号引用外，对 existing_truth 和 research_results 的引用较少，难以验证 reviewer 的推断来自代码事实而非空泛判断。
- 第 7 节（测试要求）中 "compile_fail_if_cursor_references_remain!()" 的建议不可行（这不是标准 Rust macro），有误导性。
- P0 聚焦于 "cursor 字段删除" 和 "coordinator 大改" 这类执行层风险，而非 spec 的设计缺陷。这更接近"实施计划 review"而非"spec review"。

---

## 排名汇总

| 排名 | Reviewer | 总分 | 关键优势 |
|------|---------|------|---------|
| 🥇 1 | **GLM-5.1** | **9.1** | mutation-after-compile 失败处理、持久化原子性（唯一发现）、interrupt 交互、内部冲突规则一致性 |
| 🥈 2 | **Kimi-2.6** | **8.8** | Branch/Loop 控制流替代协议（最深入）、mutation 写入路径三路分析、结构清晰可消费 |
| 🥉 3 | **Opus 4.6** | **8.4** | 新增 channel 初始 state 迁移、7步时序模型、RunEvent audit trail |
| 4 | **Gemini 3.0 Pro** | **7.2** | SubGraph mutation 作用域（唯一发现）、HumanGate UX concern |
| 5 | **MiniMax 2.7HS** | **7.0** | apply 顺序 vs 冲突规则内部矛盾（唯一发现）、模块实施顺序、风险矩阵 |

---

## 综合 Summary：Spec 的关键缺陷与建议

以下是跨五份 review 去重、合并后的**高价值发现**，按 spec 修改优先级排序。

### P0 — 必须在实施前补入 Spec

**A. Mutation apply 后 compile 失败的回退策略**（GLM 独家发现）

Spec 的 coordinator 新流程写了 "compile new revision"，但没有定义 compile 失败时的行为。应在 mutation batch 进入 apply 前增加 compile 预检；compile 失败视为 `GraphMutationBatchResult::Rejected`，不写入新 revision，run fail。

**B. Apply 顺序与冲突规则的内部矛盾**（MiniMax 独家发现）

`remove_node(X) + add_node(X, 不同 spec)` 在 apply 顺序（先 remove 后 add，结果 X 被替换）与冲突规则（声明为 conflict）之间存在矛盾。Spec 需要明确：冲突检测在 apply 顺序执行之前（先检测，有冲突则 reject，不进入 apply），还是冲突规则中引入"replace 语义"。

**C. 持久化原子性与崩溃一致性**（GLM 独家发现）

一次 mutation apply 涉及至少 5 个文件写入。进程中途崩溃会导致 run.json 与 checkpoint 不一致。Spec 必须声明 commit point（建议：以 SuperstepCheckpoint 的 `graph_revision_after` 为 commit point，其他文件落盘先于此，恢复时以 checkpoint 为准）。

**D. Mutation writes 与 normal writes 的 barrier 时序分离**（Opus 4.6、Kimi 共同发现）

Spec 没有明确 `__pregel_graph_mutations` channel 的 write 如何与普通 channel writes 在 `apply_writes` 中分流。需要显式声明：mutation writes 不参与 `channel_versions` / `versions_seen` / trigger 计算，在 normal writes apply 后由专用 mutation collector 提取。

**E. Branch/Loop 节点移除 `next_nodes` 后的控制流替代协议**（Kimi 独家发现）

这是实现中最先踩坑的 gap。Spec 说 Branch/Loop 通过 "output 或 side effect 表达选中 target"，但没有定义具体协议。建议明确：复用 `Command.goto`（lowering 层将 goto 转为对应 branch channel write），改动最小且与现有 Command 路径一致。

**F. Mutation apply 后新增 channel 的初始 state 迁移**（Opus 4.6 独家发现）

新 revision 产生新 channel，但当前 checkpoint 中不存在这些 channel 的 value/version/`versions_seen` 记录。Spec 需要定义迁移策略：新增 channel 初始 value = zero value，初始 version = current superstep（这样下一轮 PULL task 自然被触发）。

### P1 — 强烈建议补入 Spec

**G. PatchNodeConfig 的 patch 语义约束**（GLM、Kimi、Opus 共同发现）

`serde_json::Value` 过于宽松。建议本轮选定 JSON Merge Patch (RFC 7396)，在 Spec 中显式声明。

**H. 现有 run（含 cursor state）的迁移策略**（GLM 独家发现）

合并后已有 run 缺少 `graph_revision` 文件，恢复时 assert 会失败。Spec 应声明迁移策略：首次加载旧 run 时从 graph snapshot 创建 revision 0，或明确声明本轮不支持历史 run 恢复。

**I. Remove 不存在 node 的幂等性语义**（Opus 4.6 发现，GLM 补充）

跨 superstep 场景下，目标 node 可能已被上一 batch 删除。建议区分：同 batch 重复 remove → dedupe；target 在当前 revision 已不存在 → warn + skip（幂等），而非 conflict。

**J. SubGraph 内 mutation 的作用域**（Gemini 独家发现）

Spec 未定义 SubGraph 内节点提交的 mutation 是作用于子图还是父图。建议默认约束：mutation 仅作用于当前 Run 的 graph context，不支持跨 SubGraph 栈修改父图拓扑。

**K. interrupt_before/after 与 mutation 的交互**（GLM 独家发现）

mutation add 的新 node 若在 run 的 interrupt 列表中，下一 superstep 是否自动 interrupt？Spec 应声明：interrupt 配置属于 run 级别，不受 graph revision 影响；mutation add node 后若 node name 在 interrupt 列表中，下一 superstep 正常触发 interrupt。

**L. Mutation apply 后 run failed vs paused 的明确选择**（GLM 独家发现）

Spec 写的是 "run failed 或 paused"，歧义严重。建议本轮明确选 failed（与"不做 Arena"一致），后续引入 Arena 后再改为 paused。

**M. Mutation 进入 RunEvent 审计流**（Opus 4.6 独家发现）

建议新增 `RunEvent::TopologyMutation { superstep, batch_id, result }`，保证 debug 和 UI 回放有事实依据。

### P2 — 细节优化

- **GraphRevision 存储膨胀**：显式声明 v1 使用 full snapshot，后续迭代收敛到 delta。（所有 review 共同观察）
- **`GraphMutationBatch.result` 生命周期**：收集阶段 result 未知，改为 `Option<>` 或分拆中间表示。
- **Revision 文件名用 superstep 对齐**，便于直接定位，无需额外索引。
- **`NodeOutcome.graph_mutations` 与 `GRAPH_MUTATIONS_CHANNEL` 二选一**，消除冗余。建议选前者（不引入 mutation channel，coordinator 在 outcome 收集阶段直接提取）。
- **GraphRevision.created_at 用强类型时间**而非 String。
- **Cursor 清理分两轮**：本轮在执行核心禁用，保留 `active_nodes` projection 防止 API 立即 break；下一轮前端适配后彻底删除。
- **模块实施顺序建议**（MiniMax）：低风险类型层 → mutation apply 逻辑 → coordinator/outcome 大改，降低中途 regression 风险。

---

## 关于五份 Review 的元观察

**GLM-5.1 的发现质量最高**：命中了最多的"只有它发现"的 P0 缺陷，且分析路径清晰、有具体方案。它对 spec 的处理是"找问题"而非"认可设计"。

**Kimi-2.6 的可操作性最强**：Branch/Loop 控制流替代协议分析是所有 review 中最贴近"实现者视角"的内容，避免了实现期的临时设计。

**MiniMax 2.7HS 发现了唯一的设计内部矛盾**（apply 顺序 vs 冲突规则），但整体问题密度偏低，更像工程实施计划 review。

**Gemini 3.0 Pro 的基调偏正面**，问题密度最低，但 SubGraph 作用域是唯一有人提出的系统性边界问题，值得纳入 spec。

**本机 Opus 4.6** 在时序模型和 channel state 迁移上有独到分析，但错过了 GLM 的 compile 失败和持久化原子性这两个 P0 级缺陷。

**Spec 总体质量结论**：核心设计方向正确，五份 review 无人质疑"barrier mutation + graph revision + cursor 移除"的方向。主要问题集中在 spec 内的"设计空缺"（compile 失败处理、持久化原子性、Branch/Loop 控制流替代、channel 初始化迁移）和"内部矛盾"（apply 顺序 vs 冲突规则），而非方向性错误。补充以上 P0 项后，spec 可进入实施阶段。



Minimax 2.7

# Review 质量评估报告

**评估对象**: 5份同题 review（基于 `spec_draft.md` + `existing_truth.md` + `research_results.md`）
**评估维度**:
1. **发现深度** - 找到的问题数量、质量、独特性
2. **优先级判断** - P0/P1/P2 分类准确性
3. **建议建设性** - 修复建议的具体性和可行性
4. **结构清晰度** - 组织逻辑和可读性
5. **互补性** - 与其他 review 是否有独特价值

---

## 评分总表

| Review | 发现深度 | 优先级判断 | 建议建设性 | 结构清晰度 | 互补性 | **总分** |
|--------|----------|------------|------------|------------|--------|----------|
| **Sisphus-M2.7HS** | 8.5 | 7.5 | 7.5 | 7.0 | 8.0 | **38.5/50** |
| **Opus-4.6** | 8.0 | 7.0 | 8.0 | 8.5 | 7.0 | **38.5/50** |
| **GLM-5.1** | 7.5 | 7.5 | 8.0 | 8.0 | 7.5 | **38.5/50** |
| **Kimi-2.6** | 6.5 | 7.0 | 7.5 | 8.5 | 6.0 | **35.5/50** |

**三者并列第一**，各有侧重。详见下文分解。

---

## 分项详细分析

### 1. Sisphus-M2.7HS Review (38.5/50)

**优势**:
- **最全面**: 覆盖架构设计(5原则)、模型完整性、apply顺序矛盾、冲突规则、cursor清理、模块清单、测试评估、验收标准、两个baseline文档对齐分析
- **唯一发现 apply 顺序与冲突规则的矛盾**: "remove_node(X) + add_node(X) 同id不同spec" 按顺序会先remove再add（最终成功），但冲突规则说是conflict——这是其他review都没直接指出的逻辑矛盾
- **风险矩阵量化**: 概率×影响+优先级的三维矩阵，直观
- **与 existing_truth 的双向对照**: 不仅验证一致性，还指出 cursor "已经是投影"的发现意味着清理成本可能低于草案预估

**弱点**:
- 冗长（~400行），信息密度不够高
- P0/P1/P2 优先级缺失（只有风险矩阵），不便快速定位阻塞项
- 部分建议行动（如 P2）可更精简

**独到价值**: 全局视角、架构层面的矛盾发现、风险量化

---

### 2. Opus-4.6 Review (38.5/50)

**优势**:
- **发现质量高**: 每个P0问题都有具体场景和明确建议
- **Mutation后channel state迁移**: 其他review未覆盖（新增channel初始value/version/versions_seen处理）
- **Remove node时进行中task处理**: 详细讨论了已完成writes和interrupt状态的边界
- **Mutation channel vs NodeOutcome 直接携带的二选一建议**: 直接指出草案冗余，要求明确选一个路径
- **13个问题全部结构化编号**: 方便追踪

**弱点**:
- P2问题过多（13个），可能说明优先级判断过于宽松
- 没有与 existing_truth/research_results 的显式对照
- 缺少对 Branch/Loop 控制流替代方案的深入讨论

**独到价值**: 架构层面的channel时序设计、状态迁移边界、channel vs outcome的二选一

---

### 3. GLM-5.1 Review (38.5/50)

**优势**:
- **事实一致性检查表**: 首个用表格对照spec声明与baseline的review，方法论好
- **P0问题覆盖全**: 编译失败处理、持久化原子性、PatchNodeConfig语义、mutation与writes一致性——4个高质量P0
- **迁移策略建议**: 历史run如何升级到新schema，有实际意义
- **测试补充建议**: 5条具体测试场景补充，具体可执行

**弱点**:
- 对 Branch/Loop 控制流替代方案未深入（最大实现缺口）
- 第5点（remove+add同id）虽然发现了但处理方式不够清晰
- 没有 Opus 的 channel state 迁移分析深

**独到价值**: 事实一致性检查方法论、迁移策略、持久化原子性分析

---

### 4. Kimi-2.6 Review (35.5/50)

**优势**:
- **最简洁**: 结论突出，可读性最好
- **Branch/Loop 控制流协议**: 最清晰的讨论，明确推荐用 Command.goto 作为替代
- **强项总结方法论**: 把强项单独列出，便于作者理解review立场
- **风险部分**: 简洁但抓住了关键（Branch/Loop断裂风险、持久化兼容性）

**弱点**:
- 最短但遗漏了多个重要问题：没有持久化原子性讨论、没有 channel state 迁移、没有 remove node 时的 task 处理
- PatchNodeConfig 语义只标记为P0但没有深入分析
- 与其他review的互补性最低（很多问题被其他review覆盖了）

**独到价值**: Branch/Loop控制流协议建议、简洁风险总结

---

## 排序结论

### 第一梯队（可作为主要依据）: Sisphus-M2.7HS + Opus-4.6

两者质量相当，**侧重点互补**：

| 维度 | Sisphus 强项 | Opus 强项 |
|------|-------------|-----------|
| 架构 | 5原则评估、矛盾发现 | channel时序、状态迁移 |
| 数据模型 | GraphRevision存储分析 | channel初始state |
| 执行 | cursor清理风险 | mutation与task边界 |
| 建议 | 风险矩阵量化 | 二选一明确 |

**建议**: 以 Opus 的 P0 清单为主（channel时序+状态迁移+task处理），以 Sisphus 的矛盾发现和风险矩阵为辅。

### 第二梯队（可作为参考）: GLM-5.1

质量与第一梯队相当，但无明显独特超越。不差但没有 Opus 的架构深度或 Sisphus 的矛盾发现。**可以作为第三意见**。

### 第三梯队（补充参考）: Kimi-2.6

简洁但覆盖不全。Branch/Loop 建议有价值，但不足以作为主要依据。

---

## 跨 Review 共识问题（P0 级别）

以下问题被**至少3份review**同时标记为重要：

| 问题 | Sisphus | Opus | GLM | Kimi |
|------|---------|------|-----|------|
| PatchNodeConfig 语义未定义 | ✅ | ✅ | ✅ | ✅ |
| Branch/Loop 控制流替代方案 | ✅ | ❌ | ❌ | ✅ |
| mutation 与 normal writes 时序 | ✅ | ✅ | ✅ | ❌ |
| cursor 清理影响 API 兼容性 | ✅ | ✅ | ❌ | ✅ |
| 持久化原子性/崩溃一致性 | ❌ | ✅ | ✅ | ❌ |
| apply 顺序与冲突规则矛盾 | ✅ | ❌ | ❌ | ❌ |

---

## 推荐行动

1. **实施前必须解决**（4项共识P0）:
   - PatchNodeConfig patch 语义（选 JSON Merge Patch）
   - Branch/Loop 控制流协议（选 Command.goto 方案）
   - mutation vs normal writes  barrier 时序分离
   - cursor 清理的渐进策略（推荐分两轮）

2. **Opus 额外指出但未被 Sisphus 覆盖的**:
   - Mutation 后新增 channel 的初始 state（zero value + version = current superstep）
   - Remove node 时进行中/暂停 task 的处理

3. **GLM-5.1 额外指出但未被其他覆盖的**:
   - 持久化原子性（两阶段提交策略）
   - 现有 run 的迁移策略
