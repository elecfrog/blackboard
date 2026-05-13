+++
id = "000013"
lane = "bbt"
title = "BB 服务独立部署与独立 repo 运行收口"
created_at = "2026-05-07"
updated_at = "2026-05-08"
status = "archived"
assignee = "codex"
depends_on = "000005"
+++

# 当前进展

- 2026-05-07 Codex 创建本 ticket，收口 Blackboard 独立 repo 下 bb-server、bb_web、启动脚本与本地部署验证路径。
- 2026-05-07 Codex 新增 scripts/setup.py 与 scripts/dev.py，形成 clone/pull 后先 setup、日常开发跑 dev.py 的固定入口，并移除旧 restart-bb.py。
- 2026-05-07 Codex 新增根 .gitignore，忽略 Python __pycache__ 与 pyc 产物。
- 2026-05-07 Codex 移除非跨平台 shell 门禁 scripts/check-ticket-ids.sh，改为 Python 标准库实现 scripts/check_ticket_ids.py。
- 2026-05-07 Codex 移除已完成历史使命的一次性 migrate-family-to-lane.mjs，生产化 scripts 只保留可重复使用的 setup/dev/check 工具。

- 2026-05-07：修复 scripts/restart-bb.py 端口清理逻辑，macOS/Linux 用 lsof 查监听 PID，保留 Windows netstat/taskkill 路径
- 2026-05-07：将重启脚本里的后端描述统一为 bb_backend

- 2026-05-08：bb_server 新增 daemon 子命令原型（本地无头 runtime manager），增加 .bb_runtime 状态文件记录处理结果
- 2026-05-08：新增 bb-pm handoff 派发流程，按 project 扫描 inbox note 并构造单条处理 prompt，默认每轮 1 条
- 2026-05-08：daemon 增加 --dry-run/--once/--project/--retry-failed/--max-dispatch-per-scan 等试点参数
- 2026-05-08：真实试跑 bb_server daemon --agent bb-pm，完成首条 inbox handoff 的 ticket 凝练与删除
- 2026-05-08：调整 daemon 状态判定逻辑，识别权限拒绝/dry-run/retained/failed/completed 状态

# 记录

- 验证通过：python3 scripts/setup.py。
- 验证通过：python3 -m py_compile scripts/setup.py scripts/dev.py。
- 验证通过：python3 scripts/dev.py --help。
- 验证通过：npm run build --prefix bb_web。
- 验证通过：dev.py macOS lsof 端口清理 smoke。
- 验证通过：PYTHONDONTWRITEBYTECODE=1 python3 -m py_compile scripts/check_ticket_ids.py scripts/setup.py scripts/dev.py。
- 验证通过：PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_ticket_ids.py --project blackboard。
- 验证通过：PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_ticket_ids.py。
- 验证通过：rg 未来规则/当前 README 中不再要求 migrate-family-to-lane.mjs 或 check-ticket-ids.sh。

- 来源：inbox/2026-05-07-codex-restart-bb-macos.md

- 来源：inbox/2026-05-08-codex-bb-daemon-bb-pm-prototype.md

# 下一步

- 后续门禁统一使用 python3 scripts/check_ticket_ids.py；不再新增 shell-only 项目脚本。
- 如果后续仍有一次性迁移需求，优先做成 Rust 原生命令或 Python 管理脚本，并明确生命周期；完成后从常规 scripts 入口移除。
