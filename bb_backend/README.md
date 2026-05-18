# bb_backend

`bb_backend` 是 Blackboard 的本地多 project 服务 MVP。Desktop/产品运行时以用户级 `~/.bb` 为全局 root，并通过 `~/.bb/projects/__projects__.json` 注册所有打开过的 folder workspace project；源码开发态的 `scripts/dev.py` 默认使用 repo 根目录 `.bb_template`，方便维护 Blackboard 自身产品化记录并隔离运行态 `.bb`。

运行时拆成三层：

- `bb_core`：领域模型、存储、事务、权限、TaskGraph engine 和 service。
- `bb_daemon` / `bb-daemon`：本机权威常驻进程，承载 MCP、Web REST、scheduler、TaskGraph run dispatcher。
- `bb_server` / `bb-server`：本地 Web/API facade，只代理 `/api/**` 与 `/mcp/**` 到 daemon，不直接写 workspace。
- `bb_cli` / `bb`：人类命令入口；除 `init` 外兼容命令会转发到同目录 `bb-daemon`。

## 仓库布局

bb_backend 依托以下目录：

```
blackboard/
├── .bb/                         # source checkout runtime only; ignored
│   └── runtime/
└── .bb_template/
    ├── blackboard.json
    ├── agents/
    ├── task_graphs/
    ├── schemas/
    └── projects/
        ├── <project>/
        │   ├── __project__.json        # project 元数据：name / type / repos / description / lanes
        │   ├── __tickets__.json        # 机器生成：ticket 索引 + current_counter
        │   ├── __inbox__.json          # 机器生成：inbox note 索引
        │   ├── inbox/                  # JSON inbox handoff（通过工具创建/读取）
        │   └── tickets/                # JSON ticket；旧 Markdown 仅作为迁移输入
        └── ...
```

`__tickets__.json` / `__inbox__.json` 由 bb_backend 在运行时动态维护；`__project__.json` 是 project 元数据事实源：
- 首次访问时从源文件（JSON ticket / legacy Markdown / __project__.json）构建
- 每次 CRUD 操作后自动重建
- HTTP 列表接口直接读取这些文件（极快，无需遍历 ticket 源文件）
- 双下划线命名表达"机器生成，人类最好别碰"

## Commands

从任何目录都可以运行。Desktop/分发态通过 `--root ~/.bb` 指向用户级全局 root；源码开发态可以在 repo 下省略 `--root` 自动发现 repo `.bb_template`，或显式通过 `--root /abs/path/to/blackboard` 指向 repo root（会解析到 `blackboard/.bb_template`）。

```bash
cargo run -p bb_cli -- --help
cargo run -p bb_daemon -- --root ~/.bb stdio
cargo run -p bb_daemon -- --root ~/.bb serve --addr 127.0.0.1:3001
cargo run -p bb_server -- --addr 127.0.0.1:3002 --daemon-url http://127.0.0.1:3001
cargo run -p bb_cli -- --root ~/.bb init --seed /app/resources/home-template
cargo run -p bb_schema -- generate --root .bb_template
cargo run -p bb_schema -- check --root .bb_template
```

开发态推荐直接运行 `python scripts/dev.py`，它会启动 `bb_daemon`(3001)、`bb_server`(3002) 与 Vite(8060)。`GET /healthz` 用于 readiness probe。

## REST API

HTTP 同时提供 Web REST API 与 remote MCP 入口。Web 开发态访问 `bb_server`，由它代理到权威 `bb_daemon`；Desktop 单进程 sidecar 可直接使用 `bb-daemon serve`。ticket 的富写入仍以 MCP 工具为主，HTTP REST 只开放前端需要的结构化写入：inbox note 创建、lane 管理、以及按 ID 更新 ticket 的 status/lane/assignee/depends_on（不接受 raw Markdown 或任意原文 patch）。

Remote MCP：

- `/mcp` 使用 `rmcp` 提供的 MCP Streamable HTTP transport，不再提供旧的 SSE endpoint discovery。
- 客户端先 `POST /mcp` 发送 `initialize`；服务端返回 `mcp-session-id`，后续请求必须带同名 header。
- 响应格式由客户端 `Accept` 协商，可为 `application/json` 或 `text/event-stream`；`DELETE /mcp` 携带 session id 可终止会话。

- `GET /api/projects` 列出所有识别到的 project 及其 `__project__.json` 元数据（含 `lanes`）。
- `GET /api/projects/{project}/inbox/notes` 列出该 project 的 JSON inbox note 索引。
- `GET /api/projects/{project}/inbox/notes/{name}` 返回一条 JSON inbox note 的 `document` 与展示投影 `content`；路径穿越和越界访问返回 JSON 错误。
- `POST /api/projects/{project}/inbox/notes` 接受结构化字段（`source`、`topic`、可选 `title`/`time`/`done`/`validation`/`next_step`/`related_locations`/`related_tickets`/`attachments`/`extra`），并在该 project 的 inbox 中创建一个防冲突的 `YYYY-MM-DD-source-topic.json`。路由里的 `project` 会覆盖 body 中任何 `project` 字段。
- `GET /api/projects/{project}/board/summary` 返回 `BoardSummary` JSON（含 `total`/`by_status`/`by_lane`/metadata diagnostics）。
- `GET /api/projects/{project}/tickets` 返回 ticket 索引列表（不含正文 content），供 web 看板渲染结构化字段。
- `GET /api/projects/{project}/tickets/{id}/content` 返回单个 ticket 源内容；JSON ticket 原样返回，legacy Markdown 会去除 frontmatter 后返回正文。
- `PATCH /api/projects/{project}/tickets/{id}` 局部更新 ticket（body 支持 `status`、`lane`、`assignee`、`depends_on`，全部可选但至少传一个）。
- `GET /api/projects/{project}/lanes` 列出该 project 的全部 lane 定义（含 archived）。
- `POST /api/projects/{project}/lanes` 创建或替换一条 lane（body 是完整 `LaneDef`）。
- `PATCH /api/projects/{project}/lanes/{id}` 局部更新 lane 字段（label/color/description/status，全部可选）。
- `POST /api/projects/{project}/lanes/{id}/archive` 把 lane 标为 archived，返回 `{ lane, affected_ticket_count }`。

错误码映射：
- 未知 project → `404 not_found`
- 非法 project 名 / 非法 lane id / 非法输入 → `400 bad_request`
- 解析 `__project__.json` 失败 → `500 internal_error`

## stdio API

`bb-daemon stdio` 使用 `rmcp` stdio transport 在 stdin/stdout 上讲 MCP 协议。stdout 仅用于协议响应，日志和诊断写入 stderr。

核心 ticket/inbox 工具（完整清单以 `tools/list` 返回为准；project 级工具都要求 `project` 必填）：

- `list_projects`
- `list_inbox_notes` `{ project }`
- `read_inbox_note` `{ project, name }`
- `create_inbox_note` `{ project, source, topic, [title], [time], [done], [validation], [next_step], [related_locations], [related_tickets], [attachments], [extra] }` — 新建权威文件是 `YYYY-MM-DD-source-topic.json`，内容是 `schema_version=1` 的 JSON inbox document；读取接口返回 `document` 与展示投影 `content`。
- `list_tickets` `{ project }`
- `read_ticket` `{ project, name }`
- `read_ticket_by_id` `{ project, id }`
- `create_ticket` `{ project, lane, title, status, spec, [slug], [extra], [sections] }` — 新建权威文件是 `<id>-<slug>.json`；`spec` 是必填 BDD 定义，包含 `summary`、非空 `stories[{given,when,then,[sample]}]`、`risks[]`、`progress_record[]`；`stories[].id` 由后端在缺失时生成 UUID；`lane` 必须是该 project `__project__.json` 中的 active lane id；`status` 必须是 `todo` / `in_progress` / `blocked` / `review` / `done` / `archived`；非核心字段（`assignee`、`tags`、`team`、…）通过 `extra: { [key]: string }` 传入。
- `update_ticket` `{ project, id, frontmatter }` — `frontmatter` 是历史参数名，支持 `title`/`status`/`lane`/`spec`/`extra`/`remove`：核心字段直改（含 lane 切换，新 lane 必须是 active）；对 legacy Markdown 设置 `spec` 会迁移为同名 JSON ticket 并删除旧 `.md`。
- `append_ticket_sections` `{ project, id, [progress], [record], [next_step] }` — 对 JSON ticket 追加到 `progress_record`；对 legacy Markdown 仍追加到正文迁移辅助区。
- `board_summary` `{ project }`
- `list_lanes` `{ project }` — 列出所有 lane 定义（含 archived），与 HTTP 路由同源。
- `upsert_lane` `{ project, id, label, [color], [description], [status] }` — 创建或更新 lane；id 决定是否替换已有项。
- `archive_lane` `{ project, id }` — 把 lane 标为 archived，返回 `{ lane, affected_ticket_count }`。
- `search_notes` `{ project, query }` — 搜索 JSON inbox document 的展示投影。
- `search_tickets` `{ project, query }`

`project` 命名必须匹配正则 `^[a-z0-9][a-z0-9-]{0,63}$`；`lane` id 必须匹配 `^[a-z][a-z0-9-]{1,31}$`。

Search 工具使用字面量大小写不敏感子串匹配，返回带 `filename`/`path`/`status`/`line`/`snippet`；`status` 来自 JSON ticket 或 legacy frontmatter。

## Ticket tools

Web 运行时通过 HTTP 读取 tickets、inbox、board summary 与 lanes，并通过 HTTP 写入 ticket status、assignee、depends_on、inbox note 创建和 lane 管理。MCP（stdio 与 `/mcp` remote）承担更完整的 ticket CRUD、正文追加、搜索和 Agent 集成。

- `list_tickets` 列出 workspace registry 解析后的 `<project>/tickets/` 下 JSON ticket 与 legacy Markdown ticket，同时返回核心字段（`id`/`lane`/`title`/`status`/`created_at`/`updated_at`）、BDD `spec`、开放扩展 KV 映射 `extra`，以及 per-entry metadata diagnostics。
- `read_ticket` 按 `{project,name}` 读取单个 ticket 文件内容。
- `read_ticket_by_id` 按 `{project,id}` 扫描所有 ticket，报告 not-found 或 duplicate-id 错误，不做猜测。
- `create_ticket` 用结构化字段创建，由后端在对应 project 内分配六位 ID、生成 JSON 文件名并更新索引。调用方不能传 `id` 或 raw Markdown；`spec` 必填；非核心字段一律通过 `extra` 提交；`lane` 必须是 active。
- `update_ticket` 接受 `{project,id,frontmatter}`：核心字段 `title`/`status`/`lane` 直改；`spec` 用于把 legacy Markdown 迁移为 JSON ticket；其它字段通过 `extra: { [key]: string }` 上插、通过 `remove: string[]` 精确删除。不接受 raw patch，也不变更 `id`。
- `append_ticket_sections` 接受 `{project,id,progress,record,next_step}`，把结构化短句追加到 JSON `progress_record` 或 legacy Markdown 迁移辅助区，避免 Agent 手搓文件。
- `board_summary` 汇总该 project 的 status/lane 及 metadata diagnostics。
- `list_lanes` / `upsert_lane` / `archive_lane` 管理 `__project__.json` 里的 lane 目录。
- `search_tickets` 搜同一范围内的 ticket 文件。

每次 ticket 写操作都会触发一致性检查（id/lane/filename/duplicates/counter/lane_catalog），并在返回体的 `maintenance` 对象里报告。`assignee` 自 KV 化后不再是硬约束，缺失不会报错；Export 与 embedding 目前结构化地报告为"未运行/未实现"；Agent 不应该在 MCP 写流程中自己跑 shell 脚本。

## Lane 管理

Lane 是用户定义的工作分组（产品、后端、运营、增长……），定义在每个 project 的 `__project__.json` 里：

```json
{
  "lanes": [
    {
      "id": "bbp",
      "label": "产品",
      "color": "#7c3aed",
      "description": "商业化、合规、产品准入、发布门槛",
      "status": "active"
    }
  ]
}
```

约束：
- `id` 必须匹配 `^[a-z][a-z0-9-]{1,31}$`，project 内唯一。
- `status="active"` 才能挂新 ticket；`archived` 的 lane 上的历史 ticket 仍可读，但 `create_ticket` / `update_ticket(lane)` 会拒绝。
- 删除 lane **不被支持**——归档即软删除，避免破坏 ticket 引用完整性。

前端 BoardView 提供 LaneManager 对话框做增删改归档，背后调用上面四条 HTTP 路由；外部 agent 也可走 stdio MCP 工具。

## Breaking changes：v0.3 lane 重构

本版本相对前一版有两个互相关联的破坏性变更：

1. **frontmatter `family` → `lane`**。同时去掉硬编码 `["bbt","bbd","bbp","bbq"]` 枚举，改为读取该 project 的 `__project__.json` 里 `lanes` 数组。Lane id 形态扩展到 `^[a-z][a-z0-9-]{1,31}$`。
2. **ticket 文件名去 lane 前缀**。从 `<lane>-<id>-<slug>.md` 改成 `<id>-<slug>.md`，lane 仅在 frontmatter 里记录。

`create_ticket` 调用方迁移：

- 之前：`{ project, family, title, status, extra, ... }`
- 之后：`{ project, lane, title, status, extra, ... }` —— `lane` 必须存在于该 project active lane 列表
- 顶层 `family` 现在是 unknown field，会被 `additionalProperties:false` 拒绝（`-32602 unknown field`）

`update_ticket` 调用方迁移：

- 之前：`frontmatter: { title?, status?, extra?, remove? }`
- 之后：`frontmatter: { title?, status?, lane?, extra?, remove? }` —— `lane` 切换走核心字段，不再走 extra
- 想清理历史 frontmatter 里残留的 `family = "..."`：`remove: ["family"]`

`BoardSummary` JSON 字段改名：`by_family` → `by_lane`。前端 `BlackboardTicket.family` → `lane`。

仓库内已完成 `family -> lane` 与文件名归一化迁移，一次性迁移脚本不再作为生产化 scripts 入口保留。这意味着外部 agent（opencode / codex / 任何调用 MCP 的脚本）必须同步更新参数名。
