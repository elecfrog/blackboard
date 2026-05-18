# Ticket Markdown To JSON Cleanup

把 Blackboard ticket 从 Markdown 文件迁移成 JSON 文件。

目录：

`{{env.root}}/projects/{{env.project}}/tickets`

JSON schema：

`{{env.root}}/schemas/ticket.schema.json`

每轮处理 `{{inputs.batch-count}}` 个文件。

步骤：

1. 只看 tickets 目录顶层的 `*.md` 文件，不递归子目录。
2. 按文件名升序取前 `{{inputs.batch-count}}` 个 `.md`。
3. 对每个 `.md`：
   - 读取 TOML frontmatter 和正文。
   - 生成同名 `.json` 文件。
   - JSON 必须符合 `{{env.root}}/schemas/ticket.schema.json`。
   - `stories` 用 Given / When / Then，`id` 用 UUID。
   - 旧 frontmatter 里除核心字段以外的信息放到 `extra`，不要放 `schema_version` 或 `ticket_spec`。
   - JSON 写入后必须用下面命令验证，通过后才删除原 `.md`：

```bash
python3 - <<'PY' "{{env.root}}/schemas/ticket.schema.json" "JSON_FILE_PATH"
import json, sys
schema_path, ticket_path = sys.argv[1], sys.argv[2]
schema = json.load(open(schema_path))
ticket = json.load(open(ticket_path))
missing = [key for key in schema["required"] if key not in ticket]
if missing:
    raise SystemExit(f"missing required fields: {missing}")
if ticket.get("schema_version") != 1:
    raise SystemExit("schema_version must be 1")
if not ticket.get("stories"):
    raise SystemExit("stories must be non-empty")
for idx, story in enumerate(ticket["stories"]):
    for key in ["id", "given", "when", "then"]:
        if not story.get(key):
            raise SystemExit(f"stories[{idx}].{key} is required")
print("valid")
PY
```

把 `JSON_FILE_PATH` 替换成刚写出的 `.json` 文件路径。

4. 本轮文件转换完成后，运行：

```bash
python3 {{env.scripts_dir}}/rebuild_ticket_index.py --project {{env.project}} --repo-root {{env.root}}/..
```

这个脚本负责更新 `__tickets__.json` 索引。不要手写 `__tickets__.json`。

索引验证方式：

```bash
python3 - <<'PY' "{{env.root}}/projects/{{env.project}}/__tickets__.json"
import json, sys
data = json.load(open(sys.argv[1]))
bad = [t["id"] for t in data.get("tickets", []) if "spec" in t]
if bad:
    raise SystemExit(f"index must not contain spec fields: {bad}")
print("index valid")
PY
```

不要调用 `create_ticket`、`update_ticket`、`append_ticket_sections` 等 ticket 写工具。
不要运行 `qmd embed`。
不要运行 `check_ticket_ids.py`。

最终只输出 JSON：

```json
{
  "processed": true,
  "converted": ["000001"],
  "skipped": [],
  "json_files_created": ["000001-example.json"],
  "markdown_files_removed": ["000001-example.md"],
  "remaining_legacy_count": 10,
  "continue": true,
  "verification_status": {
    "json_syntax": "passed",
    "ticket_shape": "passed",
    "file_conversion": "passed",
    "index_rebuilt": "passed"
  },
  "verification": [
    "000001-example.json: valid",
    "__tickets__.json: rebuilt"
  ]
}
```

`remaining_legacy_count` 是本轮结束后 tickets 顶层还剩多少个 `*.md` 文件。
`continue` 在 `remaining_legacy_count > 0` 时为 `true`，否则为 `false`。
