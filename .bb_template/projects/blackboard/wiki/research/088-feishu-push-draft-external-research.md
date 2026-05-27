
我将首先读取上游的 scout 输出和计划文件，以它们为直接事实源进行综合分析。
# 飞书开放平台消息推送接入调研报告

**报告编号**: 088-feishu-push-draft-external-research  
**目标**: 为 Blackboard TaskGraph 接入飞书消息推送能力（ticket 088）提供外部技术调研  
**日期**: 2026-05-27  
**数据来源日期**: 2026-05 前的官方文档版本

---

## TL;DR

| 维度 | 推荐方案 |
|------|---------|
| **接入方式** | HTTP API 直调为主（TaskGraph Python runtime 轻量集成）；SDK（oapi-sdk-python）备选（自动 token 管理） |
| **认证机制** | tenant_access_token（应用身份，无用户授权流程），有效期 2 小时，<30 分钟触发续签 |
| **消息格式** | Interactive Card JSON（新版 schema），包含 header/elements/config，支持模板变量 |
| **本地测试** | lark-cli（官方 CLI，`+messages-send` 命令）+ 自定义机器人（Webhook，无需 token） |
| **Rate Limit** | 1000次/分 & 50次/秒（应用维度）；5 QPS（同一用户/群组维度） |

---

## 调研范围

本报告调研以下五个维度：

1. 飞书自建应用 Bot 发消息 API（`im/v1/messages`）
2. tenant_access_token 获取与续签机制
3. 消息卡片（Interactive Card）JSON 模板格式
4. 飞书 CLI 工具的消息发送能力
5. Rate Limit 和最佳实践

---

## 来源清单

| # | 类型 | 标题/描述 | 锚点 |
|---|------|-----------|------|
| 1 | API 文档 | 发送消息 API（im/v1/messages） | https://open.feishu.cn/document/server-docs/im-v1/message/create |
| 2 | API 文档 | tenant_access_token 获取 | https://open.feishu.cn/document/server-docs/authentication-management/access-token/tenant_access_token_internal |
| 3 | API 文档 | 卡片 JSON 结构（新版 1.0） | https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-json-structure |
| 4 | API 文档 | 卡片组件总览 | https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-components/component-overview |
| 5 | API 文档 | 卡片回传交互机制 | https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-callback-communication.md |
| 6 | API 文档 | Rate Limit 策略说明 | https://open.feishu.cn/document/server-docs/rate-limit/rate-limit-description.md |
| 7 | SDK | larksuite/oapi-sdk-python (GitHub) | https://github.com/larksuite/oapi-sdk-python |
| 8 | SDK | larksuite/oapi-sdk-go (GitHub) | https://github.com/larksuite/oapi-sdk-go |
| 9 | CLI | larksuite/cli (GitHub, 12.8k stars) | https://github.com/larksuite/cli |
| 10 | CLI | lark-cli 消息发送 reference | https://raw.githubusercontent.com/larksuite/cli/main/skills/lark-im/references/lark-im-messages-send.md |

> **注意**: 部分旧版文档（`/ukTMukTMukTM/` 路径）已标记为历史版本，新集成应使用新版路径（`/uAjLw4CM/ukzMukzMukzM/`）。

---

## 关键发现

### 1. 消息发送 API（im/v1/messages）

#### Endpoint 与请求格式

```
POST https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=xxx
Authorization: Bearer <tenant_access_token>
Content-Type: application/json
```

**必填参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| receive_id | string | 接收方 ID（open_id/union_id/user_id/email/chat_id） |
| msg_type | string | 消息类型（见下表） |
| content | string | JSON 序列化的消息内容 |

**支持的消息类型**：

| msg_type | 说明 | 限制 |
|----------|------|------|
| text | 纯文本 | 最大 150KB |
| post | 富文本（lark_md） | 最大 30KB |
| image | 图片（需先上传） | - |
| file | 文件（需先上传） | - |
| audio | 音频（需先上传） | - |
| media | 媒体（需先上传） | - |
| sticker | 表情 | - |
| **interactive** | **卡片消息** | **最大 30KB** |

**响应格式**：
```json
{
  "code": 0,           // 0=成功，非0=失败
  "msg": "success",
  "data": {
    "message_id": "xxx",
    "create_time": "xxx",
    "sender": { ... }
  }
}
```

#### 发送前前置条件

1. 机器人能力已开启
2. 应用已发布版本（生产环境需管理员审核）
3. 接收者在机器人可用范围内（单聊）或机器人在目标群组中（群聊）

---

### 2. tenant_access_token 获取与续签机制

#### 获取 API

```
POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
Content-Type: application/json

{
  "app_id": "cli_xxx",
  "app_secret": "your_app_secret"
}
```

**响应**：
```json
{
  "code": 0,
  "msg": "ok",
  "tenant_access_token": "t-xxx",
  "expire": 7200    // 固定 7200 秒（2小时）
}
```

#### 三种 access_token 对比

| Token 类型 | 值前缀 | 适用场景 | 授权流程 |
|-----------|--------|---------|---------|
| **tenant_access_token** | `t-` | 机器人推送通知等自动化场景 | 无需用户授权 |
| app_access_token | `a-` 或 `t-` | 商店应用获取 tenant token | 无需用户授权 |
| user_access_token | `u-` 或 `eyJ` 开头 | 代理用户操作 | 需要 OAuth 用户授权 |

> **结论**: Blackboard TaskGraph 应选用 tenant_access_token（应用身份），无需用户授权流程。

#### 续签策略（关键规则）

- **有效期**: 固定 2 小时（7200 秒），由飞书服务端控制
- **刷新触发条件**: 剩余有效期 < 30 分钟时，调用会返回**新 token**（两枚共存）
- **缓存策略**: 剩余有效期 >= 30 分钟时，返回**原 token**
- **客户端建议**: 无需精确计算刷新时间，每次 API 调用前尝试获取即可

#### Token 相关错误码

| 错误码 | 含义 | 处理建议 |
|--------|------|---------|
| 10012 | 获取 app_access_token 失败 | 检查 app_id/app_secret |
| 10013 | 获取 tenant_access_token 失败 | 检查凭证或网络 |
| 10015 | App Secret 错误 | 核对开发者后台凭证 |
| 4001 | Invalid token | 重新获取 token |

---

### 3. 消息卡片（Interactive Card）JSON 结构

#### 整体结构（Card JSON Schema 1.0）

```json
{
  "config": {
    "enable_forward": true,      // 是否允许转发
    "update_multi": false,       // 是否为共享卡片
    "width_mode": "default"      // 宽度模式
  },
  "header": {
    "title": {
      "tag": "plain_text",
      "content": "TaskGraph Run 完成"
    },
    "template": "green"           // blue/wathet/turquoise/green/yellow/orange/red/carmine/violet/purple/indigo/grey/default
  },
  "elements": [
    {
      "tag": "div",
      "text": {
        "tag": "lark_md",
        "content": "**Run ID**: run-20260527-xxx\n**状态**: ✅ 成功"
      }
    },
    {
      "tag": "action",
      "actions": [
        {
          "tag": "button",
          "text": {
            "tag": "plain_text",
            "content": "查看详情"
          },
          "type": "primary",
          "behaviors": [
            {
              "type": "open_url",
              "data": {
                "url": "https://..."
              }
            }
          ]
        }
      ]
    }
  ],
  "card_link": {
    "url": "https://...",
    "android_url": "...",
    "ios_url": "...",
    "pc_url": "..."
  }
}
```

#### 卡片组件分类

| 类别 | 组件 | 说明 |
|------|------|------|
| **容器类** | column_set | 分栏布局（横向多列） |
| | form | 表单容器（V6.6+，批量提交） |
| | interactive_container | 交互容器（V7.4+） |
| | collapsible_panel | 折叠面板（V7.9+） |
| | recycling-container | 循环容器（数据渲染） |
| **展示类** | header | 标题（支持 13 种主题色） |
| | div | 普通文本 |
| | markdown | 富文本（部分 GFM） |
| | img | 图片 |
| | table | 表格（V7.4+） |
| | chart | 图表（V7.1+） |
| | hr | 分割线 |
| **交互类** | button | 按钮（10 种样式） |
| | input | 输入框（V6.8+） |
| | select_static | 下拉单选 |
| | date_picker | 日期选择器 |
| | select_person | 人员单选 |

#### 卡片回调交互（card.action.trigger）

用户点击卡片按钮后，飞书会向配置的回调地址 POST 回调数据：

```json
{
  "header": {
    "event_id": "xxx",
    "event_type": "card.action.trigger",
    "create_time": "...",
    "token": "xxx",
    "app_id": "...",
    "tenant_key": "..."
  },
  "event": {
    "operator": {
      "open_id": "ou_xxx",
      "union_id": "on_xxx",
      "user_id": "u_xxx"
    },
    "action": {
      "tag": "button",
      "value": { ... },
      "name": "action_name"
    },
    "context": {
      "open_message_id": "om_xxx",
      "open_chat_id": "oc_xxx"
    }
  }
}
```

**服务端响应**（3 秒内）：
```json
{
  "schema": "2.0",
  "status": 0,
  "message": "success",
  "data": {
    "type": "toast",
    "toast": {
      "type": "success",
      "content": "操作成功"
    }
  }
}
```

或更新卡片：
```json
{
  "schema": "2.0",
  "status": 0,
  "data": {
    "type": "card",
    "card": {
      "type": "raw",
      "data": { /* 新卡片 JSON */ }
    }
  }
}
```

#### 主题色语义建议

| 颜色 | 语义 | 适用场景 |
|------|------|---------|
| green | 完成/成功 | Run 成功通知 |
| orange | 警告/警示 | Run 警告状态 |
| red | 错误/异常 | Run 失败通知 |
| grey | 失效 | 过时/取消状态 |

---

### 4. CLI 工具消息发送能力

#### lark-cli（官方 CLI，larksuite/cli）

| 属性 | 详情 |
|------|------|
| GitHub Stars | 12,793 |
| npm 包 | `@larksuite/cli` |
| 安装方式 | `npx @larksuite/cli@latest install` |
| 维护方 | 飞书/Lark 官方团队 |
| License | MIT |
| Commands | 200+（Shortcuts + API Commands + Raw API） |

#### 消息发送命令

```bash
# 基础发送
lark-cli im +messages-send \
  --as bot \
  --chat-id oc_xxx \
  --text "TaskGraph Run 完成"

# Markdown 消息（自动转为 post JSON）
lark-cli im +messages-send \
  --as bot \
  --chat-id oc_xxx \
  --markdown "**Run ID**: run-xxx"

# 自定义卡片 JSON
lark-cli im +messages-send \
  --as bot \
  --chat-id oc_xxx \
  --content '{"type":"template","data":{"template_id":"..."}}'

# 幂等发送（防止重复）
lark-cli im +messages-send \
  --as bot \
  --chat-id oc_xxx \
  --text "消息内容" \
  --idempotency-key $(uuidgen)

# 预览（不实际发送）
lark-cli im +messages-send \
  --as bot \
  --chat-id oc_xxx \
  --text "消息" \
  --dry-run
```

#### 认证初始化

```bash
# 初始化配置
lark-cli config init

# OAuth 登录（推荐）
lark-cli auth login --recommend
```

#### CLI 约束与限制

- `--markdown` 不保证完整 GFM（H1→H4 重写、图片处理有约束）
- 依赖 Node.js/npm 环境
- 不内置 rate limit 节流（需调用方控制）
- 不适合深度集成到 Python/Go 程序

#### 自定义机器人（Webhook）备选方案

无需创建应用和 token，适合固定群聊的简单通知：

```bash
# POST 到群组 webhook
curl -X POST "https://open.feishu.cn/open-apis/bot/v2/hook/{webhook_token}" \
  -H "Content-Type: application/json" \
  -d '{
    "msg_type": "text",
    "content": {"text": "TaskGraph Run 完成"}
  }'
```

**限制**：
- 仅限单群组使用（每个群独立 webhook）
- 无数据访问权限
- 限频：100次/分钟、5次/秒
- 难以动态管理多用户/多群组推送

---

### 5. Rate Limit 策略

#### 等级 4（消息发送 API）

| 维度 | 限制 |
|------|------|
| 应用维度 | 1000次/分钟、50次/秒 |
| 同一用户 | 5 QPS |
| 同一群组（机器人共享） | 5 QPS |

#### 等级体系速查

| 等级 | 限频 | 典型接口 |
|------|------|---------|
| 1 | 10次/分钟 | 低频管理接口 |
| 5 | 1次/秒 | - |
| 6 | 5次/秒 | - |
| 7 | 10次/秒 | - |
| 8 | 20次/秒 | - |
| 9 | 50次/秒 | - |
| **4** | **1000次/分 & 50次/秒** | **消息发送** |
| 10（商业版） | 100次/秒 | - |

#### 限流处理

1. 收到 HTTP 429 响应
2. 读取响应头 `x-ogw-ratelimit-reset`（等待秒数）
3. 等待后重试
4. **不建议固定间隔重试**

#### 常见消息错误码

| 错误码 | 含义 | 处理建议 |
|--------|------|---------|
| 230020 | 触发限频 | 等待 x-ogw-ratelimit-reset 后重试 |
| 230022 | 敏感信息拦截 | 检查消息内容 |
| 230025 | 消息超长 | 截断或精简内容（最大 30KB） |
| 230028 | DLP 审查未通过 | 消息含明文电话/邮箱，脱敏处理 |
| 230029 | 用户已离职 | 业务侧处理 |
| 230034 | receive_id 无效 | 核对用户 ID |
| 230035 | 无发送权限 | 检查应用可用范围 |
| 230053 | 用户停止接收机器人消息 | 业务侧处理 |

---

### 6. SDK vs HTTP API 直调 vs CLI 三方案对比

| 维度 | SDK（Python/Go/Node.js） | HTTP API 直调 | lark-cli |
|------|--------------------------|---------------|----------|
| **Token 管理** | ✅ 自动封装 | ❌ 需自行实现 | ✅ 自动封装 |
| **依赖引入** | ❌ 引入 SDK 包 | ✅ 无额外依赖 | ❌ 依赖 Node.js |
| **灵活性** | 中等 | ✅ 完全可控 | 低 |
| **易用性** | ✅ 开箱即用 | 中等 | ✅ 对 AI Agent 友好 |
| **批量处理** | ✅ 代码控制 | ✅ 代码控制 | 需 shell 脚本 |
| **Rate Limit** | 部分 SDK 可配置重试 | 需自行实现 | 需调用方控制 |
| **版本更新** | 可能滞后于 API | ✅ 始终最新 | 官方同步 |
| **生产适用** | ✅ 是 | ✅ 是 | 快速调试/本地测试 |
| **包体积** | Node.js: 26.7MB | ✅ 极小 | - |

**Blackboard TaskGraph 推荐**：
- **主方案**: HTTP API 直调（Python requests/httpx），配合自定义 token 管理模块
- **备选**: SDK（oapi-sdk-python）如果 token 管理和重试逻辑复杂度过高
- **本地测试**: lark-cli + 自定义机器人（Webhook）

---

## 可借鉴点

### 1. 轻量 token 管理模块设计

```
feishu_token_manager.py
├── _cache: {token, expire_time}
├── get_token() -> str  # 获取或刷新 token
├── _fetch_token() -> dict  # 调用 auth/v3/tenant_access_token
└── invalidate() -> None  # 强制失效（可选）
```

**策略**：每次 API 调用前检查 token 是否需要刷新，参考 SDK 的 30 分钟阈值机制。

### 2. TaskGraph Run 状态卡片模板

```
TaskGraph Run 通知卡片
├── header: 颜色对应状态（green=成功/red=失败/orange=警告）
├── elements:
│   ├── div: Run ID + 开始时间 + 耗时
│   ├── div: 状态摘要（成功 N 个 / 失败 M 个）
│   ├── div: 关键输出文件路径
│   ├── action:
│   │   ├── button: "查看 Run" → 跳转 Run 详情
│   │   └── button: "查看日志" → 跳转日志页
│   └── note: 免责声明（测试环境通知等）
└── card_link: 主跳转链接
```

### 3. 错误处理与重试机制

```python
def send_with_retry(payload, max_retries=3):
    for attempt in range(max_retries):
        resp = requests.post(url, headers=headers, json=payload)
        if resp.status_code == 429:
            wait_seconds = int(resp.headers.get('x-ogw-ratelimit-reset', 1))
            time.sleep(wait_seconds)
            continue
        elif resp.json().get('code') == 4001:  # token invalid
            token_manager.invalidate()
            continue
        return resp
    raise FeishuSendError("Max retries exceeded")
```

### 4. 集成点：TaskGraph structured tool lifecycle events

当前 TaskGraph 已有 `tool_layer` 和 `structured tool lifecycle events`，飞书推送应作为事件监听器接入：

```
ToolLifecycleEvent
├── on_start: 可选（Run 开始通知）
├── on_complete: 推荐（Run 完成通知）
├── on_error: 推荐（Run 失败通知）
└── payload: {run_id, status, duration, outputs, errors}
```

---

## 限制与风险

| 风险 | 级别 | 缓解措施 |
|------|------|---------|
| token 凭证泄露 | 高 | 环境变量注入，禁止硬编码 |
| SDK 版本滞后 | 中 | HTTP API 直调为主，SDK 备选 |
| 批量推送限频（5 QPS/用户） | 中 | 应用层发送队列 + 速率控制 |
| 消息超长（>30KB） | 低 | 内容截断和精简 |
| DLP 审查拦截（电话/邮箱） | 中 | 消息内容脱敏 |
| 应用未发布/机器人未开启 | 高 | 本地测试用自定义机器人 |
| 用户停止接收机器人消息 | 低 | 业务侧处理 |
| 用户离职 | 低 | 业务侧处理 |
| lark-cli GFM 不完整 | 低 | 仅用于本地调试，不用于生产 |
| Node.js 运行时不可用 | 低 | lark-cli 仅用于本地测试 |

---

## 后续建议

### 分阶段落地计划

| 阶段 | 目标 | 交付物 |
|------|------|--------|
| **Phase 0: 环境准备** | 飞书应用创建与配置 | app_id, app_secret, 机器人能力开启, 测试群组 |
| **Phase 1: 基础推送** | 实现 token 管理 + 文本消息发送 | `feishu_client.py`（token 管理、文本/富文本发送） |
| **Phase 2: 卡片模板** | 实现卡片消息模板 | 预定义 TaskGraph 状态卡片 JSON |
| **Phase 3: TaskGraph 集成** | 接入 tool_layer lifecycle events | 事件监听器 + Run 状态推送 |
| **Phase 4: 本地测试命令** | CLI 辅助调试 | lark-cli + 自定义机器人脚本 |
| **Phase 5: 生产完善** | 错误处理、重试、监控 | Rate limit 队列、错误告警 |

### 关键依赖

- **Ticket 000065**（TaskGraph 第三方生态）：tool_layer、structured tool lifecycle events 是飞书集成的关键基础
- **飞书应用权限**：`im:message` 或 `im:message:send_as_bot`（需在开发者后台申请并发布版本）

### 验收标准

1. ✅ 成功发送文本/卡片消息到指定用户和群组
2. ✅ token 自动续签（2 小时有效期内无需手动刷新）
3. ✅ Rate limit 429 触发时正确等待并重试
4. ✅ TaskGraph Run 完成/失败时自动推送通知
5. ✅ 本地测试命令（lark-cli）可快速验证消息发送
6. ✅ 消息内容通过 DLP 审查（无明文敏感信息）

---

## 附录：参考链接

- [飞书发送消息 API](https://open.feishu.cn/document/server-docs/im-v1/message/create)
- [tenant_access_token 获取](https://open.feishu.cn/document/server-docs/authentication-management/access-token/tenant_access_token_internal)
- [卡片 JSON 结构（新版）](https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-json-structure)
- [卡片组件文档](https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-components/component-overview)
- [Rate Limit 说明](https://open.feishu.cn/document/server-docs/rate-limit/rate-limit-description.md)
- [oapi-sdk-python](https://github.com/larksuite/oapi-sdk-python)
- [lark-cli](https://github.com/larksuite/cli)

---

*报告生成时间: 2026-05-27 | 数据来源: 飞书开放平台官方文档、SDK GitHub 仓库、lark-cli GitHub 仓库*