+++
id = "000054"
lane = "bbt"
title = "TaskGraph Task Creation & Compile：从意图到可执行计划"
created_at = "2026-05-14"
updated_at = "2026-05-17"
status = "archived"
area = "TaskGraph"
depends_on = "000053"
kind = "platform-track"
parent = "000053"
requested_by = "user"
scope = "task-creation-compile"
+++

# 当前进展

- 2026-05-14：从 TaskGraph Vision 拆出 Task Creation & Compile 主干 ticket。

- 2026-05-17:?????TaskGraph ? intent / inputs ???? graph/run ? compile ? graph workflow ???????????????

# 记录

- 目标：把 TaskIntent 转换成 TaskGraph Draft、模板选择或可执行 CompiledTaskGraph。
- Compile 负责解析 project/worktree/runtime/model/variant、权限包、输入输出 pins、Artifact contract、approval points，并校验节点完整性、runtime 可用性、cwd 存在性、权限边界、图结构合法性。
- 核心输出：CompiledTaskGraph / RunPlan，供 Task Execution 严格执行。

- ????:kb-wiki-build-workflow ????? inputs;intent resolver ???? run launcher / UI / API pre-start ?;Plan ??? data pin / artifact path ?????????????? depends_on=000053?

# 下一步

- 讨论 TaskGraph Draft 与 CompiledTaskGraph 的边界。
- 确定第一阶段是固定模板 compile，还是支持 Chat 生成 draft。
- 定义 compile error、approval point 和 artifact contract 的表现形式。

- ?????????;???????????,?????,????????
