# Gemini 3.0 Pro Spec Review: Pregel Topology Mutation

## 1. 整体评价 (Overall Assessment)
草案《Pregel Topology Mutation 后端内核》逻辑清晰、目标明确，对现有技术基线（`existing_truth.md`）和竞品实现（`research_results.md`）有精准的把握。设计方案不仅弥补了 LangGraphJS 在动态拓扑上的不足，真正实现了 Pregel 论文 3.4 节的语义，还顺势根除了老旧的 cursor 依赖，架构上非常优雅。

**结论：强烈赞同该草案方向，设计严密，具备很高的实操性，建议在补充少数细节后直接进入开发。**

## 2. 事实与背景一致性 (Alignment with Truth & Research)
- **与现状 (`existing_truth.md`) 的契合度**：草案准确指出了当前分支“静态编译拓扑 + Pregel 风格调度”的现状，以及 cursor 的冗余残留。移除 `cursor` 作为事实源，确立 `PregelCheckpoint` 的唯一调度权威，这是对重构的完美收尾。
- **与调研 (`research_results.md`) 的契合度**：调研表明 LangGraphJS 并未真正实现 Topology Mutation。草案选择不盲从 LangGraphJS 的“伪动态” (仅 Send 派发)，而是严格回归 Pregel 理论模型（barrier 同步应用、确定性顺序），这不仅超越了 LangGraphJS 的能力，也为 Agent 协作提供了真正的动态规划 (plan-review) 底座。

## 3. 核心设计亮点 (Design Highlights)
- **不可变与版本化 (GraphRevision)**：引入 `GraphRevision` 并将其与 `PregelCheckpoint` 强绑定。这是最亮眼的设计，彻底解决了持久化恢复时可能出现的“拓扑漂移”问题（避免使用新代码/新拓扑跑老的状态）。
- **Mutation as Write**：将拓扑变更抽象为写入 `GRAPH_MUTATIONS_CHANNEL` 的操作，而非直接的内存或文件修改，这完美融入了现有的 superstep/barrier 架构，将拓扑变更与普通数据变更拉平。
- **扩展操作 (`PatchNodeConfig`)**：相比原版 Pregel 论文，额外增加了 `PatchNodeConfig`，这非常贴合 Agent 业务场景（比如更新 Prompt 或超时时间），而不需要繁琐的 remove 之后再 add。

## 4. 潜在风险与建议 (Risks & Recommendations)

**风险 1：Graph Revision 的存储膨胀**
- **现象**：每个有 Mutation 的 superstep 都会生成一个完整的 `task_graph_runs/<run_id>/graph_revisions/xxxxxx.json`。如果一个 Run 持续很长时间并发生大量微调，会导致磁盘占用快速膨胀。
- **建议**：考虑到这是 MVP（最小可行产品），初始版本可以全量保存以保证安全。但在草案中建议备注，未来可能需要引入基于 `base_revision + patch` 的增量存储或清理机制。

**风险 2：SubGraph 的 Mutation 作用域**
- **现象**：`existing_truth.md` 中提到了 SubGraph 节点。但草案并未明确：如果 SubGraph 内的节点触发了 Topology Mutation，它是修改父 Graph，还是修改子 Graph？
- **建议**：在草案中明确 Mutation Request 的作用域边界。默认建议：Mutation 仅对当前 Run 所在的局部 GraphContext 生效（即修改子图自身），不支持跨越执行栈修改父图拓扑。

**风险 3：前端 API 的硬兼容性断崖**
- **现象**：草案要求清理 `TaskGraphRun.cursor`、`SuperstepCheckpoint.cursor_before/after` 以及 `next_nodes`。虽然前端改动不在本轮范围内，但完全删去这些字段会导致前端直接白屏或请求崩溃。
- **建议**：强烈建议在第一阶段实现草案中提到的 **“可选兼容方式”**，即由后端 API 在出参中投影出 `active_nodes`（或者兼容旧字段名保留 `cursor` 提供只读列表）。这符合渐进式重构原则，能大大减轻后续前端的改造阻力。

**风险 4：HumanGate 的 Resume 触发**
- **现象**：草案指出“HumanGate resume 改为纯 Pregel write”，这意味着用户的 resume 操作只是写入了 event/write，图的推进需要下一个 superstep 调度。
- **建议**：确保 API 层的 `resume_run` 接口在保存了 Pregel write 后，能够立刻显式或隐式地触发一次 coordinator 的 tick（superstep 前进），避免用户点击通过后，执行看起来“挂起”未动。

## 5. 验收标准审核 (Acceptance Criteria Check)
- 草案的验收标准集中在 Cargo 后端测试（`cargo test -p bb_core ...` 和 `bb_cli`）。这非常务实且可量化。
- **补充建议**：确保在第 10 项“Checkpoint replay”测试中，包含“中途中断并恢复”的场景：先执行几个包含 Mutation 的 superstep，产生 `revision 0 -> 1 -> 2`，中断后再次启动，断言系统能严格根据 checkpoint 内的 `graph_revision = 2` 读取对应文件，而不是默认读取最新的图定义。

---
**Review 总结**: 这是一个成熟度极高、理论基础扎实的底层架构草案。在确认 SubGraph 作用域细节和 API 向下兼容策略后，即可放手推进核心逻辑开发。