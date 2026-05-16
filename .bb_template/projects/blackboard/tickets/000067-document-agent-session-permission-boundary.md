+++
id = "000067"
lane = "bbt"
title = "Document AgentSession permission boundary and current limitations"
created_at = "2026-05-16"
updated_at = "2026-05-16"
status = "archived"
area = "agent_session"
source = "codex-2026-05-16"
topic = "permissions"
+++

# 当前进展



# 记录

- AgentSession currently acts as an external runtime orchestrator for OpenCode, Codex, and CodeBuddy. It builds runtime-specific command arguments, injects MCP configuration, injects Blackboard identity environment variables, spawns the runtime process, captures provider events, and records session results.
- AgentSession does not currently implement a centralized local-tool permission gate. It does not intercept or authorize individual runtime tool calls such as shell, edit, webfetch, task, skill, or provider-native MCP tool invocations.
- Local tool permission behavior is currently delegated to each external runtime and its own profile/config behavior. OpenCode receives `--agent <agent>` and uses its agent profile; Codex and CodeBuddy receive their respective runtime configuration and arguments.
- AgentSession currently passes non-interactive permission bypass flags for runtime execution: OpenCode receives `--dangerously-skip-permissions`; Codex receives `--dangerously-bypass-approvals-and-sandbox`. These flags mean Blackboard should not treat runtime interactive approval as a Blackboard-side hard permission boundary.
- AgentSession injects `BB_DAEMON=1`, `BB_DAEMON_PROJECT`, `BB_DAEMON_AGENT`, `BB_AGENT_SESSION`, `BB_WORKSPACE_ROOT`, and `BB_PROJECT_ROOT` into spawned runtime processes. These values can be used by Blackboard-controlled services to identify the calling agent/session.
- The bb MCP server has limited server-side authorization checks for selected operations. For example, deleting an inbox note is gated by daemon identity checks. This is separate from AgentSession and is not a general AgentSession local-tool ACL.
- TaskGraph shell nodes have their own permission check before process spawn. That shell-node permission model is separate from AgentSession runtime execution.

# 下一步

- Keep this ticket as the current-state tracking item for AgentSession permission boundaries and limitations.
