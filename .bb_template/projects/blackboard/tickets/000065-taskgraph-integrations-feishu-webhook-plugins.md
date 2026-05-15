+++
id = "000065"
lane = "bbt"
title = "TaskGraph 第三方生态：飞书通知、Webhook、本地工具与插件接入"
created_at = "2026-05-14"
updated_at = "2026-05-14"
status = "todo"
area = "TaskGraph"
depends_on = "000055"
kind = "execution-subtrack"
parent = "000055"
requested_by = "user"
scope = "integrations-feishu-webhook-plugins"
+++

# 当前进展

- 2026-05-14：从 #000055 拆出第三方生态与通知闭环子单。

# 记录

- 目标：接入 TaskGraph 外部生态能力，包括飞书通知、Webhook、本地工具、插件/MCP tool node 等。
- 第一优先级：飞书 ready_for_review 通知，至少包含项目、任务、摘要、验收链接与失败/阻塞信息。
- 定位：通知不是锦上添花；用户离开电脑后必须有 IM 闭环，否则 autonomous workflow 不成立。

# 下一步

- 定义 Feishu notify node 或 notify policy 的第一阶段方案。
- 确认飞书 webhook/bot 凭证存储与权限边界。
- 预留 Webhook、MCP tool、local tool/plugin 的 integration node 契约。
