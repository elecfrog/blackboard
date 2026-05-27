# 飞书消息推送最简接入方式调研报告

> **报告日期**：2026-05-27
> **调研目标**：为 Blackboard TaskGraph (ticket 088) 提供飞书消息推送的最轻量接入路径
> **数据来源**：飞书官方开放平台文档（开放 API v3）

---

## TL;DR

**结论：选择自定义机器人 Webhook 方案，放弃自建应用 Bot。**

对于 Blackboard 场景（TaskGraph run 终态通知），自定义机器人 Webhook 是最短路径：

| 维度 | Webhook 方案 | 自建应用 Bot |
|------|-------------|-------------|
| 接入复杂度 | ✅ 一行 curl 即可推送 | ❌ 需创建应用、配权限、发版 |
| Token 管理 | ✅ 无需 token | ❌ 2h 过期需续签逻辑 |
| 限频 | ✅ 100次/分、5次/秒 | ✅ 1000次/分、50次/秒 |
| 卡片支持 | ✅ Interactive Card | ✅ Interactive Card |
| 适用场景 | 单向群通知 | 复杂权限、单聊、API 调用 |

Blackboard 仅需单向推送通知到指定群组，Webhook 方案完全覆盖 ticket 088 的四个 story（S1–S4），且无 token 管理负担。

---

## 调研范围

本次调研覆盖以下四个方向：

1. **自定义机器人 Webhook** — 最简接入路径、请求格式、payload 结构、限制
2. **自建应用 Bot + tenant_access_token** — token 获取流程、SDK 辅助、续签机制
3. **Interactive Card JSON 格式** — 结构规范、元素类型、交互行为
4. **限频与错误处理** — 429 重试策略、错误码体系、S3 不 panic 不阻塞设计

---

## 数据来源

| # | 来源 | URL |
|---|------|-----|
| 1 | 飞书自定义机器人接入文档 | https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot |
| 2 | 消息发送 API (im/v1/messages) | https://open.feishu.cn/document/server-docs/im-v1/message/create |
| 3 | 消息卡片结构 (Interactive Card) | https://open.feishu.cn/document/common-capabilities/message-card/message-cards-content/card-structure/card-content |
| 4 | tenant_access_token 获取文档 | https://open.feishu.cn/document/ukTMukTMukTM/ukDNz4SO0MjL5QzM/auth-v3/auth/tenant_access_token_internal.md |
| 5 | 飞书错误码总览 | https://open.feishu.cn/document/ukTMukTMukTM/ugjM14COyUjL4ITN.md |
| 6 | 限频策略文档 | https://open.feishu.cn/document/ukTMukTMukTM/uUzN04SN3QjL1cDN.md |
| 7 | 卡片 JSON 结构（uAjLw4CM） | https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-json-structure |
| 8 | 卡片组件：button | https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/bot-v3/bot-overview |
| 9 | 飞书 Go SDK (oapi-sdk-go) | https://github.com/larksuite/oapi-sdk-go/blob/v3_main/README.md |
| 10 | 飞书 Python SDK | https://github.com/larksuite/oapi-sdk-python/blob/main/README.md |

---

## 一、自定义机器人 Webhook：完全不需要 token

### 1.1 接入路径

自定义机器人（Custom Bot）是飞书官方提供的最轻量接入方式，**完全不需要 token**，无需创建自建应用，无需审核发布。

接入步骤：
1. 在目标飞书群组 → 群设置 → 添加机器人 → 选择"自定义机器人"
2. 复制 Webhook URL（格式：`https://open.feishu.cn/open-apis/bot/v2/hook/{robot_id}`）
3. 直接 POST JSON 到该 URL即可推送消息

> 来源：[飞书自定义机器人接入文档](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)

### 1.2 请求格式

```
POST https://open.feishu.cn/open-apis/bot/v2/hook/{robot_id}
Content-Type: application/json
```

**文本消息示例**：
```json
{
  "msg_type": "text",
  "content": {
    "text": "【Blackboard】TaskGraph run 已完成，请查看"
  }
}
```

**卡片消息示例**（需对 card JSON 做二次 JSON 转义）：
```json
{
  "msg_type": "interactive",
  "card": "{\"config\":{\"wide_screen_mode\":true},\"header\":{\"title\":{\"tag\":\"plain_text\",\"content\":\"✅ Run 完成\"},\"template\":\"blue\"},\"elements\":[{\"tag\":\"div\",\"text\":{\"tag\":\"lark_md\",\"content\":\"**项目**: blackboard\\n**状态**: ready_for_review\"}}]}"
}
```

成功返回：`{"code":0,"msg":"success"}`

> 来源：[飞书自定义机器人接入文档](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)

### 1.3 限制条件

| 限制项 | 数值 |
|--------|------|
| 频率上限 | **单机器人 100 次/分钟，5 次/秒** |
| 请求体大小 | 最大 **20KB** |
| 安全配置（可选） | 关键词 / IP 白名单 / 签名校验 |
| 交互能力 | 仅支持 `open_url` 跳转，不支持 callback 回调 |

> ⚠️ 坊间流传"20/30/50次/分"的限制数字不准确，官方文档明确为 100次/分、5次/秒。
> 
> 来源：[飞书自定义机器人接入文档](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)

### 1.4 安全配置三件套

Webhook URL 存在泄露后被滥用的风险，官方提供三种安全加固方式，可单独或组合使用：

| 方式 | 配置内容 | 适用场景 |
|------|---------|---------|
| **关键词校验** | 消息内容必须包含预设关键词（至少一个） | 简单防护 |
| **IP 白名单** | 仅白名单内 IP 可调用 | 推荐用于固定服务器部署 |
| **签名校验** | 基于 `HMAC-SHA256 + Base64`，请求体附加 `timestamp` + `sign`，时间戳超过 1h 签名失效 | 高安全要求 |

> 来源：[飞书自定义机器人接入文档](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)

---

## 二、自建应用 Bot：完整但更复杂

### 2.1 接入流程

如果未来需要更丰富的功能（如单聊、通讯录访问、用户 @ 等），可选择自建应用 Bot。接入路径更复杂：

1. 在 [飞书开发者后台](https://open.feishu.cn/app/) 创建自建应用
2. 获取 **App ID** + **App Secret**（凭证页面）
3. 在「应用功能 → 机器人」中启用 Bot 能力
4. 配置权限（`im:message` 或 `im:message:send_as_bot`）
5. 在「版本管理与发布」创建测试版本
6. 调用 API 发送消息

> 来源：[飞书自建应用创建文档](https://open.feishu.cn/document/home/self-built-app/create-an-app)

### 2.2 tenant_access_token 获取与续签

```
POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
Content-Type: application/json

{"app_id": "cli_xxx", "app_secret": "xxx"}
```

**关键机制**：
- 有效期：**2 小时（7200 秒）**
- 当剩余有效期 **< 30 分钟** 时，获取接口返回新 token（两个同时有效）
- 当剩余有效期 **>= 30 分钟** 时，返回原 token

**推荐续签策略**：
- 缓存 token + 记录 expire 时间
- 在剩余 **25 分钟**时主动刷新
- 网络错误时指数退避重试获取

> 来源：[tenant_access_token 文档](https://open.feishu.cn/document/ukTMukTMukTM/ukDNz4SO0MjL5QzM/auth-v3/auth/tenant_access_token_internal.md)

### 2.3 消息发送 API

```
POST https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id
Authorization: Bearer {tenant_access_token}
Content-Type: application/json

{
  "receive_id": "oc_xxx",
  "msg_type": "interactive",
  "content": "{\"header\":{...},\"body\":{...}}"
}
```

限频等级：**等级 4 — 1000次/分钟，50次/秒**。向同一用户/群组发送消息各限制 5 QPS。

> 来源：[消息发送 API](https://open.feishu.cn/document/server-docs/im-v1/message/create)

### 2.4 SDK 辅助

| 语言 | 配置方式 | 封装级别 |
|------|---------|---------|
| **Go** | `core.NewInternalAppSettings(core.SetAppCredentials("AppID","AppSecret"))` + `core.NewConfig(core.DomainFeiShu, ...)` | SDK 封装 token 获取 |
| **Python** | `Config.new_internal_app_settings_from_env()` 从 `APP_ID`/`APP_SECRET` 环境变量读取 | SDK 封装 token 获取 |

> 来源：[oapi-sdk-go README](https://github.com/larksuite/oapi-sdk-go/blob/v3_main/README.md)、[oapi-sdk-python README](https://github.com/larksuite/oapi-sdk-python/blob/main/README.md)

---

## 三、Interactive Card（消息卡片）JSON 格式

### 3.1 卡片 Schema 版本

飞书卡片有两套 JSON 结构：

| 版本 | 根节点 | 主要差异 |
|------|--------|---------|
| **JSON 1.0**（历史） | 无 schema 声明 | `header` + `elements` |
| **JSON 2.0**（当前） | `"schema": "2.0"` | `header` + `body` + `elements`，废弃了部分旧标签 |

自定义机器人发送卡片时，官方示例使用 **JSON 2.0** 结构。

> 来源：[卡片结构文档](https://open.feishu.cn/document/common-capabilities/message-card/message-cards-content/card-structure/card-content)

### 3.2 卡片结构四要素

```json
{
  "config": {
    "wide_screen_mode": true,    // 宽屏模式
    "update_multi": true         // 允许更新消息
  },
  "header": {
    "title": { "tag": "plain_text", "content": "✅ TaskGraph Run 完成" },
    "subtitle": { "tag": "plain_text", "content": "blackboard" },
    "template": "blue"           // blue/red/grey/warning/danger
  },
  "body": {
    "direction": "ltr",
    "padding": "12px 20px",
    "elements": [
      {
        "tag": "div",
        "text": {
          "tag": "lark_md",
          "content": "**状态**: ready_for_review\n**run_id**: abc-123"
        }
      },
      {
        "tag": "actions",
        "actions": [
          {
            "tag": "button",
            "text": { "tag": "plain_text", "content": "查看详情" },
            "type": "primary",
            "behaviors": { "type": "open_url", "data": {}, "default_url": "https://..." }
          }
        ]
      }
    ]
  }
}
```

> 来源：[卡片内容结构](https://open.feishu.cn/document/common-capabilities/message-card/message-cards-content/card-structure/card-content)

### 3.3 常用元素组件

| 组件 | 用途 | 关键属性 |
|------|------|---------|
| `div` | 普通文本 | `tag`: `plain_text`（纯文本）或 `lark_md`（支持 Markdown） |
| `markdown` | 富文本 Markdown | 支持 `*斜体*`、`**粗体**`、`<a href>链接</a>`、`<at id=all>@所有人</at>` |
| `hr` | 分割线 | 无子元素 |
| `img` | 图片 | `img_key` |
| `note` | 备注 | 含 `elements` 子数组 |
| `column_set` | 多列布局 | 含 `columns` 数组 |
| `actions` | 交互模块 | 含 `actions` 数组（按钮等） |

**lark_md 支持的语法**：换行 `\n`、斜体 `*text*`、粗体 `**text**`、删除线 `~~text~~`、链接 `<a href>text</a>`、@人 `<at id=open_id></at>`、彩色文本 `<font color=red>`。

> 来源：[plain-text 组件文档](https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/bot-v3/bot-overview)

### 3.4 按钮交互行为（behaviors）

| type | 行为 | 自定义机器人支持 |
|------|------|---------------|
| `open_url` | 打开链接跳转 | ✅ 支持 |
| `callback` | 回传数据到服务端 | ❌ 不支持 |
| `form_action` | 表单提交 | ❌ 不支持 |

按钮样式：`type` (primary/danger/default)、`size` (small/medium/large)、`disabled`。

> 来源：[button 组件文档](https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/bot-v3/bot-overview)

### 3.5 卡片大小限制

- **卡片消息最大 30KB**（自建应用 Bot）
- **自定义机器人 Webhook 最大 20KB**
- 卡片 JSON 超过 **200 个组件/元素** 时可能触发错误

---

## 四、三种方案对比

| 维度 | Webhook 自定义机器人 | 自建应用 Bot | 官方 CLI |
|------|---------------------|-------------|---------|
| **接入复杂度** | ✅ 一行 curl | ❌ 创建应用 + 配权限 + 发版 | ❌ 需安装 + 登录 |
| **Token 管理** | ✅ 无需 token | ❌ 2h 过期需续签 | ✅ 无需 token |
| **频率上限** | 100次/分，5次/秒 | 1000次/分，50次/秒 | — |
| **消息类型** | text/post/image/share_chat/interactive | 全部 + 单聊 | text |
| **交互能力** | 仅 open_url 跳转 | open_url + callback | 无 |
| **安全配置** | 关键词/IP/签名 | OAuth 权限体系 | 无 |
| **多群支持** | 每群一个机器人 | 一个应用发多个群 | 每群一个 |
| **数据访问** | ❌ 无任何权限 | ✅ 可访问通讯录、文档等 | ❌ 无 |
| **适用场景** | 通知推送（单群） | 机器人对话、单聊、API集成 | 调试/测试 |
| **对 ticket 088 的满足度** | ✅ 完全满足 | ✅ 过度满足 | ⚠️ 调试可用但非生产方案 |

> 结论：Webhook 方案完全满足 ticket 088 的 S1（配置凭证后成功投递）、S2（终态推送卡片）、S3（错误处理）、S4（本地验证）全部四个 story。无需引入自建应用 Bot 的复杂度。

---

## 五、限频与错误处理最佳实践

### 5.1 限频体系

**自定义机器人 Webhook**：
- 单机器人：**100 次/分钟，5 次/秒**
- 建议避开整点/半点时间以避免系统压力导致 11232 限流错误

**自建应用 Bot（im/v1/messages）**：
- 等级 4：**1000 次/分钟，50 次/秒**
- 向同一用户/群组：各 5 QPS

> 来源：[限频策略文档](https://open.feishu.cn/document/ukTMukTMukTM/uUzN04SN3QjL1cDN.md)

### 5.2 限频响应与重试策略

飞书限频时：
- HTTP 状态码：429（旧版返回 400）
- 响应头：`x-ogw-ratelimit-limit`（窗口上限秒数）、`x-ogw-ratelimit-reset`（距恢复秒数）
- 业务错误码：**99991400**（request trigger frequency limit）

| 错误类型 | 错误码 | 处理策略 |
|---------|--------|---------|
| 限频 | 99991400 / 230020 / 11232 | 读取 `x-ogw-ratelimit-reset` 延迟后重试 |
| 参数错误 | 230001 | ❌ 不重试，直接返回错误 |
| 权限错误 | 230035 / 11229 | ❌ 不重试，检查配置 |
| 内部错误 | 10500 / 10101 | ✅ 指数退避（1s/2s/4s）最多重试 3 次 |
| 消息体超限 | 230025 | ❌ 精简卡片 JSON |

**幂等性**：API 支持 `uuid` 参数（最大 50 字符），传入相同 uuid 在 **1 小时内至多成功发送一条消息**，可用于幂等去重。

> 来源：[限频文档](https://open.feishu.cn/document/ukTMukTMukTM/uUzN04SN3QjL1cDN.md)、[消息 API](https://open.feishu.cn/document/server-docs/im-v1/message/create)

### 5.3 错误码速查

| 错误码 | 含义 | 方案 |
|--------|------|------|
| `0` | 成功 | — |
| `11232` | Webhook 限流 | 延迟重试 |
| `19024` | 关键词校验失败 | 检查配置 |
| `19022` | IP 不在白名单 | 检查 IP 配置 |
| `19021` | 签名校验失败 | 检查签名逻辑 |
| `9499` | 请求体格式错误 | 检查 JSON 格式 |
| `230001` | 参数错误 | 检查请求参数 |
| `230020` | 触发限频 | 降低频率 |
| `230025` | 消息体超限 | 精简内容 |
| `230035` | 发送权限被拒 | 检查应用权限 |
| `230002` | 机器人不在群组 | 将机器人加入群 |
| `230006` | 未启用机器人能力 | 启用 Bot 能力 |
| `20005/20006/20013` | token 相关错误 | 刷新 token |
| `99991400` | 请求触发限频 | 延迟重试 |

> 来源：[错误码文档](https://open.feishu.cn/document/ukTMukTMukTM/ugjM14COyUjL4ITN.md)

### 5.4 S3 保障：不 panic 不阻塞的容错设计

Ticket 088 S3 要求"凭证过期或网络不可达时返回明确错误，不 panic，不阻塞 run"。设计要点：

1. **HTTP 客户端设置合理超时**（建议 10–15 秒），捕获 `net.DialError`、`net.Timeout`
2. **token 失效**时返回 `FeishuTokenExpired` 错误类型（推断：需自行定义错误类型）
3. **429 限频**时返回 `FeishuRateLimited` 错误，可选指数退避
4. **网络不可达**时记录日志并返回 `FeishuNetworkError`（推断：建议定义此错误类型）
5. **推送失败不阻断 TaskGraph run 主流程**（fire-and-forget 或带超时的异步推送）

> 来源：[错误码文档](https://open.feishu.cn/document/ukTMukTMukTM/ugjM14COyUjL4ITN.md)

---

## 六、风险汇总

| # | 风险 | 级别 | 缓解方案 |
|---|------|------|---------|
| R1 | Webhook URL 泄露后被滥用发送垃圾消息 | ⚠️ 中 | IP 白名单 + 不提交到公开仓库 |
| R2 | 自定义机器人只能推送到所在群，不支持跨群/单聊 | ℹ️ 低 | 多群场景需每个群单独配置一个机器人 |
| R3 | 卡片 JSON 转义容易出错（需二次 JSON 序列化） | ⚠️ 中 | 封装成独立函数，测试用例覆盖 |
| R4 | 签名校验时间戳有效期仅 1h，长期运行需定期更新 | ℹ️ 低 | 若使用签名校验，需维护定期刷新逻辑 |
| R5 | 飞书 API 内部错误 10500/10101 建议重试但无熔断可能雪崩 | ⚠️ 中 | 实现有限次数重试 + circuit breaker |
| R6 | 卡片内 @ 指定人需要 open_id（推断：无法通过邮箱查找） | ℹ️ 低 | 仅 @ 所有人，或未来升级为自建应用 Bot |
| R7 | WikiSlice 提到的"token 2h 过期"和"50QPS 限频"是**自建应用 Bot** 特性，Webhook 方案无此风险 | ✅ 已排除 | 选择 Webhook 方案规避 |

---

## 七、后续建议与迭代 Draft

### 7.1 推荐的最短路径（Phase 1 — MVP）

```
Blackboard TaskGraph 状态机
    │
    ├─ run 进入 ready_for_review / failed / blocked
    │
    └─> feishu_push.go（新增文件）
           │
           ├─ 读取配置（webhook_url、群 ID）
           ├─ 组装 Interactive Card JSON（header + body + button）
           ├─ HTTP POST → webhook URL
           ├─ 解析响应 code ≠ 0 → 记录错误
           └─ 超时/网络错误 → fire-and-forget，不阻塞 run
```

**配置文件（blackboard.yaml）**：
```yaml
feishu:
  webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/xxx"
  # 可选安全配置
  # security:
  #   ip_whitelist: ["1.2.3.4"]
  #   keyword: "Blackboard"
```

### 7.2 分阶段落地

| 阶段 | 内容 | 覆盖 Story |
|------|------|-----------|
| **Phase 1** | 飞书推送模块（S1）：读取 webhook URL 配置，发送文本消息到群 | S1 |
| **Phase 2** | 卡片消息（S2）：组装 Interactive Card JSON，header 颜色映射状态，body 含项目名/摘要，button 跳转查看链接 | S2 |
| **Phase 3** | 错误处理（S3）：超时捕获、错误码处理、fire-and-forget 不阻塞 run、错误日志规范 | S3 |
| **Phase 4** | 本地验证 CLI（S4）：`bb_cli feishu test-push` 命令，支持 `--webhook-url` 或从配置文件读取 | S4 |

### 7.3 需要改动的文件（推断）

| 文件 | 改动 |
|------|------|
| `bb_cli/` 下新增 `feishu/` 模块 | 实现推送逻辑和 `test-push` 子命令 |
| `blackboard.yaml` 或配置模块 | 新增 `feishu.webhook_url` 配置项 |
| TaskGraph 状态机代码 | 在终态触发时调用推送模块（注入 hook） |
| 文档/wiki | 添加飞书接入配置说明 |

---

## 附录：本地 curl 验证命令

**macOS / Linux**：
```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"msg_type":"text","content":{"text":"【Blackboard】Test push from curl"}}' \
  "https://open.feishu.cn/open-apis/bot/v2/hook/{your_robot_id}"
```

**Windows CMD**：
```cmd
curl -X POST -H "Content-Type: application/json" -d "{\"msg_type\":\"text\",\"content\":{\"text\":\"【Blackboard】Test push\"}}" "https://open.feishu.cn/open-apis/bot/v2/hook/{your_robot_id}"
```

**Windows PowerShell**：
```powershell
curl -X POST -ContentType "application/json" -Body (@{msg_type="text";content=@{text="【Blackboard】Test push"}} | ConvertTo-Json -Compress) "https://open.feishu.cn/open-apis/bot/v2/hook/{your_robot_id}"
```

> 来源：[飞书自定义机器人接入文档](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)

---

*本报告所有关键结论均标注 source anchors，部分实现细节（如错误类型定义、文件改动位置）基于任务上下文推断，应在实际实现前验证。*