use super::*;
use crate::{Blackboard, InboxError};
use std::fs;
use tempfile::TempDir;

fn wiki_fixture() -> (TempDir, Blackboard) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    fs::create_dir_all(root.join("inbox")).unwrap();
    fs::create_dir_all(root.join("tickets")).unwrap();
    fs::write(
        root.join("__project__.json"),
        r##"{
  "name": "kb",
  "type": "workflow",
  "repos": [],
  "description": "wiki fixture",
  "lanes": [
    { "id": "bbt", "label": "后端", "color": "#0f766e", "description": "", "status": "active" }
  ]
}"##,
    )
    .unwrap();
    let board = Blackboard::open(&root).unwrap();
    (temp, board)
}

#[test]
fn list_wiki_tree_returns_empty_when_dir_missing() {
    let (_temp, board) = wiki_fixture();
    let resp = board.list_wiki_tree().unwrap();
    assert!(resp.tree.is_empty());
    assert!(!resp.generated_at.is_empty());
}

#[test]
fn list_wiki_tree_lists_nested_structure_even_without_index() {
    let (_temp, board) = wiki_fixture();
    let wiki = board.ensure_wiki_root().unwrap();
    // Intentionally *no* top-level index.md to prove the tree does
    // not gate on it.
    fs::create_dir_all(wiki.join("modules/context-management")).unwrap();
    fs::write(wiki.join("modules/session.md"), "# Session").unwrap();
    fs::write(wiki.join("modules/context-management/index.md"), "# CM\n").unwrap();
    fs::write(
        wiki.join("modules/context-management/diagram.svg"),
        "<svg/>",
    )
    .unwrap();
    fs::write(wiki.join(".gitkeep"), "").unwrap();

    let resp = board.list_wiki_tree().unwrap();
    // One top-level dir entry: `modules/`.
    assert_eq!(resp.tree.len(), 1);
    let modules = &resp.tree[0];
    assert_eq!(modules.name, "modules");
    assert!(matches!(modules.kind, WikiNodeKind::Dir));
    assert_eq!(modules.path, "modules");

    let children = modules.children.as_ref().unwrap();
    // Dirs first (context-management), then files (session.md).
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "context-management");
    assert_eq!(children[0].path, "modules/context-management");
    assert!(matches!(children[0].kind, WikiNodeKind::Dir));
    assert_eq!(children[1].name, "session.md");
    assert_eq!(children[1].path, "modules/session.md");
    assert!(matches!(children[1].kind, WikiNodeKind::File));

    // `.gitkeep` is hidden and must not appear.
    let names: Vec<&str> = resp.tree.iter().map(|n| n.name.as_str()).collect();
    assert!(!names.contains(&".gitkeep"));
}

#[test]
fn read_wiki_content_reads_nested_markdown() {
    let (_temp, board) = wiki_fixture();
    let wiki = board.ensure_wiki_root().unwrap();
    fs::create_dir_all(wiki.join("modules")).unwrap();
    fs::write(wiki.join("modules/session.md"), "# Session\nbody").unwrap();

    let resp = board.read_wiki_content("modules/session.md").unwrap();
    assert_eq!(resp.content, "# Session\nbody");
    assert_eq!(resp.content_type, "md");
}

#[test]
fn read_wiki_content_reads_text_like_formats() {
    // SVG / XML / JSON / source code all count as readable text
    // documents; the frontend decides how to present each one.
    let (_temp, board) = wiki_fixture();
    let wiki = board.ensure_wiki_root().unwrap();
    fs::write(wiki.join("diagram.svg"), "<svg/>").unwrap();
    fs::write(wiki.join("data.json"), r#"{"a":1}"#).unwrap();
    fs::write(wiki.join("config.yaml"), "name: bb\n").unwrap();
    fs::write(wiki.join("script.ts"), "export const x = 1").unwrap();

    let svg = board.read_wiki_content("diagram.svg").unwrap();
    assert_eq!(svg.content, "<svg/>");
    assert_eq!(svg.content_type, "svg");

    let json = board.read_wiki_content("data.json").unwrap();
    assert_eq!(json.content, r#"{"a":1}"#);
    assert_eq!(json.content_type, "json");

    assert_eq!(
        board.read_wiki_content("config.yaml").unwrap().content_type,
        "yaml"
    );
    assert_eq!(
        board.read_wiki_content("script.ts").unwrap().content_type,
        "ts"
    );
}

#[test]
fn read_wiki_content_rejects_binary_only_formats() {
    // Binary formats (png, jpg, pdf, ...) must still go through the
    // asset endpoint; the document endpoint rejects them.
    let (_temp, board) = wiki_fixture();
    let wiki = board.ensure_wiki_root().unwrap();
    fs::write(wiki.join("logo.png"), b"\x89PNG").unwrap();
    fs::write(wiki.join("paper.pdf"), b"%PDF").unwrap();

    let err = board.read_wiki_content("logo.png").unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)), "got: {err:?}");
    let err = board.read_wiki_content("paper.pdf").unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)), "got: {err:?}");
}

#[test]
fn read_wiki_content_rejects_escapes() {
    let (_temp, board) = wiki_fixture();
    board.ensure_wiki_root().unwrap();
    for bad in [
        "../secrets.md",
        "/etc/passwd",
        "C:/Windows/notes.md",
        "",
        "modules//session.md",
        "modules/../../tickets/sentinel.md",
    ] {
        let err = board.read_wiki_content(bad).unwrap_err();
        assert!(
            matches!(err, InboxError::InvalidInput(_)),
            "expected InvalidInput for {bad:?}, got {err:?}"
        );
    }
}

#[test]
fn read_wiki_content_returns_not_found_for_missing_file() {
    let (_temp, board) = wiki_fixture();
    board.ensure_wiki_root().unwrap();
    let err = board.read_wiki_content("does-not-exist.md").unwrap_err();
    assert!(matches!(err, InboxError::NotFound(_)), "got: {err:?}");
}

#[test]
fn read_wiki_asset_returns_bytes_and_mime() {
    let (_temp, board) = wiki_fixture();
    let wiki = board.ensure_wiki_root().unwrap();
    fs::create_dir_all(wiki.join("modules")).unwrap();
    fs::write(wiki.join("modules/diagram.svg"), "<svg/>").unwrap();
    fs::write(wiki.join("modules/logo.png"), b"\x89PNG").unwrap();
    fs::write(wiki.join("modules/paper.pdf"), b"%PDF").unwrap();

    let svg = board.read_wiki_asset("modules/diagram.svg").unwrap();
    assert_eq!(svg.content_type, "image/svg+xml");
    assert_eq!(svg.bytes, b"<svg/>");

    let png = board.read_wiki_asset("modules/logo.png").unwrap();
    assert_eq!(png.content_type, "image/png");

    let pdf = board.read_wiki_asset("modules/paper.pdf").unwrap();
    assert_eq!(pdf.content_type, "application/pdf");
}

#[test]
fn ensure_wiki_root_is_idempotent() {
    let (_temp, board) = wiki_fixture();
    assert!(board.wiki_root().is_none());
    let first = board.ensure_wiki_root().unwrap();
    assert!(first.is_dir());
    let second = board.ensure_wiki_root().unwrap();
    assert_eq!(first, second);
}

#[test]
fn validate_wiki_path_for_upload_accepts_assets_and_docs() {
    let (_temp, board) = wiki_fixture();
    board.ensure_wiki_root().unwrap();
    // Docs and assets are both upload-allowed.
    board.validate_wiki_path_for_upload("index.md").unwrap();
    board
        .validate_wiki_path_for_upload("modules/session.md")
        .unwrap();
    board
        .validate_wiki_path_for_upload("modules/logo.png")
        .unwrap();
    // Disallowed extensions are rejected.
    let err = board.validate_wiki_path_for_upload("evil.exe").unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)), "got: {err:?}");
}
