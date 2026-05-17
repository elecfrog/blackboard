# Codex: add remote MCP fallback rule for Blackboard tools

时间: 2026-05-17T10:08:17Z
来源: Codex
项目: blackboard

## 做了什么

- Updated .bb_template/agents/CODEX.md with a mandatory Codex Remote MCP fallback protocol.
- Rule now states tool_search failure is not Blackboard MCP unavailability; Codex must check config, /healthz, /mcp initialize, /mcp tools/list, then use /mcp tools/call.
- Synced the codex agent connector after editing the template.

## 验证了什么

- Read .bb_template/agents/CODEX.md after patch: passed.
- remote /mcp tools/call sync_agent_connector: passed.
- No Rust or frontend source changed; no cargo/npm test required.

## 下一步

- If future Codex sessions still fail to expose active mcp__bb tools, they must use the documented remote /mcp fallback instead of reporting missing tools.

## 相关位置

- D:\\Dev\\blackboard\\.bb_template\\agents\\CODEX.md
- C:\\Users\\Administrator\\.codex\\AGENTS.md
