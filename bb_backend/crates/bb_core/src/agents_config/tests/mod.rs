use super::*;
use tempfile::TempDir;

fn make_bb_root(content: Option<&str>) -> TempDir {
    let bb_root = TempDir::new().expect("bb root tmp");
    if let Some(body) = content {
        let agents_dir = bb_root.path().join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("AGENTS.md"), body).unwrap();
    }
    bb_root
}

fn pick_codex(list: &AgentConnectorList) -> &AgentConnector {
    list.connectors.iter().find(|c| c.id == "codex").unwrap()
}

#[test]
fn list_reports_synced_when_target_matches_source() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("hello world"));
    // Sync pulls the connector fully into place (AGENTS.md + MCP). After
    // that every target should be `Synced` and therefore so should the
    // aggregate state surfaced by `list_with_home`.
    sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(list.source_state, AgentSourceState::Present);
    assert_eq!(pick_codex(&list).state, AgentConnectorState::Synced);
    assert!(pick_codex(&list).target_sha256_short.is_some());
    assert_eq!(
        pick_codex(&list).target_sha256_short,
        pick_codex(&list).source_sha256_short
    );
}

#[test]
fn list_reports_drift_when_target_differs() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("hello world"));
    let codex_dir = home.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(codex_dir.join("AGENTS.md"), "outdated").unwrap();

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codex(&list).state, AgentConnectorState::Drift);
    assert!(pick_codex(&list).target_sha256_short.is_some());
    assert_ne!(
        pick_codex(&list).target_sha256_short,
        pick_codex(&list).source_sha256_short
    );
}

#[test]
fn list_reports_missing_when_parent_exists_but_file_does_not() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("hello world"));
    fs::create_dir_all(home.path().join(".codex")).unwrap();

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codex(&list).state, AgentConnectorState::Missing);
    assert!(pick_codex(&list).target_sha256_short.is_none());
}

#[test]
fn list_reports_unreachable_when_parent_dir_missing() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("hello world"));

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codex(&list).state, AgentConnectorState::Unreachable);
}

#[test]
fn list_reports_source_missing_for_every_connector() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(None);
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::write(home.path().join(".codex/AGENTS.md"), "anything").unwrap();

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(list.source_state, AgentSourceState::Missing);
    for connector in &list.connectors {
        assert_eq!(connector.state, AgentConnectorState::SourceMissing);
    }
}

#[test]
fn sync_creates_parent_dir_and_writes_target() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("ground truth"));

    let before = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codex(&before).state, AgentConnectorState::Unreachable);

    let synced = sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_eq!(synced.state, AgentConnectorState::Synced);
    let target = home.path().join(".codex/AGENTS.md");
    assert_eq!(fs::read_to_string(&target).unwrap(), "ground truth");
}

#[test]
fn sync_uses_runtime_mcp_url_override() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("ground truth"));
    let runtime_mcp = "http://127.0.0.1:3002/mcp";

    let synced = sync_with_home_and_mcp_url(
        bb_root.path(),
        "codex",
        Some(home.path()),
        Some(runtime_mcp),
    )
    .unwrap();
    assert_eq!(synced.state, AgentConnectorState::Synced);
    let rendered = fs::read_to_string(home.path().join(".codex/config.toml")).unwrap();
    assert!(rendered.contains(runtime_mcp));
    assert!(!rendered.contains("http://127.0.0.1:3001/mcp"));

    let dynamic_list =
        list_with_home_and_mcp_url(bb_root.path(), Some(home.path()), Some(runtime_mcp)).unwrap();
    assert_eq!(pick_codex(&dynamic_list).state, AgentConnectorState::Synced);
    let default_list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codex(&default_list).state, AgentConnectorState::Drift);
}

#[cfg(unix)]
#[test]
fn sync_replaces_symlink_with_regular_file() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("ground truth"));
    let codex_dir = home.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    let other = home.path().join("other.md");
    fs::write(&other, "elsewhere").unwrap();
    let target = codex_dir.join("AGENTS.md");
    std::os::unix::fs::symlink(&other, &target).unwrap();

    let before = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert!(pick_codex(&before).is_symlink);

    let synced = sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_eq!(synced.state, AgentConnectorState::Synced);
    let meta = fs::symlink_metadata(&target).unwrap();
    assert!(!meta.file_type().is_symlink());
    assert_eq!(fs::read_to_string(&other).unwrap(), "elsewhere");
    assert_eq!(fs::read_to_string(&target).unwrap(), "ground truth");

    // Strict-flatten semantics still hold (asserted above). On top of that,
    // the sync must not be silent: it emits a structured `FlattenedSymlink`
    // event with the original link target, surfaced both on the aggregate
    // connector and on the specific target that was flattened.
    assert_eq!(
        synced.events.len(),
        1,
        "sync should emit exactly one event when exactly one symlink was flattened"
    );
    let target_path_display = synced.targets[0].target_path.clone();
    match &synced.events[0] {
        AgentConnectorSyncEvent::FlattenedSymlink {
            target_path: evt_target_path,
            previous_link_target,
        } => {
            assert_eq!(evt_target_path, &target_path_display);
            assert_eq!(
                previous_link_target.as_deref(),
                Some(other.to_string_lossy().as_ref()),
                "event should record the original symlink destination"
            );
        }
    }
    assert_eq!(
        synced.targets[0].events.len(),
        1,
        "the flattened-symlink event must be attached to the specific target it describes"
    );
}

#[test]
fn sync_returns_error_when_source_missing() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(None);
    fs::create_dir_all(home.path().join(".codex")).unwrap();

    let err = sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)));
}

#[test]
fn disconnect_returns_to_missing_state() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("ground truth"));
    sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    let target = home.path().join(".codex/AGENTS.md");
    assert!(target.exists());

    let after = disconnect_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_eq!(after.state, AgentConnectorState::Missing);
    assert!(!target.exists());
    assert!(home.path().join(".codex").is_dir());
}

#[test]
fn disconnect_is_idempotent_when_target_already_absent() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("ground truth"));
    fs::create_dir_all(home.path().join(".codex")).unwrap();

    let after = disconnect_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_eq!(after.state, AgentConnectorState::Missing);
}

#[test]
fn unknown_connector_id_is_invalid_input() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("x"));
    assert!(matches!(
        { sync_with_home(bb_root.path(), "no-such-agent", Some(home.path())) },
        Err(InboxError::InvalidInput(_))
    ));
    assert!(matches!(
        { disconnect_with_home(bb_root.path(), "no-such-agent", Some(home.path())) },
        Err(InboxError::InvalidInput(_))
    ));
}

fn pick_codebuddy(list: &AgentConnectorList) -> &AgentConnector {
    list.connectors
        .iter()
        .find(|c| c.id == "codebuddy")
        .unwrap()
}

#[test]
fn codebuddy_connector_syncs_agents_md_and_rules_mdc() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("# My Rules\n\nDo things."));

    let synced = sync_with_home(bb_root.path(), "codebuddy", Some(home.path())).unwrap();
    assert_eq!(synced.state, AgentConnectorState::Synced);
    // After the catalog grew an MCP injection target the connector now has
    // three targets: AGENTS.md, Rules.mdc, and MCP (bb).
    assert_eq!(synced.targets.len(), 3);

    let agents = home.path().join(".codebuddy/AGENTS.md");
    assert_eq!(
        fs::read_to_string(&agents).unwrap(),
        "# My Rules\n\nDo things."
    );
    let target = home.path().join(".codebuddy/rules/blackboard-rules.mdc");
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.starts_with(
            "---\ndescription: Blackboard cross-Agent rules\nglobs:\nalwaysApply: true\ntype: always\n---\n"
        ));
    assert!(content.contains("# My Rules\n\nDo things."));

    // MCP config file should now exist and contain the `bb` entry.
    let mcp_path = home.path().join(".codebuddy/mcp.json");
    let mcp_content = fs::read_to_string(&mcp_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();
    let bb = parsed
        .get("mcpServers")
        .and_then(|v| v.get("bb"))
        .expect("mcpServers.bb");
    assert_eq!(
        bb.get("url").and_then(|v| v.as_str()),
        Some("http://127.0.0.1:3001/mcp")
    );
}

#[test]
fn rules_connector_reports_missing_when_codebuddy_root_exists() {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".codebuddy")).unwrap();
    let bb_root = make_bb_root(Some("# Rules"));

    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codebuddy(&list).state, AgentConnectorState::Missing);
}

#[test]
fn codebuddy_connector_detects_synced_when_both_targets_match() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("# My Rules\n\nDo things."));

    // First sync
    sync_with_home(bb_root.path(), "codebuddy", Some(home.path())).unwrap();

    // Now inspect – should be Synced
    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codebuddy(&list).state, AgentConnectorState::Synced);
}

#[test]
fn codebuddy_connector_detects_drift_when_either_target_differs() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("# Original Rules"));

    // First sync
    sync_with_home(bb_root.path(), "codebuddy", Some(home.path())).unwrap();

    let rules_target = home.path().join(".codebuddy/rules/blackboard-rules.mdc");
    fs::write(rules_target, "# Modified Rules").unwrap();

    // Now inspect – should be Drift
    let list = list_with_home(bb_root.path(), Some(home.path())).unwrap();
    assert_eq!(pick_codebuddy(&list).state, AgentConnectorState::Drift);
}

#[test]
fn codebuddy_connector_disconnect_removes_both_files() {
    let home = TempDir::new().unwrap();
    let bb_root = make_bb_root(Some("# Rules"));

    sync_with_home(bb_root.path(), "codebuddy", Some(home.path())).unwrap();

    let agents = home.path().join(".codebuddy/AGENTS.md");
    let target = home.path().join(".codebuddy/rules/blackboard-rules.mdc");
    assert!(agents.exists());
    assert!(target.exists());

    let after = disconnect_with_home(bb_root.path(), "codebuddy", Some(home.path())).unwrap();
    assert_eq!(after.state, AgentConnectorState::Missing);
    assert!(!agents.exists());
    assert!(!target.exists());
    // Parent directory should remain
    assert!(home.path().join(".codebuddy/rules").is_dir());
}

#[test]
fn extract_rules_body_strips_frontmatter() {
    let content = "---\ndescription: Blackboard cross-Agent rules\nglobs:\nalwaysApply: true\ntype: always\n---\n# My Rules\nHello";
    assert_eq!(extract_rules_body(content), "# My Rules\nHello");
}

#[test]
fn extract_rules_body_returns_full_content_if_no_frontmatter() {
    let content = "# Just rules\nNo frontmatter here";
    assert_eq!(
        extract_rules_body(content),
        "# Just rules\nNo frontmatter here"
    );
}

#[test]
fn codex_connector_sync_injects_mcp_bb_and_preserves_user_config() {
    let home = TempDir::new().unwrap();
    let codex_dir = home.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    // Pre-populate a user config.toml with unrelated keys + an unrelated
    // mcp_servers entry. After connector sync, all of these must survive.
    fs::write(
        codex_dir.join("config.toml"),
        "model = \"gpt-5.5\"\n\n[mcp_servers.qmd]\nargs = [\"mcp\"]\ncommand = \"/opt/homebrew/bin/qmd\"\n",
    )
    .unwrap();
    let bb_root = make_bb_root(Some("rules"));

    let synced = sync_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_eq!(synced.state, AgentConnectorState::Synced);

    let rendered = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(rendered.contains("model = \"gpt-5.5\""));
    assert!(rendered.contains("[mcp_servers.qmd]"));
    assert!(rendered.contains("/opt/homebrew/bin/qmd"));
    assert!(rendered.contains("[mcp_servers.bb]"));
    assert!(rendered.contains("http://127.0.0.1:3001/mcp"));

    // Disconnect must leave user MCP servers alone but drop our bb entry.
    let after = disconnect_with_home(bb_root.path(), "codex", Some(home.path())).unwrap();
    assert_ne!(after.state, AgentConnectorState::Synced);
    let rendered = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(!rendered.contains("[mcp_servers.bb]"));
    assert!(rendered.contains("[mcp_servers.qmd]"));
    assert!(rendered.contains("/opt/homebrew/bin/qmd"));
    assert!(rendered.contains("model = \"gpt-5.5\""));
}

#[test]
fn opencode_connector_sync_preserves_tools_permission_and_siblings() {
    let home = TempDir::new().unwrap();
    let opencode_dir = home.path().join(".config/opencode");
    fs::create_dir_all(&opencode_dir).unwrap();
    fs::write(
        opencode_dir.join("opencode.json"),
        r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "qmd": {"type": "local", "command": ["qmd", "mcp"], "enabled": true}
  },
  "tools": {"qmd_*": false},
  "permission": {"read": "allow"}
}"#,
    )
    .unwrap();
    let bb_root = make_bb_root(Some("rules"));

    sync_with_home(bb_root.path(), "opencode", Some(home.path())).unwrap();
    let rendered = fs::read_to_string(opencode_dir.join("opencode.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert!(parsed.get("$schema").is_some());
    let mcp = parsed.get("mcp").and_then(|v| v.as_object()).unwrap();
    assert!(mcp.contains_key("qmd"));
    assert!(mcp.contains_key("bb"));
    assert!(parsed.get("tools").is_some());
    assert!(parsed.get("permission").is_some());

    disconnect_with_home(bb_root.path(), "opencode", Some(home.path())).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(opencode_dir.join("opencode.json")).unwrap())
            .unwrap();
    let mcp = parsed.get("mcp").and_then(|v| v.as_object()).unwrap();
    assert!(!mcp.contains_key("bb"));
    assert!(mcp.contains_key("qmd"));
}
