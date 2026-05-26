# Pi TaskGraph Runtime Guide

适用范围：Blackboard TaskGraph 中 `runtime = "pi"` 的 LLM 节点。

这篇文档给新来的 agent 或工程师使用。读完后应该能回答三件事：

- 一个 Pi graph node 需要哪些安装条件。
- Graph 里如何声明 Pi 的 tools、MCP 和 skills。
- 出问题时先看哪里、不要误改哪里。

## Mental Model

TaskGraph 负责 workflow、状态、artifact、system write 和最终归档；Pi 只负责执行单个 LLM node 的一次 headless agent turn。

Pi node 的能力来自三层：

| 层 | 由谁提供 | Graph 中怎么声明 | 用途 |
| --- | --- | --- | --- |
| Pi runtime | Pi CLI | `runtime = "pi"` | 启动 headless agent，输出 JSON events |
| Pi MCP adapter | 已安装的 `pi-mcp-adapter` package | `custom_args` + `tool_policy` + `--mcp-config` | 让 Pi 调用 Blackboard MCP |
| Blackboard skills | `<bb-root>/skills/<id>/SKILL.md` | `config.skills` | 给 Pi 节点注入任务规则和输出契约 |

不要把这三层混在一起。Settings 里看到 Pi CLI installed，只说明 runtime 本体可用；如果 graph 要用 `mcp` tool，还需要 Pi 里安装了 `pi-mcp-adapter`。

## Existing Patterns

### Plain Pi LLM Node

用于总结、判断、生成报告。通常不需要 tools。

```json
{
  "type": "llm",
  "config": {
    "runtime": "pi",
    "agent": "native",
    "model": "minimax-cn/MiniMax-M2.7-highspeed",
    "prompt": { "mode": "inline", "template": "..." },
    "tool_policy": { "allowed_tools": [] }
  }
}
```

例子：`inbox-batch-cleanup.json` 的 `llm-decide-cleanup`，`nightly-auto-code-optimization.json` 的 brief/report 节点。

### Pi Node With File Tools

用于代码检查或小范围修复。必须显式写 `tool_policy`，尤其是 `write_roots` 和 bash allow/deny。

```json
{
  "runtime": "pi",
  "tool_policy": {
    "allowed_tools": ["read", "grep", "find", "ls", "edit"],
    "write_roots": ["{{env.root}}/../bb_web/src"]
  }
}
```

例子：`code-monitor-check-fix.json` 的 `frontend-i18n`、`frontend-theme`、`clippy-fix`。

### Pi Node With Skills

用于研究、review、scout fix 这类需要稳定行为契约的节点。

```json
{
  "runtime": "pi",
  "skills": ["code-research", "research-output-contract"]
}
```

Runtime 会把 `<bb-root>/skills/<id>/` 复制到本次 run/node 的 snapshot，再给 Pi 传：

```text
--no-skills --skill <runtime/task_graph_runs/.../skill_snapshots/...>
```

不要手工把这些 skills 写进共享 `.pi/skills` 作为 TaskGraph 依赖。共享 `.pi/skills` 容易被并发 run 污染，Graph 使用的是 snapshot。

例子：

- `research-internal.json`: `code-research`, `research-output-contract`
- `research-external.json`: `external-research`, `research-output-contract`
- `rust-vue-code-review.json`: `rust-vue-code-review`
- `code-review-scout-fix.json`: `code-review-scout-fix`

### Pi Node With Blackboard MCP

用于让 Pi 节点调用 Blackboard 结构化工具，例如写 inbox handoff。

Graph 里必须把 MCP 使用写清楚：

```json
{
  "runtime": "pi",
  "custom_args": ["--no-builtin-tools", "--tools", "mcp"],
  "tool_policy": {
    "allowed_tools": ["mcp"],
    "mcp": {
      "allow": ["^(bb_)?create_inbox_note$"]
    }
  }
}
```

Prompt 里也要明确调用方式：

```text
mcp({ tool: "bb_create_inbox_note", args: "<JSON string>" })
```

例子：`code-monitor-check-fix.json` 的 `mcp-handoff`。

## Installation Requirements

### Required For Any Pi Node

Pi CLI must be installed and version-compatible with Blackboard:

```text
npm install -g --ignore-scripts @earendil-works/pi-coding-agent@0.75.4
```

Blackboard Settings currently models this layer as the Pi tool install/check.

### Required For `mcp` Tool Nodes

Pi must also have the MCP adapter package installed:

```text
pi install npm:pi-mcp-adapter
```

Check with:

```text
pi list
```

If a graph node uses `custom_args: ["--no-builtin-tools", "--tools", "mcp"]` and Pi cannot see the `mcp` tool, check adapter installation first.

## MCP Adapter Contract

Pi core is not treated as Blackboard's native MCP client. The current contract is:

1. Blackboard renders pi-mcp-adapter config from the node's resolved tool plan.
2. `run_pi_turn` writes it under the AgentSession artifact directory:

```text
runtime/agent_sessions/<project>/<session-id>/artifacts/pi-mcp/mcp.json
```

3. Blackboard starts Pi with:

```text
--mcp-config <that mcp.json>
```

4. The installed `pi-mcp-adapter` package reads that config and exposes the `mcp` tool to Pi.

Do not remove `--mcp-config` because “Pi does not natively support MCP”. In this project it is the adapter handoff mechanism.

The generated config uses the Blackboard stdio MCP server:

```json
{
  "settings": {
    "toolPrefix": "server",
    "idleTimeout": 10,
    "directTools": false
  },
  "mcpServers": {
    "bb": {
      "command": "<current bb exe>",
      "args": ["--root", "<bb-root>", "stdio"],
      "env": {
        "BB_DAEMON": "1",
        "BB_DAEMON_AGENT": "<agent>"
      },
      "directTools": [
        "list_projects",
        "create_inbox_note",
        "read_ticket_by_id",
        "search_tickets"
      ],
      "lifecycle": "lazy",
      "idleTimeout": 10
    }
  }
}
```

Even if the runtime can render this config for many Pi nodes, a graph should only intentionally use MCP when the node's `custom_args`, `tool_policy`, and prompt make that use explicit.

## Runtime Flow

For a Pi node, the backend flow is:

1. Resolve graph node config into `ResolvedLlmInvocation`.
2. Render prompt from inline/file template and node inputs.
3. Render `pi_mcp_config_content` from the tool layer.
4. If `config.skills` is non-empty, create a run-local skill snapshot and append `--no-skills --skill <snapshot>`.
5. Create an AgentSession for the TaskGraph node.
6. Start Pi roughly as:

```text
pi --mode json --print
  [--session <provider-session-id>]
  [--model <model>]
  [--thinking <level>]
  [--extension <artifacts/pi-tool-policy/bb-pi-tool-policy.ts>]
  [--mcp-config <artifacts/pi-mcp/mcp.json>]
  [node/profile custom_args...]
  <prompt-or-@prompt.md>
```

7. Parse Pi JSON events into Blackboard AgentSession events.
8. Write node output/artifacts back to TaskGraph run state.

Useful code paths:

| Need | Path |
| --- | --- |
| Find actual Pi graph nodes | `.bb_template/task_graphs/system/*.json` |
| Pi spawn arguments | `bb_backend/crates/bb_core/src/agent_session/runtime.rs` |
| Pi event parser | `bb_backend/crates/bb_core/src/agent_session/providers/pi.rs` |
| Tool/MCP config rendering | `bb_backend/crates/bb_core/src/task_graph/tool_layer.rs` |
| Skill snapshot injection | `bb_backend/crates/bb_core/src/task_graph/nodes/runtime/llm_node.rs` |
| Pi CLI install spec | `bb_backend/crates/bb_core/src/agent_tools.rs` |

## AGENTS.md Boundary

Current Pi runs do not add `--no-context-files`, so Pi may read its normal context files, including `~/.pi/agent/AGENTS.md` and project-level `AGENTS.md`.

For TaskGraph correctness, do not put node-critical behavior only in AGENTS.md. Put it in one of:

- graph prompt
- ResourceBundle
- `config.skills`
- `tool_policy`
- deterministic system nodes

Treat AGENTS.md as background instructions for Pi, not as the authoritative graph contract. If the product needs fully deterministic headless Pi runs, implement that in runtime code with an explicit context strategy; documentation alone cannot guarantee it.

## Adding A New Pi Node

Use this checklist when authoring a graph:

1. Decide the node type: plain LLM, file-editing node, skill-driven node, or MCP node.
2. Set `runtime = "pi"` and choose the model.
3. Put task-specific behavior in the prompt or a skill, not only in AGENTS.md.
4. If the node can edit files, set `tool_policy.allowed_tools` and `write_roots`.
5. If the node can run shell, add bash whitelist/blacklist rules.
6. If the node needs Blackboard MCP, add `custom_args: ["--no-builtin-tools", "--tools", "mcp"]` and a narrow `tool_policy.mcp.allow`.
7. Prefer deterministic system nodes for final file writes; use MCP only when the LLM really must call a structured Blackboard tool.
8. Make expected output explicit through `output_contract` or downstream validation.

## Troubleshooting

| Symptom | Check First |
| --- | --- |
| Pi node will not start | `pi` CLI install/version and `pi_command` in `config/task_graph_runner.toml` |
| `mcp` tool is missing | `pi list` includes `npm:pi-mcp-adapter`; node has `--tools mcp` |
| MCP call blocked | `tool_policy.allowed_tools` includes `mcp`; `tool_policy.mcp.allow` matches the tool name |
| MCP server not reached | generated `artifacts/pi-mcp/mcp.json`; Blackboard stdio command/args/env |
| Skill behavior missing | graph `config.skills`; source exists under `<bb-root>/skills/<id>/`; snapshot was created |
| Node writes outside scope | `write_roots`, bash rules, and prompt constraints |
| Behavior differs from expected prompt | check whether Pi read AGENTS/context files; node-critical rules should be in graph prompt/skills |

## Do And Do Not

Do:

- Start from the graph definition when reasoning about Pi behavior.
- Treat `pi-mcp-adapter` as a real runtime dependency for MCP nodes.
- Keep skill behavior in `<bb-root>/skills` and let runtime snapshot it.
- Use `tool_policy` to make every writable/shell/MCP capability explicit.
- Preserve `--mcp-config` unless replacing the adapter architecture intentionally.

Do not:

- Assume Pi CLI installation alone is enough for MCP graph nodes.
- Assume AGENTS.md is the authoritative TaskGraph contract.
- Hand-edit shared `.pi/skills` to fix TaskGraph skill behavior.
- Remove `pi_mcp_config_content` just because Pi core is not an MCP client.
- Let a Pi node modify Blackboard managed ticket/inbox/wiki files with generic file tools; use structured Blackboard tools or system nodes.
