use super::*;
use std::fs;
use tempfile::TempDir;

fn fixture() -> (TempDir, Blackboard) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    fs::create_dir_all(root.join("inbox")).unwrap();
    fs::create_dir_all(root.join("tickets")).unwrap();
    // Seed a minimal __project__.json with the four historical lane ids so
    // create_ticket has a catalog to validate against. Tests that want to
    // exercise the "unknown lane" or "archived lane" branches overwrite
    // this file explicitly.
    fs::write(
        root.join("__project__.json"),
        r##"{
  "name": "kb",
  "type": "workflow",
  "repos": [],
  "description": "fixture",
  "lanes": [
    { "id": "bbt", "label": "后端", "color": "#0f766e", "description": "", "status": "active" },
    { "id": "bbd", "label": "前端", "color": "#2563eb", "description": "", "status": "active" },
    { "id": "bbp", "label": "产品", "color": "#7c3aed", "description": "", "status": "active" },
    { "id": "bbq", "label": "质量", "color": "#ea580c", "description": "", "status": "active" }
  ]
}"##,
    )
    .unwrap();
    let board = Blackboard::open(&root).unwrap();
    (temp, board)
}

fn input(topic: &str) -> InboxNoteInput {
    InboxNoteInput {
        title: Some("Test Note".to_string()),
        time: Some("2026-05-04T00:00:00Z".to_string()),
        source: "Codex Agent".to_string(),
        project: Some("kb".to_string()),
        topic: topic.to_string(),
        done: vec!["implemented something".to_string()],
        validation: vec!["cargo test".to_string()],
        next_step: vec!["review".to_string()],
        related_locations: vec!["blackboard/bb-server".to_string()],
        related_tickets: Vec::new(),
        attachments: Vec::new(),
        extra: FrontmatterExtra::new(),
    }
}

fn ticket_frontmatter(id: &str, lane: &str, title: &str, current: &str, status: &str) -> String {
    // NOTE: tests continue to write `current = "..."` as an extra KV to
    // exercise the legacy-pass-through path. The canonical frontmatter
    // shape produced by render_ticket only includes core fields; extra
    // KVs are optional.
    let spec_json = serde_json::to_string(&ticket_spec(title)).unwrap();
    format!(
            "+++\nid = \"{id}\"\nlane = \"{lane}\"\ntitle = \"{title}\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nassignee = \"alice\"\ncurrent = \"{current}\"\nstatus = \"{status}\"\nticket_spec = {spec_json:?}\n+++\n\n# {title}\n\n{current}\n"
    )
}

fn ticket_spec(summary: &str) -> TicketSpec {
    TicketSpec {
        summary: summary.to_string(),
        stories: vec![TicketStory {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            given: "用户处于目标场景".to_string(),
            when: "用户触发目标动作".to_string(),
            then: "系统表现出预期行为".to_string(),
            sample: None,
        }],
        risks: Vec::new(),
        progress_record: Vec::new(),
    }
}

fn create_ticket_input(title: &str) -> CreateTicketInput {
    CreateTicketInput {
        lane: "bbt".to_string(),
        title: title.to_string(),
        status: "todo".to_string(),
        spec: ticket_spec(title),
        attachments: Vec::new(),
        slug: None,
        extra: [("assignee".to_string(), "opencode".to_string())]
            .into_iter()
            .collect(),
        sections: Some(TicketBodySections {
            progress: vec!["已创建".to_string()],
            record: vec!["结构化输入".to_string()],
            next_step: vec!["继续处理".to_string()],
        }),
    }
}

fn write_ticket_index_counter(board: &Blackboard, counter: &str) {
    fs::write(
        board.root().join("__tickets__.json"),
        format!("{{\"current_counter\":\"{counter}\",\"tickets\":[]}}"),
    )
    .unwrap();
}

#[test]
fn lists_and_reads_only_json_note_names() {
    let (_temp, board) = fixture();
    let created = board.create_note(input("Agent Topic")).unwrap();
    fs::write(board.inbox().join("2026-05-04-agent-topic.md"), "ignored").unwrap();
    fs::write(board.inbox().join("notes.txt"), "ignored").unwrap();

    let list = board.list_notes().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, created.name);
    assert_eq!(list[0].title, "Test Note");
    assert_eq!(list[0].topic, "Agent Topic");
    assert_eq!(list[0].source, "Codex Agent");

    let note = board.read_note(&created.name).unwrap();
    assert_eq!(note.document.title, "Test Note");
    assert_eq!(note.document.topic, "Agent Topic");
    assert!(note.content.contains("implemented something"));
}

#[test]
fn rejects_traversal_and_missing_note_names() {
    let (_temp, board) = fixture();

    assert!(matches!(
        board.read_note("../tickets/x.json"),
        Err(InboxError::InvalidName(_))
    ));
    assert!(matches!(
        board.read_note("missing.json"),
        Err(InboxError::NotFound(_))
    ));
    assert!(matches!(
        board.read_note("legacy.md"),
        Err(InboxError::InvalidName(_))
    ));
}

#[test]
fn deletes_inbox_note_and_rebuilds_index() {
    let (_temp, board) = fixture();
    let created = board.create_note(input("Delete Me")).unwrap();
    let index = board.read_inbox_index().unwrap();
    assert_eq!(index.notes.len(), 1);

    let deleted = board.delete_note(&created.name).unwrap();
    assert_eq!(deleted.name, created.name);
    assert!(!board.inbox().join(&created.name).exists());
    assert!(board.read_note(&created.name).is_err());

    let index = board.read_inbox_index().unwrap();
    assert!(index.notes.is_empty());
}

#[test]
fn archives_inbox_note_and_rebuilds_index() {
    let (_temp, board) = fixture();
    let created = board.create_note(input("Archive Me")).unwrap();
    let archived = board.archive_note(&created.name).unwrap();

    assert_eq!(archived.name, created.name);
    assert_eq!(archived.original_path, format!("inbox/{}", created.name));
    assert_eq!(
        archived.archived_path,
        format!("inbox/archive/{}", created.name)
    );
    assert!(!board.inbox().join(&created.name).exists());
    assert!(board.inbox().join("archive").join(&created.name).is_file());
    assert!(matches!(
        board.read_note(&created.name),
        Err(InboxError::NotFound(_))
    ));

    let index = board.read_inbox_index().unwrap();
    assert!(index.notes.is_empty());
}

#[test]
fn archives_inbox_note_without_overwriting_existing_archive() {
    let (_temp, board) = fixture();
    let first = board.create_note(input("Archive Collision")).unwrap();
    let archived_first = board.archive_note(&first.name).unwrap();
    assert_eq!(archived_first.archived_name, first.name);

    let second = board.create_note(input("Archive Collision")).unwrap();
    let archived_second = board.archive_note(&second.name).unwrap();
    assert_ne!(archived_second.archived_name, archived_first.archived_name);
    assert!(board
        .inbox()
        .join("archive")
        .join(&archived_second.archived_name)
        .is_file());
}

#[test]
fn inbox_index_excerpt_truncates_unicode_safely() {
    let (_temp, board) = fixture();
    let mut note = input("Unicode Excerpt");
    note.done = vec![
        "完成 000034：抽取 GraphCanvas，把节点拖拽、连线、viewport fit 与布局保存入口沉到通用 Graph Canvas。"
            .repeat(4),
    ];
    let created = board.create_note(note).unwrap();
    board.rebuild_inbox_index().unwrap();
    let index = board.read_inbox_index().unwrap();
    let entry = index
        .notes
        .iter()
        .find(|entry| entry.name == created.name)
        .unwrap();

    assert!(entry.excerpt.ends_with("..."));
    assert!(entry.excerpt.chars().count() <= 143);
}

#[test]
fn creates_collision_resistant_notes_without_overwrite() {
    let (_temp, board) = fixture();
    let first = board.create_note(input("Same Topic!")).unwrap();
    let second = board.create_note(input("Same Topic!")).unwrap();

    assert_ne!(first.name, second.name);
    assert!(first.name.ends_with("codex-agent-same-topic.json"));
    assert!(second.name.ends_with("codex-agent-same-topic-1.json"));
    assert!(board.inbox().join(&first.name).exists());
    assert!(board.inbox().join(&second.name).exists());
}

#[test]
fn creates_ticket_with_backend_allocated_id_and_maintenance() {
    let (_temp, board) = fixture();
    let result = board
        .create_ticket(create_ticket_input("结构化写入 Ticket"))
        .unwrap();

    assert_eq!(result.ticket.id, "000001");
    assert_eq!(result.ticket.lane, "bbt");
    assert_eq!(result.ticket.status, "todo");
    // Filename is now `<id>-<slug>.json`; the lane (previously `bbt-`) no
    // longer appears as a prefix.
    assert!(result
        .ticket
        .file_name
        .starts_with("000001-结构化写入-ticket"));
    assert_eq!(result.maintenance.consistency.status, "passed");
    assert_eq!(result.maintenance.export.status, "completed");
    assert_eq!(
        result.maintenance.embedding.status,
        "external_embedding_pending"
    );
    let index = board.read_ticket_index().unwrap();
    assert_eq!(index.current_counter, "000001");

    let content = fs::read_to_string(board.root().join(&result.ticket.path)).unwrap();
    let document: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(document["id"], "000001");
    assert_eq!(document["lane"], "bbt");
    assert_eq!(document["extra"]["assignee"], "opencode");
    assert!(document["progress_record"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["summary"] == "已创建"));

    let read = board.read_ticket_by_id("000001").unwrap();
    assert_eq!(read.name, result.ticket.file_name);
}

#[test]
fn creates_ticket_after_existing_max_id_without_duplicate() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbd-000007-existing.md"),
        ticket_frontmatter("000007", "bbd", "Existing", "done", "done"),
    )
    .unwrap();

    let result = board
        .create_ticket(create_ticket_input("Next Ticket"))
        .unwrap();
    assert_eq!(result.ticket.id, "000008");
    let index = board.read_ticket_index().unwrap();
    assert_eq!(index.current_counter, "000008");
}

#[test]
fn rejects_invalid_structured_ticket_create_inputs() {
    let (_temp, board) = fixture();
    // Unknown lane id (format is valid but it is not in __project__.json):
    // the error bubbles up as InvalidInput rather than a dedicated
    // InvalidTicketFamily variant, which is the KV-era shape.
    let mut invalid = create_ticket_input("Invalid");
    invalid.lane = "no-such-lane".to_string();
    assert!(matches!(
        board.create_ticket(invalid),
        Err(InboxError::InvalidInput(_))
    ));

    // Empty core field (title) must still be rejected even though extra
    // fields are now free-form. assignee is no longer a required core
    // field, so its emptiness is explicitly allowed.
    let mut invalid = create_ticket_input("Invalid");
    invalid.title = "".to_string();
    assert!(matches!(
        board.create_ticket(invalid),
        Err(InboxError::InvalidInput(_))
    ));

    // Extra cannot shadow any CORE_FRONTMATTER_FIELDS entry.
    let mut invalid = create_ticket_input("Invalid");
    invalid
        .extra
        .insert("title".to_string(), "shadow".to_string());
    assert!(matches!(
        board.create_ticket(invalid),
        Err(InboxError::InvalidInput(_))
    ));
}

#[test]
fn updates_ticket_frontmatter_status_without_body_rewrite() {
    let (_temp, board) = fixture();
    let created = board.create_ticket(create_ticket_input("Move Me")).unwrap();
    let original_content = fs::read_to_string(board.root().join(&created.ticket.path)).unwrap();
    let original: serde_json::Value = serde_json::from_str(&original_content).unwrap();
    assert!(original["progress_record"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["summary"] == "Record: 结构化输入"));

    let result = board
        .update_ticket(UpdateTicketInput {
            id: created.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                title: Some("Moved".to_string()),
                status: Some("done".to_string()),
                lane: None,
                spec: None,
                attachments: None,
                extra: [("assignee".to_string(), "codex".to_string())]
                    .into_iter()
                    .collect(),
                remove: Vec::new(),
            }),
        })
        .unwrap();

    assert_eq!(result.ticket.title, "Moved");
    assert_eq!(
        result.ticket.extra.get("assignee").map(String::as_str),
        Some("codex")
    );
    assert_eq!(result.ticket.status, "done");
    // File stays in place (flat directory), only frontmatter changes
    assert!(board.root().join(&result.ticket.path).exists());
    let updated_content = fs::read_to_string(board.root().join(&result.ticket.path)).unwrap();
    let updated: serde_json::Value = serde_json::from_str(&updated_content).unwrap();
    assert_eq!(updated["title"], "Moved");
    assert_eq!(updated["extra"]["assignee"], "codex");
    assert_eq!(updated["status"], "done");
    assert!(updated["progress_record"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["summary"] == "Record: 结构化输入"));
    assert_eq!(result.maintenance.consistency.status, "passed");
}

#[test]
fn appends_ticket_sections_without_raw_markdown_patch() {
    let (_temp, board) = fixture();
    let created = board
        .create_ticket(create_ticket_input("Append Me"))
        .unwrap();

    let result = board
        .append_ticket_sections(AppendTicketSectionsInput {
            id: created.ticket.id.clone(),
            progress: vec!["完成 assignee 前端编辑".to_string()],
            record: vec!["来源 inbox/2026-05-06-demo.json".to_string()],
            next_step: vec!["人工验收 Dashboard".to_string()],
        })
        .unwrap();

    assert_eq!(result.ticket.id, created.ticket.id);
    let updated = fs::read_to_string(board.root().join(&result.ticket.path)).unwrap();
    let document: serde_json::Value = serde_json::from_str(&updated).unwrap();
    let records = document["progress_record"].as_array().unwrap();
    assert!(records
        .iter()
        .any(|record| record["summary"] == "完成 assignee 前端编辑"));
    assert!(records
        .iter()
        .any(|record| record["summary"] == "Record: 来源 inbox/2026-05-06-demo.json"));
    assert!(records
        .iter()
        .any(|record| record["summary"] == "Next step: 人工验收 Dashboard"));
    assert!(document["updated_at"].as_str().unwrap() >= created.ticket.updated_at.as_str());
}

#[test]
fn update_ticket_status_change_does_not_move_file() {
    let (_temp, board) = fixture();
    let created = board
        .create_ticket(create_ticket_input("Status Change"))
        .unwrap();
    let file_path = board.root().join(&created.ticket.path);
    assert!(file_path.exists());

    // Change status to "done" — file stays in place
    let result = board
        .update_ticket(UpdateTicketInput {
            id: created.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                status: Some("done".to_string()),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();
    assert_eq!(result.ticket.status, "done");
    // File is still at the same path
    assert!(file_path.exists());
    assert_eq!(result.ticket.path, created.ticket.path);
}

#[test]
fn deprecate_ticket_moves_file_and_resolves_active_relationships() {
    let (_temp, board) = fixture();
    let deprecated = board
        .create_ticket(create_ticket_input("Deprecated route"))
        .unwrap();
    let dependent = board
        .create_ticket(create_ticket_input("Depends on deprecated"))
        .unwrap();
    let attached = board
        .create_ticket(create_ticket_input("Attaches deprecated"))
        .unwrap();

    board
        .update_ticket(UpdateTicketInput {
            id: dependent.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                extra: [("depends_on".to_string(), deprecated.ticket.id.clone())]
                    .into_iter()
                    .collect(),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();
    board
        .update_ticket(UpdateTicketInput {
            id: attached.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                attachments: Some(vec![
                    TicketAttachment {
                        kind: "ticket".to_string(),
                        target: deprecated.ticket.id.clone(),
                        label: None,
                        description: None,
                    },
                    TicketAttachment {
                        kind: "wiki".to_string(),
                        target: "keep.md".to_string(),
                        label: None,
                        description: None,
                    },
                ]),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();

    let result = board
        .deprecate_ticket(DeprecateTicketInput {
            id: deprecated.ticket.id.clone(),
        })
        .unwrap();

    assert_eq!(
        result.ticket.path,
        format!("tickets/_deprecated/{}", deprecated.ticket.file_name)
    );
    assert!(!board.root().join(&deprecated.ticket.path).exists());
    assert!(board.root().join(&result.ticket.path).exists());
    assert_eq!(board.list_tickets().unwrap().tickets.len(), 2);
    assert!(result
        .removed_dependency_refs
        .contains(&dependent.ticket.id));
    assert!(result.removed_attachment_refs.contains(&attached.ticket.id));

    let dependent_content = fs::read_to_string(board.root().join(&dependent.ticket.path)).unwrap();
    assert!(!dependent_content.contains("depends_on ="));
    let attached_content = fs::read_to_string(board.root().join(&attached.ticket.path)).unwrap();
    assert!(!attached_content.contains(&format!(r#""target":"{}""#, deprecated.ticket.id)));
    assert!(attached_content.contains("keep.md"));

    let created_after_deprecation = board.create_ticket(create_ticket_input("Next id")).unwrap();
    assert_eq!(created_after_deprecation.ticket.id, "000004");
}

#[test]
fn update_ticket_rejects_empty_or_unsafe_changes() {
    let (_temp, board) = fixture();
    let created = board.create_ticket(create_ticket_input("Rejects")).unwrap();

    assert!(matches!(
        board.update_ticket(UpdateTicketInput {
            id: created.ticket.id.clone(),
            frontmatter: None,
        }),
        Err(InboxError::InvalidInput(_))
    ));

    assert!(matches!(
        board.update_ticket(UpdateTicketInput {
            id: "12345".to_string(),
            frontmatter: Some(TicketFrontmatterPatch {
                status: Some("done".to_string()),
                ..TicketFrontmatterPatch::default()
            }),
        }),
        Err(InboxError::InvalidTicketId(_))
    ));

    assert!(matches!(
        board.update_ticket(UpdateTicketInput {
            id: created.ticket.id,
            frontmatter: Some(TicketFrontmatterPatch {
                title: Some(" bad ".to_string()),
                ..TicketFrontmatterPatch::default()
            }),
        }),
        Err(InboxError::InvalidInput(_))
    ));
}

#[test]
fn update_ticket_does_not_add_created_at_when_missing() {
    let (_temp, board) = fixture();
    fs::write(
            board.root().join("tickets/bbt-000001-no-created.md"),
            "+++\nid = \"000001\"\nfamily = \"bbt\"\ntitle = \"No Created\"\nupdated_at = \"2026-05-01\"\nassignee = \"opencode\"\ncurrent = \"old\"\nstatus = \"todo\"\n+++\n\nbody",
        )
        .unwrap();
    write_ticket_index_counter(&board, "000001");

    let result = board
        .update_ticket(UpdateTicketInput {
            id: "000001".to_string(),
            frontmatter: Some(TicketFrontmatterPatch {
                // Write a new extension KV to force a frontmatter rewrite;
                // the test's point is that rewriting doesn't synthesize a
                // created_at field the original file never had.
                extra: [("assignee".to_string(), "codex".to_string())]
                    .into_iter()
                    .collect(),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();

    let content = fs::read_to_string(board.root().join(&result.ticket.path)).unwrap();
    assert!(!content.contains("created_at ="));
    assert!(content.contains("assignee = \"codex\""));
}

#[test]
fn consistency_reports_missing_frontmatter_id_without_filename_fallback() {
    let (_temp, board) = fixture();
    fs::write(
            board.root().join("tickets/bbt-000005-bad.md"),
            "+++\nfamily = \"bbt\"\ntitle = \"Bad\"\nupdated_at = \"2026-05-01\"\nassignee = \"opencode\"\ncurrent = \"bad\"\nstatus = \"todo\"\n+++\n\nbody",
        )
        .unwrap();
    write_ticket_index_counter(&board, "000005");

    let result = board
        .create_ticket(create_ticket_input("After Bad"))
        .unwrap();
    assert_eq!(result.ticket.id, "000006");
    assert_eq!(result.maintenance.consistency.status, "failed");
    assert!(result
        .maintenance
        .consistency
        .errors
        .iter()
        .any(|error| error.contains("invalid id in tickets/bbt-000005-bad.md")));
}

#[test]
fn discovers_blackboard_root_from_nested_workspace() {
    let (temp, _board) = fixture();
    let nested = temp.path().join("blackboard/bb-server/crates/bb-core");
    fs::create_dir_all(&nested).unwrap();

    let discovered = Blackboard::discover_from(nested).unwrap();
    assert!(discovered.root().ends_with("blackboard"));
}

#[test]
fn lists_tickets_by_status_order_and_ignores_non_markdown() {
    let (_temp, board) = fixture();
    fs::write(board.root().join("tickets/bbt-000002-two.md"), "two").unwrap();
    fs::write(board.root().join("tickets/bbt-000001-one.md"), "one").unwrap();
    fs::write(board.root().join("tickets/ignored.txt"), "ignored").unwrap();
    fs::create_dir_all(board.root().join("tickets/nested.md")).unwrap();
    fs::write(board.root().join("tickets/bbd-000003-done.md"), "done").unwrap();

    let list = board.list_tickets().unwrap();
    assert_eq!(
        list.tickets,
        vec![
            TicketEntry {
                name: "bbd-000003-done.md".to_string(),
                path: "tickets/bbd-000003-done.md".to_string(),
                id: Some("000003".to_string()),
                lane: None,
                title: None,
                status: None,
                created_at: None,
                updated_at: None,
                spec: None,
                attachments: Vec::new(),
                extra: FrontmatterExtra::new(),
                metadata_error: Some("missing leading +++ frontmatter".to_string()),
                metadata_warnings: REQUIRED_METADATA_FIELDS
                    .iter()
                    .map(|field| format!("missing frontmatter field `{field}`"))
                    .collect(),
            },
            TicketEntry {
                name: "bbt-000001-one.md".to_string(),
                path: "tickets/bbt-000001-one.md".to_string(),
                id: Some("000001".to_string()),
                lane: None,
                title: None,
                status: None,
                created_at: None,
                updated_at: None,
                spec: None,
                attachments: Vec::new(),
                extra: FrontmatterExtra::new(),
                metadata_error: Some("missing leading +++ frontmatter".to_string()),
                metadata_warnings: REQUIRED_METADATA_FIELDS
                    .iter()
                    .map(|field| format!("missing frontmatter field `{field}`"))
                    .collect(),
            },
            TicketEntry {
                name: "bbt-000002-two.md".to_string(),
                path: "tickets/bbt-000002-two.md".to_string(),
                id: Some("000002".to_string()),
                lane: None,
                title: None,
                status: None,
                created_at: None,
                updated_at: None,
                spec: None,
                attachments: Vec::new(),
                extra: FrontmatterExtra::new(),
                metadata_error: Some("missing leading +++ frontmatter".to_string()),
                metadata_warnings: REQUIRED_METADATA_FIELDS
                    .iter()
                    .map(|field| format!("missing frontmatter field `{field}`"))
                    .collect(),
            },
        ]
    );
}

#[test]
fn lists_ticket_metadata_and_per_entry_diagnostics() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbt-000001-one.md"),
        ticket_frontmatter("000001", "bbt", "One", "next", "todo"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbp-000009-mismatch.md"),
        ticket_frontmatter("000010", "bbp", "Mismatch", "check", "blocked"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbd-000003-missing.md"),
        "+++\nid = \"bad\"\nstatus = \"done\"\n+++\nbody",
    )
    .unwrap();

    let list = board.list_tickets().unwrap();
    let first = list
        .tickets
        .iter()
        .find(|entry| entry.name == "bbt-000001-one.md")
        .unwrap();
    assert_eq!(first.id.as_deref(), Some("000001"));
    assert_eq!(first.lane.as_deref(), Some("bbt"));
    assert_eq!(first.title.as_deref(), Some("One"));
    // `assignee` and `current` used to be first-class fields; now they
    // live in the open `extra` KV map. The fixture still writes both so
    // we verify the new storage location.
    assert_eq!(
        first.extra.get("assignee").map(String::as_str),
        Some("alice")
    );
    assert_eq!(first.extra.get("current").map(String::as_str), Some("next"));
    assert_eq!(first.status.as_deref(), Some("todo"));
    assert_eq!(first.updated_at.as_deref(), Some("2026-05-05"));
    assert!(first.metadata_error.is_none());
    assert!(first.metadata_warnings.is_empty());

    let mismatch = list
        .tickets
        .iter()
        .find(|entry| entry.name == "bbp-000009-mismatch.md")
        .unwrap();
    assert_eq!(mismatch.id.as_deref(), Some("000010"));
    assert!(mismatch
        .metadata_warnings
        .iter()
        .any(|warning| warning.contains("does not match filename id `000009`")));

    let missing = list
        .tickets
        .iter()
        .find(|entry| entry.name == "bbd-000003-missing.md")
        .unwrap();
    assert_eq!(missing.id.as_deref(), Some("000003"));
    assert_eq!(missing.status.as_deref(), Some("done"));
    assert!(missing
        .metadata_error
        .as_deref()
        .unwrap()
        .contains("invalid frontmatter id"));
    assert!(missing
        .metadata_warnings
        .iter()
        .any(|warning| warning.contains("missing frontmatter field `lane`")));
}

#[test]
fn reads_ticket_content_without_mutating_file() {
    let (_temp, board) = fixture();
    let path = board.root().join("tickets/bbt-000001-one.md");
    fs::write(&path, "ticket body").unwrap();

    let ticket = board.read_ticket("bbt-000001-one.md").unwrap();
    assert_eq!(ticket.name, "bbt-000001-one.md");
    assert_eq!(ticket.path, "tickets/bbt-000001-one.md");
    assert_eq!(ticket.content, "ticket body");
    assert_eq!(fs::read_to_string(path).unwrap(), "ticket body");
}

#[test]
fn reads_ticket_by_id_with_frontmatter_authority() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbt-000001-one.md"),
        ticket_frontmatter("000001", "bbt", "One", "active work", "in_progress"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbd-000002-two.md"),
        ticket_frontmatter("000007", "bbd", "Two", "done work", "done"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbp-000003-three.md"),
        "archived body without frontmatter",
    )
    .unwrap();

    let active = board.read_ticket_by_id("000001").unwrap();
    assert_eq!(active.status.as_deref(), Some("in_progress"));
    assert!(active.content.contains("active work"));

    let done = board.read_ticket_by_id("000007").unwrap();
    assert_eq!(done.name, "bbd-000002-two.md");

    let fallback = board.read_ticket_by_id("000003").unwrap();
    // Without frontmatter, status is absent and diagnostics explain why.
    assert_eq!(
        fallback.metadata_error.as_deref(),
        Some("missing leading +++ frontmatter")
    );

    assert!(matches!(
        board.read_ticket_by_id("000002"),
        Err(InboxError::TicketIdNotFound(id)) if id == "000002"
    ));
    assert!(matches!(
        board.read_ticket_by_id("12345"),
        Err(InboxError::InvalidTicketId(_))
    ));
}

#[test]
fn read_ticket_by_id_ignores_ids_from_unusable_frontmatter() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbt-000008-broken.md"),
        "+++\ntitle = \"Broken\"\n\n# Body\nid = \"000009\"\n",
    )
    .unwrap();

    assert!(matches!(
        board.read_ticket_by_id("000009"),
        Err(InboxError::TicketIdNotFound(id)) if id == "000009"
    ));

    let fallback = board.read_ticket_by_id("000008").unwrap();
    assert_eq!(fallback.name, "bbt-000008-broken.md");
    assert_eq!(
        fallback.metadata_error.as_deref(),
        Some("unterminated +++ frontmatter")
    );
    assert_eq!(fallback.id.as_deref(), Some("000008"));
}

#[test]
fn read_ticket_by_id_reports_duplicates() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbt-000001-one.md"),
        ticket_frontmatter("000001", "bbt", "One", "active", "todo"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbd-000001-copy.md"),
        ticket_frontmatter("000001", "bbd", "Copy", "done", "done"),
    )
    .unwrap();

    assert!(matches!(
        board.read_ticket_by_id("000001"),
        Err(InboxError::DuplicateTicketId { id, matches })
            if id == "000001" && matches.len() == 2
    ));
}

#[test]
fn summarizes_board_by_status_lane_and_metadata_diagnostics() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/bbt-000001-one.md"),
        ticket_frontmatter("000001", "bbt", "One", "active", "todo"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbp-000002-two.md"),
        ticket_frontmatter("000002", "bbp", "Two", "blocked", "blocked"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbd-000003-done.md"),
        ticket_frontmatter("000003", "bbd", "Done", "done", "done"),
    )
    .unwrap();
    fs::write(
        board.root().join("tickets/bbt-000004-archived.md"),
        "body without frontmatter",
    )
    .unwrap();

    let summary = board.board_summary().unwrap();
    assert_eq!(summary.total, 4);
    // Ticket without frontmatter defaults to todo for aggregate counts.
    assert_eq!(summary.by_status["todo"], 2);
    assert_eq!(summary.by_status["in_progress"], 0);
    assert_eq!(summary.by_status["blocked"], 1);
    assert_eq!(summary.by_status["done"], 1);
    assert_eq!(summary.by_status["archived"], 0);
    assert_eq!(summary.by_lane["bbt"], 1);
    assert_eq!(summary.by_lane["bbp"], 1);
    assert_eq!(summary.metadata_error_count, 1);
    // REQUIRED_METADATA_FIELDS was trimmed from 7 to 5 (id/family/title/
    // status/updated_at) after the KV migration, so the minimum expected
    // warning count for a completely empty ticket drops accordingly.
    assert!(summary.metadata_warning_count >= 5);
}

#[test]
fn rejects_unsafe_ticket_reads() {
    let (_temp, board) = fixture();

    // Traversal attempts are rejected
    assert!(matches!(
        board.read_ticket("../done/bbt-000001-one.md"),
        Err(InboxError::InvalidName(_))
    ));
    assert!(matches!(
        board.read_ticket("..%2Fdone%2Fbbt-000001-one.md"),
        Err(InboxError::InvalidName(_))
    ));
    // Non-.md extension is rejected
    assert!(matches!(
        board.read_ticket("bbt-000001-one.txt"),
        Err(InboxError::InvalidName(_))
    ));
    // Valid name but file does not exist
    assert!(matches!(
        board.read_ticket("missing.md"),
        Err(InboxError::TicketNotFound { .. })
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_tickets_dir() {
    use std::os::unix::fs::symlink;

    let (_temp, board) = fixture();
    fs::remove_dir(board.root().join("tickets")).unwrap();
    symlink(board.inbox(), board.root().join("tickets")).unwrap();

    assert!(matches!(
        board.list_tickets(),
        Err(InboxError::InvalidInput(_))
    ));
}

#[test]
fn searches_notes_with_utf8_and_ignores_out_of_scope_files() {
    let (_temp, board) = fixture();
    let mut note = input("Search Topic");
    note.done = vec!["中文 Context Hit".to_string()];
    note.validation = vec!["alpha beta".to_string()];
    let created = board.create_note(note).unwrap();
    fs::write(
        board.inbox().join("2026-05-04-agent-topic.md"),
        "Context Hit",
    )
    .unwrap();
    fs::write(board.inbox().join("notes.txt"), "Context Hit").unwrap();
    fs::create_dir_all(board.inbox().join("nested.json")).unwrap();

    let result = board.search_notes("context").unwrap();
    assert_eq!(
        result.matches,
        vec![NoteSearchMatch {
            filename: created.name.clone(),
            path: format!("inbox/{}", created.name),
            line: 10,
            snippet: "- 中文 Context Hit".to_string(),
        }]
    );

    assert!(matches!(
        board.search_notes("   "),
        Err(InboxError::InvalidInput(_))
    ));

    assert!(board.search_notes(" nomatch ").unwrap().matches.is_empty());
}

#[test]
fn searches_tickets_by_status_order_without_mutating_files() {
    let (_temp, board) = fixture();
    let active = board.root().join("tickets/bbt-000001-one.md");
    let done = board.root().join("tickets/bbd-000003-done.md");
    fs::write(&active, "first\nAlpha keyword\nlast").unwrap();
    fs::write(&done, "done KEYWORD").unwrap();
    fs::write(board.root().join("tickets/ignored.txt"), "keyword").unwrap();
    fs::create_dir_all(board.root().join("tickets/nested.md")).unwrap();

    let result = board.search_tickets("keyword").unwrap();
    assert_eq!(
        result.matches,
        vec![
            TicketSearchMatch {
                filename: "bbd-000003-done.md".to_string(),
                path: "tickets/bbd-000003-done.md".to_string(),
                status: "todo".to_string(),
                line: 1,
                snippet: "done KEYWORD".to_string(),
            },
            TicketSearchMatch {
                filename: "bbt-000001-one.md".to_string(),
                path: "tickets/bbt-000001-one.md".to_string(),
                status: "todo".to_string(),
                line: 2,
                snippet: "Alpha keyword".to_string(),
            },
        ]
    );
    assert_eq!(
        fs::read_to_string(active).unwrap(),
        "first\nAlpha keyword\nlast"
    );
    assert_eq!(fs::read_to_string(done).unwrap(), "done KEYWORD");
}

fn workspace_fixture() -> (TempDir, Workspace) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    fs::create_dir_all(root.join("projects")).unwrap();
    let workspace = Workspace::open(&root).unwrap();
    (temp, workspace)
}

#[test]
fn workspace_open_accepts_repo_root_with_dotbb_data_root() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().join("blackboard");
    let data_root = repo_root.join(".bb");
    fs::create_dir_all(data_root.join("projects")).unwrap();

    let workspace = Workspace::open(&repo_root).unwrap();

    assert_eq!(
        workspace.root(),
        crate::fs_util::canonicalize(&data_root).unwrap()
    );
    assert_eq!(
        workspace.projects_root(),
        crate::fs_util::canonicalize(&data_root.join("projects")).unwrap()
    );
    assert!(data_root.join("runtime").is_dir());
    assert!(data_root.join("blackboard.json").is_file());
}

#[test]
fn workspace_open_accepts_repo_root_with_template_data_root() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().join("blackboard");
    let data_root = repo_root.join(".bb_template");
    fs::create_dir_all(data_root.join("projects")).unwrap();

    let workspace = Workspace::open(&repo_root).unwrap();

    assert_eq!(
        workspace.root(),
        crate::fs_util::canonicalize(&data_root).unwrap()
    );
    assert_eq!(
        workspace.projects_root(),
        crate::fs_util::canonicalize(&data_root.join("projects")).unwrap()
    );
    assert!(!data_root.join("runtime").exists());
    assert!(repo_root.join(".bb/runtime").is_dir());
    assert!(data_root.join("blackboard.json").is_file());
}

#[test]
fn workspace_discovery_prefers_template_data_root_over_dotbb_and_legacy_projects() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().join("blackboard");
    fs::create_dir_all(repo_root.join("projects")).unwrap();
    fs::create_dir_all(repo_root.join(".bb/projects")).unwrap();
    fs::create_dir_all(repo_root.join(".bb_template/projects")).unwrap();
    let start = repo_root.join("bb_backend/crates");
    fs::create_dir_all(&start).unwrap();

    let workspace = Workspace::discover_from(&start).unwrap();

    assert_eq!(
        workspace.root(),
        crate::fs_util::canonicalize(&repo_root.join(".bb_template")).unwrap()
    );
}

#[test]
fn workspace_init_from_seed_copies_assets_without_runtime() {
    let temp = TempDir::new().unwrap();
    let seed_repo = temp.path().join("seed");
    let seed_data = seed_repo.join(".bb_template");
    fs::create_dir_all(seed_data.join("agents/prompts")).unwrap();
    fs::create_dir_all(seed_data.join("projects/blackboard/inbox")).unwrap();
    fs::create_dir_all(seed_data.join("projects/blackboard/tickets")).unwrap();
    fs::create_dir_all(seed_data.join("projects/blackboard/wiki")).unwrap();
    fs::create_dir_all(seed_data.join("config")).unwrap();
    fs::create_dir_all(seed_data.join("task_graphs/system")).unwrap();
    fs::create_dir_all(seed_data.join("schemas")).unwrap();
    fs::create_dir_all(seed_data.join("templates")).unwrap();
    fs::create_dir_all(seed_data.join("runtime/task_graph_runs")).unwrap();
    fs::create_dir_all(seed_repo.join("scripts")).unwrap();
    fs::write(
        seed_data.join("config/task_graph_runner.toml"),
        "node_timeout_secs = 900",
    )
    .unwrap();
    fs::write(seed_data.join("schemas/ticket.schema.json"), "{}").unwrap();
    fs::write(seed_data.join("templates/legacy-template.json"), "{}").unwrap();
    fs::write(seed_data.join("runtime/seed-state.json"), "do not copy").unwrap();
    fs::write(seed_repo.join("scripts/check_ticket_ids.py"), "script").unwrap();
    fs::write(
        seed_data.join("blackboard.json"),
        r#"{"schema_version":1,"layout":"seed"}"#,
    )
    .unwrap();
    fs::write(
        seed_data.join("projects/__projects__.json"),
        r#"{"projects":{},"machines":{}}"#,
    )
    .unwrap();

    let target = temp.path().join("user-data");
    let workspace = Workspace::init_from_seed(&target, &seed_repo).unwrap();

    assert_eq!(
        workspace.root(),
        crate::fs_util::canonicalize(&target.join(".bb")).unwrap()
    );
    assert!(target.join(".bb/schemas/ticket.schema.json").is_file());
    assert!(target.join(".bb/config/task_graph_runner.toml").is_file());
    assert!(target.join(".bb/scripts/check_ticket_ids.py").is_file());
    assert!(target.join(".bb/projects/__projects__.json").is_file());
    assert!(!target.join(".bb/templates").exists());
    assert!(target.join(".bb/runtime").is_dir());
    assert!(!target.join(".bb/runtime/seed-state.json").exists());
}

#[test]
fn workspace_init_project_capsule_injects_dotbb_into_non_empty_folder() {
    let temp = TempDir::new().unwrap();

    let folder = temp.path().join("Sample Code");
    fs::create_dir_all(folder.join("src")).unwrap();
    fs::write(folder.join("src/main.rs"), "fn main() {}\n").unwrap();

    let entry =
        Workspace::init_project_capsule(&folder, "Sample Code", Some("Sample Code")).unwrap();

    assert_eq!(entry.name, "Sample-Code");
    assert!(folder.join(".bb/__project__.json").is_file());
    assert!(folder.join(".bb/__tickets__.json").is_file());
    assert!(folder.join(".bb/__inbox__.json").is_file());
    assert!(folder.join(".bb/inbox").is_dir());
    assert!(folder.join(".bb/tickets").is_dir());
    assert!(folder.join(".bb/wiki").is_dir());
    assert!(!folder.join(".bb/blackboard.json").exists());
    assert!(!folder.join(".bb/projects").exists());
    assert!(!folder.join(".bb/agents").exists());
    assert!(!folder.join(".bb/task_graphs").exists());
    assert!(!folder.join(".bb/templates").exists());
    assert!(!folder.join(".bb/runtime").exists());
    assert!(!folder.join(".gitignore").exists());

    let meta = project::read_project_meta(&folder.join(".bb/__project__.json")).unwrap();
    assert_eq!(meta.name, "Sample Code");
    assert_eq!(
        meta.repos,
        vec![crate::path_to_string(
            &crate::fs_util::canonicalize(&folder).unwrap()
        )]
    );
}

#[test]
fn workspace_init_project_capsule_rejects_existing_dotbb_without_overwriting() {
    let temp = TempDir::new().unwrap();

    let folder = temp.path().join("invalid");
    fs::create_dir_all(folder.join(".bb")).unwrap();
    fs::write(folder.join(".bb/sentinel.txt"), "keep").unwrap();

    let err = Workspace::init_project_capsule(&folder, "invalid", Some("Invalid")).unwrap_err();

    assert!(err.to_string().contains("already contains .bb"));
    assert_eq!(
        fs::read_to_string(folder.join(".bb/sentinel.txt")).unwrap(),
        "keep"
    );
    assert!(!folder.join(".bb/__project__.json").exists());
}

#[test]
fn workspace_migrates_legacy_single_project_folder_workspace_to_capsule() {
    let temp = TempDir::new().unwrap();
    let folder = temp.path().join("Legacy Code");
    let legacy_project = folder.join(".bb/projects/Legacy-Code");
    fs::create_dir_all(legacy_project.join("inbox")).unwrap();
    fs::create_dir_all(legacy_project.join("tickets")).unwrap();
    fs::create_dir_all(legacy_project.join("wiki")).unwrap();
    fs::write(
        folder.join(".bb/blackboard.json"),
        r#"{"schema_version":1,"layout":"legacy-folder-workspace"}"#,
    )
    .unwrap();
    fs::write(
        folder.join(".bb/projects/__projects__.json"),
        r#"{
  "projects": {
    "Legacy-Code": {
      "uuid": "22222222-2222-4222-8222-222222222222",
      "locations": {
        "CURRENT": {
          "absolute_path": "",
          "relative_path": "./Legacy-Code"
        }
      }
    }
  },
  "machines": {
    "CURRENT": "TEST"
  }
}"#,
    )
    .unwrap();
    fs::write(
        legacy_project.join("__project__.json"),
        r#"{
  "name": "Legacy Code",
  "type": "tool",
  "repos": [],
  "description": "legacy",
  "lanes": [
    { "id": "bbt", "label": "后端", "color": "", "description": "", "status": "active" }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        legacy_project.join("__tickets__.json"),
        r#"{"current_counter":"000000","tickets":[]}"#,
    )
    .unwrap();
    fs::write(legacy_project.join("__inbox__.json"), r#"{"notes":[]}"#).unwrap();
    fs::write(legacy_project.join("inbox/hello.md"), "hello").unwrap();

    let migrated = Workspace::migrate_legacy_folder_workspace_to_capsule(&folder)
        .unwrap()
        .unwrap();

    assert_eq!(migrated.name, "Legacy-Code");
    assert!(folder.join(".bb/__project__.json").is_file());
    assert!(folder.join(".bb/__tickets__.json").is_file());
    assert!(folder.join(".bb/__inbox__.json").is_file());
    assert!(folder.join(".bb/inbox/hello.md").is_file());
    assert_eq!(
        migrated.meta.data_root,
        Some(crate::path_to_string(&folder.join(".bb")))
    );
}

fn create_project(workspace: &Workspace, name: &str, kind: &str, repos: &[&str]) {
    let project_root = workspace.projects_root().join(name);
    fs::create_dir_all(project_root.join("inbox")).unwrap();
    fs::create_dir_all(project_root.join("tickets")).unwrap();
    let repos_json = repos
        .iter()
        .map(|repo| format!("    {repo:?}"))
        .collect::<Vec<_>>()
        .join(",\n");
    // Workspace fixtures share the same default lane catalog as the
    // single-project `fixture()` helper so create_ticket validation
    // succeeds without per-test boilerplate.
    let project_json = format!(
            "{{\n  \"name\": {name:?},\n  \"type\": {kind:?},\n  \"repos\": [\n{repos_json}\n  ],\n  \"description\": \"fixture project\",\n  \"lanes\": [\n    {{ \"id\": \"bbt\", \"label\": \"后端\", \"color\": \"\", \"description\": \"\", \"status\": \"active\" }},\n    {{ \"id\": \"bbd\", \"label\": \"前端\", \"color\": \"\", \"description\": \"\", \"status\": \"active\" }},\n    {{ \"id\": \"bbp\", \"label\": \"产品\", \"color\": \"\", \"description\": \"\", \"status\": \"active\" }},\n    {{ \"id\": \"bbq\", \"label\": \"质量\", \"color\": \"\", \"description\": \"\", \"status\": \"active\" }}\n  ]\n}}",
        );
    fs::write(project_root.join("__project__.json"), project_json).unwrap();
    let mut registry = workspace.read_projects_registry().unwrap();
    registry.projects.insert(
        name.to_string(),
        ProjectRegistryEntry {
            uuid: uuid::Uuid::new_v4().to_string(),
            locations: std::collections::BTreeMap::from([(
                "CURRENT".to_string(),
                ProjectLocation {
                    absolute_path: String::new(),
                    relative_path: format!("./{name}"),
                },
            )]),
        },
    );
    workspace.write_projects_registry(&registry).unwrap();
}

#[test]
fn validate_project_name_accepts_kebab_and_rejects_unsafe_inputs() {
    assert_eq!(validate_project_name("kb").unwrap(), "kb");
    assert_eq!(
        validate_project_name("sample-product").unwrap(),
        "sample-product"
    );
    assert_eq!(validate_project_name("Sample-BB").unwrap(), "Sample-BB");
    assert!(matches!(
        validate_project_name(""),
        Err(InboxError::InvalidProjectName(_))
    ));
    assert!(matches!(
        validate_project_name(" kb"),
        Err(InboxError::InvalidProjectName(_))
    ));
    assert!(matches!(
        validate_project_name("-leading"),
        Err(InboxError::InvalidProjectName(_))
    ));
    assert!(matches!(
        validate_project_name("../escape"),
        Err(InboxError::InvalidProjectName(_))
    ));
    assert!(matches!(
        validate_project_name("dot.inside"),
        Err(InboxError::InvalidProjectName(_))
    ));
}

#[test]
fn workspace_lists_projects_with_metadata_and_skips_directories_without_project_json() {
    let (_temp, workspace) = workspace_fixture();
    create_project(&workspace, "demo", "workflow", &["./demo"]);
    create_project(
        &workspace,
        "sampletool",
        "tool",
        &["/abs/sample-tool", "/abs/sample-ui"],
    );
    // a scratch directory without __project__.json must not appear in listing
    fs::create_dir_all(workspace.projects_root().join("scratch")).unwrap();

    let projects = workspace.list_projects().unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].name, "demo");
    assert_eq!(projects[0].meta.kind, "workflow");
    assert_eq!(projects[1].name, "sampletool");
    assert_eq!(projects[1].meta.kind, "tool");
    assert_eq!(projects[1].meta.repos.len(), 2);
    assert_eq!(
        projects[0].meta.description.as_deref(),
        Some("fixture project")
    );
}

#[test]
fn workspace_lists_projects_skips_unreachable_absolute_registry_location() {
    let (temp, workspace) = workspace_fixture();
    create_project(&workspace, "demo", "workflow", &["./demo"]);

    let missing = temp.path().join("missing").join("project-data-root");
    let mut registry = workspace.read_projects_registry().unwrap();
    registry.projects.insert(
        "stale".to_string(),
        ProjectRegistryEntry {
            uuid: uuid::Uuid::new_v4().to_string(),
            locations: std::collections::BTreeMap::from([(
                "CURRENT".to_string(),
                ProjectLocation {
                    absolute_path: crate::path_to_string(&missing),
                    relative_path: String::new(),
                },
            )]),
        },
    );
    workspace.write_projects_registry(&registry).unwrap();

    let projects = workspace.list_projects().unwrap();
    let names: Vec<_> = projects
        .iter()
        .map(|project| project.name.as_str())
        .collect();
    assert_eq!(names, vec!["demo"]);
    assert!(matches!(
        workspace.open_project("stale"),
        Err(InboxError::ProjectNotFound(_))
    ));
}

#[test]
fn workspace_can_mount_project_data_root_outside_source_tree() {
    let (temp, workspace) = workspace_fixture();
    let external_root = temp.path().join("sample-harness");
    fs::create_dir_all(external_root.join("inbox")).unwrap();
    fs::create_dir_all(external_root.join("tickets")).unwrap();
    fs::create_dir_all(external_root.join("wiki")).unwrap();

    let data_root = external_root.to_string_lossy().to_string();
    let mut registry = workspace.read_projects_registry().unwrap();
    registry.projects.insert(
        "Sample-BB".to_string(),
        ProjectRegistryEntry {
            uuid: "14080830-ebb4-40ef-8228-95da445b5220".to_string(),
            locations: std::collections::BTreeMap::from([(
                "CURRENT".to_string(),
                ProjectLocation {
                    absolute_path: data_root.clone(),
                    relative_path: String::new(),
                },
            )]),
        },
    );
    workspace.write_projects_registry(&registry).unwrap();
    fs::write(
        external_root.join("__project__.json"),
        r#"{
  "name": "Sample",
  "type": "tool",
  "repos": [],
  "description": "external data",
  "lanes": [
    { "id": "bbt", "label": "后端", "color": "", "description": "", "status": "active" },
    { "id": "bbd", "label": "前端", "color": "", "description": "", "status": "active" },
    { "id": "bbp", "label": "产品", "color": "", "description": "", "status": "active" },
    { "id": "bbq", "label": "质量", "color": "", "description": "", "status": "active" }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        external_root.join("__tickets__.json"),
        "{\n  \"current_counter\": \"000000\",\n  \"tickets\": []\n}",
    )
    .unwrap();
    fs::write(
        external_root.join("__inbox__.json"),
        "{\n  \"notes\": []\n}",
    )
    .unwrap();

    let projects = workspace.list_projects().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Sample-BB");
    assert_eq!(projects[0].uuid, "14080830-ebb4-40ef-8228-95da445b5220");
    assert_eq!(projects[0].meta.name, "Sample");
    assert_eq!(
        projects[0].meta.description.as_deref(),
        Some("external data")
    );
    let canonical_data_root =
        crate::path_to_string(&crate::fs_util::canonicalize(&external_root).unwrap());
    assert_eq!(
        projects[0].meta.data_root.as_deref(),
        Some(canonical_data_root.as_str())
    );

    let board = workspace.open_project("Sample-BB").unwrap();
    assert_eq!(board.name(), "Sample-BB");
    assert_eq!(
        board.root(),
        crate::fs_util::canonicalize(&external_root).unwrap()
    );
    assert_eq!(
        board.workspace_root().unwrap(),
        workspace.root().to_path_buf()
    );

    let created = board
        .create_ticket(create_ticket_input("External Root"))
        .unwrap();
    assert_eq!(created.ticket.id, "000001");
    assert!(external_root
        .join("tickets")
        .join(&created.ticket.file_name)
        .exists());
}

#[test]
fn workspace_creates_opens_and_deduplicates_external_project_directory() {
    let (temp, workspace) = workspace_fixture();
    // The directory basename is deliberately different from the user-chosen
    // Project ID; paths must not define project identity.
    let external_root = temp.path().join("Sample.BB");
    let created = workspace
        .create_project_directory(ProjectDirectoryCreate {
            name: "aa".to_string(),
            data_root: external_root.to_string_lossy().to_string(),
            uuid: Some("14080830-ebb4-40ef-8228-95da445b5220".to_string()),
            display_name: Some("AA".to_string()),
            kind: "tool".to_string(),
            repos: vec!["C:/Work/Sample".to_string()],
            description: Some("Sample project data".to_string()),
        })
        .unwrap();

    assert_eq!(created.name, "aa");
    assert_eq!(created.meta.name, "AA");
    assert!(external_root.join("__project__.json").exists());
    assert!(external_root.join("__tickets__.json").exists());
    assert!(external_root.join("__inbox__.json").exists());
    assert!(external_root.join("inbox").is_dir());
    assert!(external_root.join("tickets").is_dir());
    assert!(external_root.join("wiki").is_dir());
    assert!(!fs::read_to_string(external_root.join("__project__.json"))
        .unwrap()
        .contains("data_root"));

    let reopened = workspace
        .open_project_directory(ProjectDirectoryOpen {
            name: "aa".to_string(),
            data_root: external_root.to_string_lossy().to_string(),
        })
        .unwrap();
    assert_eq!(reopened.name, "aa");

    let duplicate = workspace
        .open_project_directory(ProjectDirectoryOpen {
            name: "aa-copy".to_string(),
            data_root: external_root.to_string_lossy().to_string(),
        })
        .unwrap_err();
    assert!(
        matches!(duplicate, InboxError::InvalidInput(_)),
        "got: {duplicate:?}"
    );

    let name_collision = workspace
        .create_project_directory(ProjectDirectoryCreate {
            name: "aa".to_string(),
            data_root: temp
                .path()
                .join("AnotherRoot")
                .to_string_lossy()
                .to_string(),
            uuid: None,
            display_name: Some("Another".to_string()),
            kind: "tool".to_string(),
            repos: Vec::new(),
            description: None,
        })
        .unwrap_err();
    assert!(
        matches!(name_collision, InboxError::InvalidInput(_)),
        "got: {name_collision:?}"
    );
    assert!(!temp.path().join("AnotherRoot/__project__.json").exists());
}

#[test]
fn workspace_open_project_validates_name_and_returns_scoped_board() {
    let (_temp, workspace) = workspace_fixture();
    create_project(&workspace, "demo", "workflow", &[]);

    let board = workspace.open_project("demo").unwrap();
    assert_eq!(board.name(), "demo");
    assert!(board.root().ends_with("projects/demo"));
    assert!(board.inbox().ends_with("projects/demo/inbox"));

    assert!(matches!(
        workspace.open_project("missing"),
        Err(InboxError::ProjectNotFound(_))
    ));
    assert!(matches!(
        workspace.open_project("../demo"),
        Err(InboxError::InvalidProjectName(_))
    ));
    assert!(matches!(
        workspace.open_project("bad.name"),
        Err(InboxError::InvalidProjectName(_))
    ));
}

#[test]
fn ticket_ids_increment_independently_per_project() {
    let (_temp, workspace) = workspace_fixture();
    create_project(&workspace, "demo", "workflow", &[]);
    create_project(&workspace, "sampletool", "tool", &[]);

    let demo = workspace.open_project("demo").unwrap();
    let sampletool = workspace.open_project("sampletool").unwrap();

    let demo_first = demo.create_ticket(create_ticket_input("Demo One")).unwrap();
    let demo_second = demo.create_ticket(create_ticket_input("Demo Two")).unwrap();
    let sample_first = sampletool
        .create_ticket(create_ticket_input("Sample One"))
        .unwrap();

    // demo counter advances 000001 → 000002.
    assert_eq!(demo_first.ticket.id, "000001");
    assert_eq!(demo_second.ticket.id, "000002");
    // sampletool starts over at 000001 because each project has its own
    // `__tickets__.json.current_counter`.
    assert_eq!(sample_first.ticket.id, "000001");

    assert_eq!(demo.read_ticket_index().unwrap().current_counter, "000002");
    assert_eq!(
        sampletool.read_ticket_index().unwrap().current_counter,
        "000001"
    );
}

#[test]
fn workspace_open_requires_projects_directory_to_exist() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("blackboard");
    fs::create_dir_all(&root).unwrap();

    assert!(matches!(
        Workspace::open(&root),
        Err(InboxError::ProjectsRootMissing(_))
    ));
}

#[test]
fn workspace_rejects_malformed_project_json() {
    let (_temp, workspace) = workspace_fixture();
    let project_root = workspace.projects_root().join("kb");
    fs::create_dir_all(project_root.join("inbox")).unwrap();
    fs::write(project_root.join("__project__.json"), "not json").unwrap();
    let mut registry = workspace.read_projects_registry().unwrap();
    registry.projects.insert(
        "kb".to_string(),
        ProjectRegistryEntry {
            uuid: uuid::Uuid::new_v4().to_string(),
            locations: std::collections::BTreeMap::from([(
                "CURRENT".to_string(),
                ProjectLocation {
                    absolute_path: String::new(),
                    relative_path: "./kb".to_string(),
                },
            )]),
        },
    );
    workspace.write_projects_registry(&registry).unwrap();

    assert!(matches!(
        workspace.list_projects(),
        Err(InboxError::InvalidProjectMeta { .. })
    ));
}

// ─── KV frontmatter extension test suite ─────────────────────────────
//
// The tests below guard the "frontmatter is a KV map" contract introduced
// with [`FrontmatterExtra`]: create/read round-trip arbitrary keys, legacy
// fields (e.g. historical `current`) surface transparently without any
// warning, the `remove` patch channel drops precise keys on demand, and
// the core-field shadowing guard is enforced on both the create and
// update paths.

#[test]
fn create_ticket_roundtrips_arbitrary_extra_fields() {
    let (_temp, board) = fixture();
    let mut input = create_ticket_input("With Extras");
    input
        .extra
        .insert("team".to_string(), "squad-a".to_string());
    input
        .extra
        .insert("tags".to_string(), "alpha,beta".to_string());

    let result = board.create_ticket(input).unwrap();
    assert_eq!(
        result.ticket.extra.get("team").map(String::as_str),
        Some("squad-a")
    );
    assert_eq!(
        result.ticket.extra.get("tags").map(String::as_str),
        Some("alpha,beta")
    );

    let read = board.read_ticket_by_id(&result.ticket.id).unwrap();
    assert_eq!(read.extra.get("team").map(String::as_str), Some("squad-a"));
    assert_eq!(
        read.extra.get("tags").map(String::as_str),
        Some("alpha,beta")
    );
}

#[test]
fn create_ticket_requires_and_roundtrips_bdd_spec() {
    let (_temp, board) = fixture();
    let mut input = create_ticket_input("BDD Shape");
    input.spec = TicketSpec {
        summary: "用户删除内容前需要明确确认，避免误删。".to_string(),
        stories: vec![TicketStory {
            id: "22222222-2222-4222-8222-222222222222".to_string(),
            given: "用户看到一条可删除记录".to_string(),
            when: "用户点击删除按钮".to_string(),
            then: "系统先显示确认，而不是立即删除".to_string(),
            sample: Some("写具体 UI 行为，不写笼统体验优化。".to_string()),
        }],
        risks: vec![TicketRisk {
            id: "accidental_delete".to_string(),
            description: "确认行为不清晰会导致误删。".to_string(),
            mitigation: Some("确认与取消都必须有明确结果。".to_string()),
            status: None,
        }],
        progress_record: vec![TicketProgressRecord {
            at: Some("2026-05-18".to_string()),
            summary: "产品定义进入结构化 BDD ticket。".to_string(),
            evidence: vec!["用户明确要求 title/summary/stories/risks/progress_record".to_string()],
        }],
    };

    let result = board.create_ticket(input).unwrap();
    assert_eq!(
        result
            .ticket
            .spec
            .as_ref()
            .map(|spec| spec.summary.as_str()),
        Some("用户删除内容前需要明确确认，避免误删。")
    );
    assert!(!result.ticket.extra.contains_key("ticket_spec"));
    assert!(result.ticket.file_name.ends_with(".json"));

    let content = fs::read_to_string(board.root().join(&result.ticket.path)).unwrap();
    let document: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(
        document["summary"],
        "用户删除内容前需要明确确认，避免误删。"
    );
    assert_eq!(document["stories"][0]["given"], "用户看到一条可删除记录");

    let read = board.read_ticket_by_id(&result.ticket.id).unwrap();
    assert_eq!(
        read.spec
            .as_ref()
            .and_then(|spec| spec.stories.first())
            .map(|story| story.id.as_str()),
        Some("22222222-2222-4222-8222-222222222222")
    );
}

#[test]
fn create_ticket_rejects_missing_story_in_bdd_spec() {
    let (_temp, board) = fixture();
    let mut input = create_ticket_input("Invalid BDD");
    input.spec.stories.clear();

    let err = board.create_ticket(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("spec.stories requires at least one"),
        "msg: {msg}"
    );
}

#[test]
fn update_ticket_spec_migrates_legacy_markdown_to_json_ticket() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/000042-legacy.md"),
        "+++\nid = \"000042\"\nlane = \"bbt\"\ntitle = \"Legacy\"\ncreated_at = \"2026-05-04\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\n+++\n\n# 当前进展\n\nold body\n",
    )
    .unwrap();
    write_ticket_index_counter(&board, "000042");

    let updated = board
        .update_ticket(UpdateTicketInput {
            id: "000042".to_string(),
            frontmatter: Some(TicketFrontmatterPatch {
                spec: Some(ticket_spec("迁移后的结构化摘要。")),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();

    assert_eq!(
        updated
            .ticket
            .spec
            .as_ref()
            .map(|spec| spec.summary.as_str()),
        Some("迁移后的结构化摘要。")
    );
    assert_eq!(updated.ticket.file_name, "000042-legacy.json");
    assert!(!board.root().join("tickets/000042-legacy.md").exists());
    let content = fs::read_to_string(board.root().join(&updated.ticket.path)).unwrap();
    let document: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(document["summary"], "迁移后的结构化摘要。");
    assert!(document["stories"][0]["id"].as_str().unwrap().contains('-'));
}

#[test]
fn update_ticket_recovers_invalid_json_ticket_with_full_bdd_patch() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/000042-corrupt.json"),
        r#"{"schema_version":1,"id":"not-a-ticket","scope":"project","title":"Graph"}"#,
    )
    .unwrap();
    write_ticket_index_counter(&board, "000042");

    let updated = board
        .update_ticket(UpdateTicketInput {
            id: "000042".to_string(),
            frontmatter: Some(TicketFrontmatterPatch {
                title: Some("Recovered".to_string()),
                lane: Some("bbt".to_string()),
                status: Some("in_progress".to_string()),
                spec: Some(ticket_spec("恢复后的 BDD 摘要。")),
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();

    assert_eq!(updated.ticket.title, "Recovered");
    assert_eq!(updated.ticket.lane, "bbt");
    assert_eq!(
        updated
            .ticket
            .spec
            .as_ref()
            .map(|spec| spec.summary.as_str()),
        Some("恢复后的 BDD 摘要。")
    );

    let entry = board.read_ticket_by_id("000042").unwrap();
    assert!(entry.metadata_error.is_none(), "{:?}", entry.metadata_error);
    assert_eq!(entry.title.as_deref(), Some("Recovered"));
}

#[test]
fn patch_remove_drops_extra_key_without_migration() {
    let (_temp, board) = fixture();
    let mut input = create_ticket_input("Needs Cleanup");
    input
        .extra
        .insert("legacy_flag".to_string(), "remove-me".to_string());
    let created = board.create_ticket(input).unwrap();
    assert!(fs::read_to_string(board.root().join(&created.ticket.path))
        .unwrap()
        .contains("legacy_flag"));

    let updated = board
        .update_ticket(UpdateTicketInput {
            id: created.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                remove: vec!["legacy_flag".to_string()],
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();

    let content = fs::read_to_string(board.root().join(&updated.ticket.path)).unwrap();
    assert!(!content.contains("legacy_flag"));
    assert!(!updated.ticket.extra.contains_key("legacy_flag"));
}

#[test]
fn patch_rejects_core_fields_in_extra_and_remove() {
    let (_temp, board) = fixture();
    let created = board
        .create_ticket(create_ticket_input("Core Guard"))
        .unwrap();

    // extra must not shadow a core field like `title`.
    assert!(matches!(
        board.update_ticket(UpdateTicketInput {
            id: created.ticket.id.clone(),
            frontmatter: Some(TicketFrontmatterPatch {
                extra: [("title".to_string(), "nope".to_string())]
                    .into_iter()
                    .collect(),
                ..TicketFrontmatterPatch::default()
            }),
        }),
        Err(InboxError::InvalidInput(_))
    ));

    // remove must not target any core field either.
    assert!(matches!(
        board.update_ticket(UpdateTicketInput {
            id: created.ticket.id,
            frontmatter: Some(TicketFrontmatterPatch {
                remove: vec!["status".to_string()],
                ..TicketFrontmatterPatch::default()
            }),
        }),
        Err(InboxError::InvalidInput(_))
    ));
}

#[test]
fn legacy_current_field_is_transparent_extra_without_warnings() {
    let (_temp, board) = fixture();
    // Synthesize a pre-KV-migration ticket on disk: it still carries the
    // old `current = "..."` line. The post-migration server must expose
    // it via `extra.current` and must not raise any metadata warning,
    // because `current` is no longer in REQUIRED_METADATA_FIELDS.
    fs::write(
            board.root().join("tickets/bbt-000042-legacy.md"),
            "+++\nid = \"000042\"\nfamily = \"bbt\"\ntitle = \"Legacy\"\nupdated_at = \"2026-05-05\"\nstatus = \"todo\"\nassignee = \"alice\"\ncurrent = \"legacy-note\"\n+++\n\nbody",
        )
        .unwrap();
    write_ticket_index_counter(&board, "000042");

    let entry = board
        .list_tickets()
        .unwrap()
        .tickets
        .into_iter()
        .find(|entry| entry.name == "bbt-000042-legacy.md")
        .expect("legacy ticket must be listed");

    assert!(entry.metadata_error.is_none());
    assert!(entry
        .metadata_warnings
        .contains(&"missing frontmatter field `ticket_spec`".to_string()));
    assert_eq!(
        entry.extra.get("current").map(String::as_str),
        Some("legacy-note")
    );
    assert_eq!(
        entry.extra.get("assignee").map(String::as_str),
        Some("alice")
    );

    // A targeted `remove: ["current"]` patch is the blessed path for
    // sunsetting the legacy field on a case-by-case basis with no
    // migration script.
    let cleaned = board
        .update_ticket(UpdateTicketInput {
            id: "000042".to_string(),
            frontmatter: Some(TicketFrontmatterPatch {
                remove: vec!["current".to_string()],
                ..TicketFrontmatterPatch::default()
            }),
        })
        .unwrap();
    let content = fs::read_to_string(board.root().join(&cleaned.ticket.path)).unwrap();
    assert!(!content.contains("current ="));
}

// ─── Lane catalog test suite ─────────────────────────────────────────
//
// These tests pin down the contract introduced when `family` was replaced
// by user-managed lanes: lane definitions live in __project__.json, validation
// is dynamic (catalog lookup, not enum), and ticket files no longer carry
// a lane prefix in their filename.

#[test]
fn create_ticket_rejects_unknown_lane() {
    let (_temp, board) = fixture();
    let mut input = create_ticket_input("Unknown Lane");
    input.lane = "no-such-lane".to_string();
    let err = board.create_ticket(input).unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)));
    let msg = format!("{err}");
    assert!(
        msg.contains("not defined in __project__.json"),
        "msg: {msg}"
    );
}

#[test]
fn create_ticket_rejects_archived_lane() {
    let (_temp, board) = fixture();
    // Archive `bbq` and verify create_ticket can no longer cite it.
    board.archive_lane("bbq").unwrap();

    let mut input = create_ticket_input("Stale Lane");
    input.lane = "bbq".to_string();
    let err = board.create_ticket(input).unwrap_err();
    assert!(matches!(err, InboxError::InvalidInput(_)));
    let msg = format!("{err}");
    assert!(msg.contains("must cite an active lane"), "msg: {msg}");
}

#[test]
fn upsert_lane_roundtrips_through_project_json() {
    let (_temp, board) = fixture();
    let new_lane = LaneDef {
        id: "ops".to_string(),
        label: "运营".to_string(),
        color: "#0ea5e9".to_string(),
        description: "运营 / 增长 / 用户支持".to_string(),
        status: "active".to_string(),
    };
    let saved = board.upsert_lane(new_lane.clone()).unwrap();
    assert_eq!(saved, new_lane);

    // Round-trip through disk to ensure we did not just mutate in-memory.
    let reread = board.list_lanes().unwrap();
    assert!(reread.iter().any(|lane| lane.id == "ops"));

    // Updating an existing lane (same id) replaces in place rather than
    // appending a duplicate entry.
    let mut updated = new_lane.clone();
    updated.label = "Operations".to_string();
    board.upsert_lane(updated.clone()).unwrap();
    let reread = board.list_lanes().unwrap();
    let matches: Vec<_> = reread.iter().filter(|lane| lane.id == "ops").collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].label, "Operations");
}

#[test]
fn archive_lane_reports_affected_ticket_count() {
    let (_temp, board) = fixture();
    // Two tickets in lane `bbp`; archiving the lane must surface the
    // count so the UI can warn the user.
    board
        .create_ticket(CreateTicketInput {
            lane: "bbp".to_string(),
            title: "Affected One".to_string(),
            status: "todo".to_string(),
            spec: ticket_spec("Affected One"),
            attachments: Vec::new(),
            slug: None,
            extra: FrontmatterExtra::new(),
            sections: None,
        })
        .unwrap();
    board
        .create_ticket(CreateTicketInput {
            lane: "bbp".to_string(),
            title: "Affected Two".to_string(),
            status: "todo".to_string(),
            spec: ticket_spec("Affected Two"),
            attachments: Vec::new(),
            slug: None,
            extra: FrontmatterExtra::new(),
            sections: None,
        })
        .unwrap();

    let result = board.archive_lane("bbp").unwrap();
    assert_eq!(result.lane.id, "bbp");
    assert_eq!(result.lane.status, "archived");
    assert_eq!(result.affected_ticket_count, 2);
}

#[test]
fn ticket_filename_omits_lane_prefix() {
    let (_temp, board) = fixture();
    let result = board
        .create_ticket(create_ticket_input("New Shape"))
        .unwrap();
    // Post-migration filename pattern: `<id>-<slug>.md`. The lane ("bbt")
    // is recovered from the frontmatter, not the filename.
    assert!(
        result.ticket.file_name.starts_with("000001-"),
        "filename should start with id only: {}",
        result.ticket.file_name
    );
    assert!(!result.ticket.file_name.starts_with("bbt-"));
}

#[test]
fn ticket_index_counter_uses_max_ticket_id_without_counter_file() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/000011-existing.md"),
        ticket_frontmatter("000011", "bbt", "Existing", "done", "done"),
    )
    .unwrap();

    board.rebuild_ticket_index().unwrap();
    let index = board.read_ticket_index().unwrap();
    assert_eq!(index.current_counter, "000011");
}

#[test]
fn ticket_index_preserves_nonzero_persisted_counter() {
    let (_temp, board) = fixture();
    fs::write(
        board.root().join("tickets/000011-existing.md"),
        ticket_frontmatter("000011", "bbt", "Existing", "done", "done"),
    )
    .unwrap();
    write_ticket_index_counter(&board, "000042");

    let index = board.read_ticket_index().unwrap();
    assert_eq!(index.current_counter, "000042");
}

#[test]
fn legacy_family_filename_still_lists_with_legacy_prefix_tolerated() {
    let (_temp, board) = fixture();
    // Pre-migration ticket: filename uses the old `<lane>-<id>-<slug>.md`
    // shape, frontmatter only has `family`. The lister must surface it
    // (no metadata_error) and resolve `lane` from the legacy field so the
    // UI can still render it during the migration window.
    fs::write(
            board.root().join("tickets/bbt-000099-legacy.md"),
            "+++\nid = \"000099\"\nfamily = \"bbt\"\ntitle = \"Legacy\"\nstatus = \"todo\"\nupdated_at = \"2026-05-05\"\n+++\n\nbody",
        )
        .unwrap();

    let entry = board
        .list_tickets()
        .unwrap()
        .tickets
        .into_iter()
        .find(|entry| entry.name == "bbt-000099-legacy.md")
        .expect("legacy ticket must list");
    assert!(entry.metadata_error.is_none(), "{:?}", entry.metadata_error);
    // Empty `metadata_warnings` would also be acceptable; just ensure the
    // missing-`lane` warning is suppressed by the family bridge.
    assert!(
        !entry
            .metadata_warnings
            .iter()
            .any(|warning| warning.contains("missing frontmatter field `lane`")),
        "warnings: {:?}",
        entry.metadata_warnings
    );
    assert_eq!(entry.lane.as_deref(), Some("bbt"));
}

#[test]
fn validate_lane_id_enforces_pattern() {
    // Happy paths
    assert!(validate_lane_id("bbp").is_ok());
    assert!(validate_lane_id("ops").is_ok());
    assert!(validate_lane_id("growth-2026").is_ok());
    // Failure modes
    assert!(matches!(
        validate_lane_id("Ops"),
        Err(InboxError::InvalidInput(_))
    ));
    assert!(matches!(
        validate_lane_id("1ops"),
        Err(InboxError::InvalidInput(_))
    ));
    assert!(matches!(
        validate_lane_id("a"),
        Err(InboxError::InvalidInput(_))
    ));
    assert!(matches!(
        validate_lane_id("ops_team"),
        Err(InboxError::InvalidInput(_))
    ));
}
