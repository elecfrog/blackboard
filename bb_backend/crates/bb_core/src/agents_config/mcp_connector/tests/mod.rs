//! Tests for three-format Agent MCP connector upsert / remove / inspect.
//!
//! Each test pins the "preserve unrelated keys" invariant because that is
//! the single most important contract of this module: the user's existing
//! MCP servers, tools, permissions, commands must survive the Blackboard
//! connector unchanged.

use super::*;
use std::fs;
use tempfile::TempDir;

fn bb_desired() -> AgentMcpServerConfig {
    AgentMcpServerConfig {
        id: "bb".to_string(),
        description: "Blackboard local MCP".to_string(),
        transport: AgentMcpTransport::RemoteHttp {
            url: "http://127.0.0.1:3001/mcp".to_string(),
        },
    }
}

// ---------------- Codex TOML ----------------

#[test]
fn codex_upsert_into_empty_file_creates_skeleton() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    upsert_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    assert!(rendered.contains("[mcp_servers.bb]"));
    assert!(rendered.contains("url = \"http://127.0.0.1:3001/mcp\""));
}

#[test]
fn codex_upsert_preserves_unrelated_sections() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    fs::write(
        &path,
        "\
model = \"gpt-5.5\"
notify = [\"/opt/tool\", \"turn-ended\"]

[mcp_servers.qmd]
args = [\"mcp\"]
command = \"/opt/homebrew/bin/qmd\"

[mcp_servers.rider]
url = \"http://127.0.0.1:64342/stream\"

[projects.\"/home/alice/proj\"]
trust_level = \"trusted\"
",
    )
    .unwrap();

    upsert_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    assert!(rendered.contains("model = \"gpt-5.5\""));
    assert!(rendered.contains("[mcp_servers.qmd]"));
    assert!(rendered.contains("/opt/homebrew/bin/qmd"));
    assert!(rendered.contains("[mcp_servers.rider]"));
    assert!(rendered.contains("http://127.0.0.1:64342/stream"));
    assert!(rendered.contains("[projects.\"/home/alice/proj\"]"));
    assert!(rendered.contains("[mcp_servers.bb]"));
    assert!(rendered.contains("url = \"http://127.0.0.1:3001/mcp\""));
}

#[test]
fn codex_inspect_synced_when_url_matches() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    upsert_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    let state = inspect_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    assert_eq!(state, AgentMcpServerTarget::Synced);
}

#[test]
fn codex_inspect_drift_when_url_edited() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    fs::write(
        &path,
        "[mcp_servers.bb]\nurl = \"http://127.0.0.1:9999/mcp\"\n",
    )
    .unwrap();
    let state = inspect_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    match state {
        AgentMcpServerTarget::Drift { actual_summary } => {
            assert!(actual_summary.contains("9999"));
        }
        other => panic!("expected Drift, got {other:?}"),
    }
}

#[test]
fn codex_inspect_missing_when_key_absent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    fs::write(&path, "[mcp_servers.qmd]\nurl = \"x\"\n").unwrap();
    let state = inspect_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    assert_eq!(state, AgentMcpServerTarget::Missing);
}

#[test]
fn codex_inspect_treats_unrepairable_parse_error_as_drift() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    fs::write(&path, "model = \"unterminated\n").unwrap();
    match inspect_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap() {
        AgentMcpServerTarget::Drift { actual_summary } => {
            assert!(actual_summary.contains("parse error"), "{actual_summary}");
        }
        other => panic!("expected Drift, got {other:?}"),
    }
}

#[test]
fn codex_inspect_unreachable_when_parent_missing() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nonexistent/config.toml");
    let state = inspect_server(AgentMcpConfigFormat::CodexToml, &path, &bb_desired()).unwrap();
    assert_eq!(state, AgentMcpServerTarget::Unreachable);
}

#[test]
fn codex_remove_leaves_other_servers_intact() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.toml");
    fs::write(
        &path,
        "\
[mcp_servers.qmd]
args = [\"mcp\"]
command = \"/opt/homebrew/bin/qmd\"

[mcp_servers.bb]
url = \"http://127.0.0.1:3001/mcp\"
",
    )
    .unwrap();

    remove_server(AgentMcpConfigFormat::CodexToml, &path, "bb").unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    assert!(!rendered.contains("[mcp_servers.bb]"));
    assert!(rendered.contains("[mcp_servers.qmd]"));
    assert!(rendered.contains("/opt/homebrew/bin/qmd"));
}

// ---------------- CodeBuddy JSON ----------------

#[test]
fn codebuddy_upsert_into_empty_file_creates_skeleton() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("mcp.json");
    upsert_server(AgentMcpConfigFormat::CodebuddyJson, &path, &bb_desired()).unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let bb = parsed
        .get("mcpServers")
        .and_then(|s| s.get("bb"))
        .expect("mcpServers.bb");
    assert_eq!(
        bb.get("type").and_then(|v| v.as_str()),
        Some("streamable-http")
    );
    assert_eq!(
        bb.get("url").and_then(|v| v.as_str()),
        Some("http://127.0.0.1:3001/mcp")
    );
}

#[test]
fn codebuddy_upsert_preserves_existing_servers() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("mcp.json");
    fs::write(
        &path,
        r#"{
  "mcpServers": {
    "sample-mcp": {
      "command": "npx",
      "args": ["-y", "@tencent/sample-mcp-server@latest"],
      "disabled": false
    },
    "qmd": {
      "command": "qmd",
      "args": ["mcp"]
    }
  }
}"#,
    )
    .unwrap();

    upsert_server(AgentMcpConfigFormat::CodebuddyJson, &path, &bb_desired()).unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let servers = parsed
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .unwrap();
    assert!(servers.contains_key("sample-mcp"));
    assert!(servers.contains_key("qmd"));
    assert!(servers.contains_key("bb"));
    assert_eq!(
        servers
            .get("sample-mcp")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str()),
        Some("npx")
    );
}

#[test]
fn codebuddy_inspect_synced_then_drift_after_edit() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("mcp.json");
    upsert_server(AgentMcpConfigFormat::CodebuddyJson, &path, &bb_desired()).unwrap();
    assert_eq!(
        inspect_server(AgentMcpConfigFormat::CodebuddyJson, &path, &bb_desired()).unwrap(),
        AgentMcpServerTarget::Synced
    );

    // User edits url by hand.
    let mut parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    parsed["mcpServers"]["bb"]["url"] = serde_json::json!("http://evil/mcp");
    fs::write(&path, serde_json::to_string_pretty(&parsed).unwrap()).unwrap();

    match inspect_server(AgentMcpConfigFormat::CodebuddyJson, &path, &bb_desired()).unwrap() {
        AgentMcpServerTarget::Drift { .. } => {}
        other => panic!("expected Drift, got {other:?}"),
    }
}

#[test]
fn codebuddy_remove_only_touches_bb_key() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("mcp.json");
    fs::write(
        &path,
        r#"{
  "mcpServers": {
    "qmd": {"command": "qmd", "args": ["mcp"]},
    "bb": {"type": "streamable-http", "url": "http://127.0.0.1:3001/mcp"}
  }
}"#,
    )
    .unwrap();
    remove_server(AgentMcpConfigFormat::CodebuddyJson, &path, "bb").unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let servers = parsed
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .unwrap();
    assert!(!servers.contains_key("bb"));
    assert!(servers.contains_key("qmd"));
}

// ---------------- OpenCode JSON ----------------

#[test]
fn opencode_upsert_into_empty_file_creates_skeleton() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("opencode.json");
    upsert_server(AgentMcpConfigFormat::OpencodeJson, &path, &bb_desired()).unwrap();
    let rendered = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let bb = parsed.get("mcp").and_then(|m| m.get("bb")).expect("mcp.bb");
    assert_eq!(bb.get("type").and_then(|v| v.as_str()), Some("remote"));
    assert_eq!(
        bb.get("url").and_then(|v| v.as_str()),
        Some("http://127.0.0.1:3001/mcp")
    );
    assert_eq!(bb.get("enabled").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn opencode_upsert_preserves_tools_permission_command_sections() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("opencode.json");
    fs::write(
        &path,
        r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "qmd": {"type": "local", "command": ["qmd", "mcp"], "enabled": true},
    "context7": {"type": "remote", "url": "https://mcp.context7.com/mcp", "enabled": true}
  },
  "tools": {"qmd_*": false, "bb_*": true},
  "permission": {"read": "allow"},
  "command": {"kb": {"description": "Run KB"}}
}"#,
    )
    .unwrap();
    upsert_server(AgentMcpConfigFormat::OpencodeJson, &path, &bb_desired()).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(parsed.get("$schema").is_some());
    let mcp = parsed.get("mcp").and_then(|v| v.as_object()).unwrap();
    assert!(mcp.contains_key("qmd"));
    assert!(mcp.contains_key("context7"));
    assert!(mcp.contains_key("bb"));
    assert!(parsed.get("tools").is_some());
    assert!(parsed.get("permission").is_some());
    assert!(parsed.get("command").and_then(|v| v.get("kb")).is_some());
}

#[test]
fn opencode_upsert_overwrites_existing_bb_without_disturbing_others() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("opencode.json");
    fs::write(
        &path,
        r#"{
  "mcp": {
    "bb": {"type": "local", "command": ["old", "stdio"], "enabled": true},
    "qmd": {"type": "local", "command": ["qmd", "mcp"], "enabled": true}
  }
}"#,
    )
    .unwrap();
    upsert_server(AgentMcpConfigFormat::OpencodeJson, &path, &bb_desired()).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let bb = parsed.get("mcp").and_then(|v| v.get("bb")).unwrap();
    assert_eq!(bb.get("type").and_then(|v| v.as_str()), Some("remote"));
    assert_eq!(
        bb.get("url").and_then(|v| v.as_str()),
        Some("http://127.0.0.1:3001/mcp")
    );
    // Old stdio keys are gone — upsert is authoritative.
    assert!(bb.get("command").is_none());
    // qmd still intact.
    assert!(parsed.get("mcp").and_then(|v| v.get("qmd")).is_some());
}

#[test]
fn opencode_inspect_missing_when_mcp_section_absent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("opencode.json");
    fs::write(&path, r#"{"tools": {}}"#).unwrap();
    let state = inspect_server(AgentMcpConfigFormat::OpencodeJson, &path, &bb_desired()).unwrap();
    assert_eq!(state, AgentMcpServerTarget::Missing);
}

#[test]
fn opencode_remove_keeps_siblings() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("opencode.json");
    fs::write(
        &path,
        r#"{
  "mcp": {
    "bb": {"type": "remote", "url": "http://127.0.0.1:3001/mcp", "enabled": true},
    "qmd": {"type": "local", "command": ["qmd", "mcp"], "enabled": true}
  },
  "tools": {"bb_*": true}
}"#,
    )
    .unwrap();
    remove_server(AgentMcpConfigFormat::OpencodeJson, &path, "bb").unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let mcp = parsed.get("mcp").and_then(|v| v.as_object()).unwrap();
    assert!(!mcp.contains_key("bb"));
    assert!(mcp.contains_key("qmd"));
    assert!(parsed.get("tools").is_some());
}
