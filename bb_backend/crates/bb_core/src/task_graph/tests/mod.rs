//! Unit tests for the task_graph module.

mod runner;

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::store::*;
    use super::super::types::*;
    use super::super::validation::*;
    use std::fs;
    use tempfile::TempDir;

    // ─── Test helpers ────────────────────────────────────────────────────────

    /// Create a minimal valid graph for testing.
    fn minimal_valid_graph() -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "test-graph".to_string(),
            scope: TaskGraphScope::Project,
            title: "Test Graph".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![
                TaskGraphNode {
                    id: "start".to_string(),
                    node_type: NodeType::Start,
                    label: "Start".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({}),
                    pins: vec![],
                },
                TaskGraphNode {
                    id: "end-success".to_string(),
                    node_type: NodeType::End,
                    label: "Success".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({ "result": "succeeded" }),
                    pins: vec![],
                },
            ],
            edges: vec![TaskGraphEdge {
                id: "start__end-success".to_string(),
                from: "start".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            }],
            layout: None,
        }
    }

    /// Setup a workspace with system graph fixture.
    fn setup_workspace() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create system graphs dir
        let sys_dir = root.join("task_graphs").join("system");
        fs::create_dir_all(&sys_dir).unwrap();

        // Copy fixture
        let fixture =
            include_str!("../../../../../../.bb_template/task_graphs/system/frontend-smoke.json");
        fs::write(sys_dir.join("frontend-smoke.json"), fixture).unwrap();

        // Create projects dir
        fs::create_dir_all(
            root.join("projects")
                .join("test-project")
                .join("task_graphs"),
        )
        .unwrap();

        tmp
    }

    // ─── System Graph Registry Tests ─────────────────────────────────────────

    #[test]
    fn test_list_system_graphs() {
        let tmp = setup_workspace();
        let graphs = list_system_graphs(tmp.path()).unwrap();
        assert_eq!(graphs.len(), 1);
        assert_eq!(graphs[0].id, "frontend-smoke");
        assert_eq!(graphs[0].scope, TaskGraphScope::System);
        assert!(graphs[0].readonly);
        assert_eq!(graphs[0].node_count, 4);
        assert_eq!(graphs[0].edge_count, 3);
    }

    #[test]
    fn test_list_system_graphs_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let graphs = list_system_graphs(tmp.path()).unwrap();
        assert!(graphs.is_empty());
    }

    #[test]
    fn test_read_system_graph() {
        let tmp = setup_workspace();
        let def = read_system_graph(tmp.path(), "frontend-smoke").unwrap();
        assert_eq!(def.id, "frontend-smoke");
        assert_eq!(def.scope, TaskGraphScope::System);
        assert_eq!(def.schema_version, 1);
        assert_eq!(def.nodes.len(), 4);
        assert_eq!(def.edges.len(), 3);
    }

    #[test]
    fn test_read_system_graph_not_found() {
        let tmp = setup_workspace();
        let result = read_system_graph(tmp.path(), "nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::NotFound { scope, id } => {
                assert_eq!(scope, "system");
                assert_eq!(id, "nonexistent");
            }
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_shell_config_defaults_deserialize() {
        let config: ShellConfig = serde_json::from_value(serde_json::json!({
            "command": "git"
        }))
        .unwrap();

        assert_eq!(config.cwd, ".");
        assert_eq!(config.args, Vec::<String>::new());
        assert!(config.env.is_empty());
        assert_eq!(config.timeout_ms, 600_000);
        assert_eq!(config.permission, ShellPermission::ReadOnly);
        assert_eq!(config.expected_exit_codes, vec![0]);
        assert_eq!(config.capture.max_bytes, 1_048_576);
        assert!(config.capture.strip_ansi);
    }

    #[test]
    fn test_shell_validation_rejects_raw_command_string() {
        let mut graph = minimal_valid_graph();
        graph.nodes.insert(
            1,
            TaskGraphNode {
                id: "shell".to_string(),
                node_type: NodeType::Shell,
                label: "Shell".to_string(),
                description: None,
                position: None,
                config: serde_json::json!({
                    "command": "git status"
                }),
                pins: vec![],
            },
        );
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__shell".to_string(),
                from: "start".to_string(),
                to: "shell".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "shell__end-success".to_string(),
                from: "shell".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];

        let errors = validate_graph(&graph);
        assert!(errors
            .iter()
            .any(|err| err.code == "raw_shell_string_not_supported"));
    }

    // ─── Project Graph Store Tests ───────────────────────────────────────────

    #[test]
    fn test_create_project_graph() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();

        let saved = save_project_graph(tmp.path(), "test-project", graph, None).unwrap();
        assert_eq!(saved.version, 1);
        assert_eq!(saved.scope, TaskGraphScope::Project);
        assert!(!saved.readonly);

        // Verify it can be read back
        let loaded = read_project_graph(tmp.path(), "test-project", "test-graph").unwrap();
        assert_eq!(loaded.id, "test-graph");
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn test_update_project_graph_version_bump() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();

        // Create
        save_project_graph(tmp.path(), "test-project", graph.clone(), None).unwrap();

        // Update
        let mut updated = graph.clone();
        updated.title = "Updated Title".to_string();
        let saved = save_project_graph(tmp.path(), "test-project", updated, Some(1)).unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(saved.title, "Updated Title");
    }

    #[test]
    fn test_update_project_graph_stale_version() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();

        save_project_graph(tmp.path(), "test-project", graph.clone(), None).unwrap();

        // Try to update with wrong expected version
        let result = save_project_graph(tmp.path(), "test-project", graph, Some(5));
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::StaleVersion { expected, found } => {
                assert_eq!(expected, 5);
                assert_eq!(found, 1);
            }
            other => panic!("Expected StaleVersion, got {:?}", other),
        }
    }

    #[test]
    fn test_create_duplicate_project_graph() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();

        save_project_graph(tmp.path(), "test-project", graph.clone(), None).unwrap();

        // Try to create again
        let result = save_project_graph(tmp.path(), "test-project", graph, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::DuplicateGraphId(id) => assert_eq!(id, "test-graph"),
            other => panic!("Expected DuplicateGraphId, got {:?}", other),
        }
    }

    #[test]
    fn test_reject_system_scope_write() {
        let tmp = setup_workspace();
        let mut graph = minimal_valid_graph();
        graph.scope = TaskGraphScope::System;

        let result = save_project_graph(tmp.path(), "test-project", graph, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::ReadonlyGraph(_) => {}
            other => panic!("Expected ReadonlyGraph, got {:?}", other),
        }
    }

    #[test]
    fn test_list_project_graphs() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();
        save_project_graph(tmp.path(), "test-project", graph, None).unwrap();

        let graphs = list_project_graphs(tmp.path(), "test-project").unwrap();
        assert_eq!(graphs.len(), 1);
        assert_eq!(graphs[0].id, "test-graph");
        assert_eq!(graphs[0].scope, TaskGraphScope::Project);
    }

    #[test]
    fn test_delete_project_graph() {
        let tmp = setup_workspace();
        let graph = minimal_valid_graph();
        save_project_graph(tmp.path(), "test-project", graph, None).unwrap();

        delete_project_graph(tmp.path(), "test-project", "test-graph").unwrap();

        let result = read_project_graph(tmp.path(), "test-project", "test-graph");
        assert!(result.is_err());
    }

    // ─── Validation Tests: Valid Graphs ──────────────────────────────────────

    #[test]
    fn test_validate_minimal_valid_graph() {
        let graph = minimal_valid_graph();
        let errors = validate_graph(&graph);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_graph_run_policy_defaults_to_serial_without_queue() {
        let policy = TaskGraphRunPolicy::default();
        assert!(!policy.allow_concurrent_runs);
        assert_eq!(policy.max_concurrent_runs, 1);
        assert_eq!(policy.effective_max_concurrent_runs(), 1);
        assert!(!policy.queue_enabled);
        assert_eq!(
            policy.max_queue_wait_ms,
            TaskGraphRunPolicy::DEFAULT_MAX_QUEUE_WAIT_MS
        );
    }

    #[test]
    fn test_validate_graph_run_policy_ranges() {
        let mut graph = minimal_valid_graph();
        graph.metadata = Some(GraphMetadata {
            tags: None,
            related_tickets: None,
            owner: None,
            created_by: None,
            updated_by: None,
            recursion_limit: None,
            interrupt_before: None,
            interrupt_after: None,
            run_policy: Some(TaskGraphRunPolicy {
                allow_concurrent_runs: true,
                max_concurrent_runs: 33,
                queue_enabled: true,
                max_queue_wait_ms: 59_000,
            }),
        });

        let errors = validate_graph(&graph);
        assert!(errors.iter().any(|error| {
            error.path == "metadata.run_policy.max_concurrent_runs" && error.code == "out_of_range"
        }));
        assert!(errors.iter().any(|error| {
            error.path == "metadata.run_policy.max_queue_wait_ms" && error.code == "out_of_range"
        }));
    }

    #[test]
    fn test_validate_system_graph_fixture() {
        let fixture =
            include_str!("../../../../../../.bb_template/task_graphs/system/frontend-smoke.json");
        let def: TaskGraphDefinition = serde_json::from_str(fixture).unwrap();
        let errors = validate_graph(&def);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_graph_source_reports_invalid_json() {
        let errors = validate_graph_source(br#"{ "id": "#).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.code == "invalid_json" && e.path == "$"));
    }

    #[test]
    fn test_validate_graph_value_reports_field_path_decode_errors() {
        let mut value = serde_json::to_value(minimal_valid_graph()).unwrap();
        value["nodes"] = serde_json::json!("not-an-array");

        let errors = validate_graph_value_at(value, "graph").unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.code == "invalid_type" && e.path == "graph.nodes"));
    }

    #[test]
    fn test_validate_graph_value_decodes_valid_graph() {
        let value = serde_json::to_value(minimal_valid_graph()).unwrap();
        let graph = validate_graph_value(value).unwrap();
        assert_eq!(graph.id, "test-graph");
    }

    // ─── Validation Tests: Invalid Graphs ────────────────────────────────────

    #[test]
    fn test_validate_invalid_graph_id() {
        let mut graph = minimal_valid_graph();
        graph.id = "INVALID_ID".to_string();
        let errors = validate_graph(&graph);
        assert!(errors
            .iter()
            .any(|e| e.code == "invalid_format" && e.path == "id"));
    }

    #[test]
    fn test_validate_no_start_node() {
        let mut graph = minimal_valid_graph();
        graph.nodes.retain(|n| n.node_type != NodeType::Start);
        graph.edges.clear();
        let errors = validate_graph(&graph);
        assert!(errors.iter().any(|e| e.code == "start_node_count"));
    }

    #[test]
    fn test_validate_no_end_node() {
        let mut graph = minimal_valid_graph();
        graph.nodes.retain(|n| n.node_type != NodeType::End);
        graph.edges.clear();
        let errors = validate_graph(&graph);
        assert!(errors.iter().any(|e| e.code == "end_node_missing"));
    }

    #[test]
    fn test_validate_edge_endpoint_not_found() {
        let mut graph = minimal_valid_graph();
        graph.edges.push(TaskGraphEdge {
            id: "bad-edge".to_string(),
            from: "nonexistent".to_string(),
            to: "end-success".to_string(),
            kind: EdgeKind::Exec,
            label: None,
            source_handle: None,
            target_handle: None,
            from_pin: None,
            to_pin: None,
        });
        let errors = validate_graph(&graph);
        assert!(errors.iter().any(|e| e.code == "endpoint_not_found"));
    }

    #[test]
    fn test_validate_start_has_incoming() {
        let mut graph = minimal_valid_graph();
        graph.edges.push(TaskGraphEdge {
            id: "bad-incoming".to_string(),
            from: "end-success".to_string(),
            to: "start".to_string(),
            kind: EdgeKind::Exec,
            label: None,
            source_handle: None,
            target_handle: None,
            from_pin: None,
            to_pin: None,
        });
        let errors = validate_graph(&graph);
        assert!(errors
            .iter()
            .any(|e| e.code == "start_has_incoming" || e.code == "end_has_outgoing"));
    }

    #[test]
    fn test_validate_illegal_cycle() {
        let mut graph = minimal_valid_graph();
        // Add a node that creates a cycle without loop
        graph.nodes.push(TaskGraphNode {
            id: "middle".to_string(),
            node_type: NodeType::Llm,
            label: "Middle".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "runtime": "codex",
                "agent": "codex",
                "model": null,
                "prompt": { "mode": "inline", "template": "test" },
                "inputs": null,
                "output": null
            }),
            pins: vec![],
        });
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__middle".to_string(),
                from: "start".to_string(),
                to: "middle".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "middle__start".to_string(),
                from: "middle".to_string(),
                to: "start".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];
        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "invalid_cycle"),
            "Expected invalid_cycle error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_unknown_runtime() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "bad-llm".to_string(),
            node_type: NodeType::Llm,
            label: "Bad LLM".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "runtime": "unknown-runtime",
                "agent": "codex",
                "model": null,
                "prompt": { "mode": "inline", "template": "test" }
            }),
            pins: vec![],
        });
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__bad-llm".to_string(),
                from: "start".to_string(),
                to: "bad-llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "bad-llm__end-success".to_string(),
                from: "bad-llm".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];
        let errors = validate_graph(&graph);
        assert!(errors.iter().any(|e| e.code == "runtime_not_found"));
    }

    #[test]
    fn test_validate_loop_missing_max_iterations() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "bad-loop".to_string(),
            node_type: NodeType::Loop,
            label: "Bad Loop".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "body_entry": "start",
                "body_exit": "end-success"
            }),
            pins: vec![],
        });
        graph.edges.push(TaskGraphEdge {
            id: "start__bad-loop".to_string(),
            from: "start".to_string(),
            to: "bad-loop".to_string(),
            kind: EdgeKind::Exec,
            label: None,
            source_handle: None,
            target_handle: None,
            from_pin: None,
            to_pin: None,
        });
        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "invalid_config"
                || e.code == "missing_loop_body_edge"
                || e.code == "missing_loop_exit_edge"),
            "Expected loop config errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_loop_max_iterations_out_of_range() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "bad-loop".to_string(),
            node_type: NodeType::Loop,
            label: "Bad Loop".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "max_iterations": 100,
                "body_entry": "start",
                "body_exit": "end-success",
                "on_max_iterations": "fail"
            }),
            pins: vec![],
        });
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__bad-loop".to_string(),
                from: "start".to_string(),
                to: "bad-loop".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "bad-loop__end-success__body".to_string(),
                from: "bad-loop".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "bad-loop__end-success__exit".to_string(),
                from: "bad-loop".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "end-success__bad-loop__return".to_string(),
                from: "end-success".to_string(),
                to: "bad-loop".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: Some("return".to_string()),
                from_pin: None,
                to_pin: None,
            },
        ];
        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "out_of_range"),
            "Expected out_of_range error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_loop_rejects_stale_body_refs_and_condition_node() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "cleanup-loop".to_string(),
            node_type: NodeType::Loop,
            label: "Cleanup Loop".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "max_iterations": 3,
                "body_entry": "old-body-node",
                "body_exit": "old-body-node",
                "condition": {
                    "input_ref": "$.nodes.old-body-node.output",
                    "op": "equals",
                    "path": "$.continue",
                    "value": true
                },
                "on_max_iterations": "succeed"
            }),
            pins: vec![],
        });
        graph.nodes.push(TaskGraphNode {
            id: "body-llm".to_string(),
            node_type: NodeType::Llm,
            label: "Body".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "runtime": "opencode",
                "agent": "bb-pm",
                "prompt": { "mode": "inline", "template": "cleanup" }
            }),
            pins: vec![],
        });
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__cleanup-loop".to_string(),
                from: "start".to_string(),
                to: "cleanup-loop".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "cleanup-loop__body-llm".to_string(),
                from: "cleanup-loop".to_string(),
                to: "body-llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "cleanup-loop__end-success".to_string(),
                from: "cleanup-loop".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];

        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "loop_endpoint_not_found"),
            "Expected missing loop endpoint error, got: {:?}",
            errors
        );
        assert!(
            errors
                .iter()
                .any(|e| e.code == "loop_body_entry_edge_mismatch"),
            "Expected body_entry edge mismatch error, got: {:?}",
            errors
        );
        assert!(
            errors
                .iter()
                .any(|e| e.code == "loop_condition_node_not_found"),
            "Expected stale condition node error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_loop_accepts_body_exit_hidden_return_config() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "cleanup-loop".to_string(),
            node_type: NodeType::Loop,
            label: "Cleanup Loop".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "max_iterations": 3,
                "body_entry": "body-llm",
                "body_exit": "body-llm",
                "condition": {
                    "input_ref": "$.nodes.body-llm.output",
                    "op": "equals",
                    "path": "$.continue",
                    "value": true
                },
                "on_max_iterations": "succeed"
            }),
            pins: vec![],
        });
        graph.nodes.push(TaskGraphNode {
            id: "body-llm".to_string(),
            node_type: NodeType::Llm,
            label: "Body".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "runtime": "opencode",
                "agent": "bb-pm",
                "prompt": { "mode": "inline", "template": "cleanup" }
            }),
            pins: vec![],
        });
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__cleanup-loop".to_string(),
                from: "start".to_string(),
                to: "cleanup-loop".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "cleanup-loop__body-llm".to_string(),
                from: "cleanup-loop".to_string(),
                to: "body-llm".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("body".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "cleanup-loop__end-success".to_string(),
                from: "cleanup-loop".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: Some("exit".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];

        let errors = validate_graph(&graph);
        assert!(
            !errors.iter().any(|e| e.code.starts_with("loop_")),
            "Expected no loop validation errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_branch_missing_edge() {
        let mut graph = minimal_valid_graph();
        graph.nodes.push(TaskGraphNode {
            id: "my-branch".to_string(),
            node_type: NodeType::Branch,
            label: "Branch".to_string(),
            description: None,
            position: None,
            config: serde_json::json!({
                "mode": "first_match",
                "input_ref": "$.nodes.start.output",
                "rules": [
                    { "id": "yes", "label": "Yes", "when": { "op": "always" } },
                    { "id": "no", "label": "No", "when": { "op": "always" } }
                ],
                "default_rule_id": "no"
            }),
            pins: vec![],
        });
        // Only provide edge for "yes" rule, missing "no"
        graph.edges = vec![
            TaskGraphEdge {
                id: "start__my-branch".to_string(),
                from: "start".to_string(),
                to: "my-branch".to_string(),
                kind: EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
            TaskGraphEdge {
                id: "my-branch__end-success__yes".to_string(),
                from: "my-branch".to_string(),
                to: "end-success".to_string(),
                kind: EdgeKind::Exec,
                label: Some("Yes".to_string()),
                source_handle: Some("rule:yes".to_string()),
                target_handle: None,
                from_pin: None,
                to_pin: None,
            },
        ];
        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "missing_branch_edge"),
            "Expected missing_branch_edge error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_validate_invalid_end_result() {
        let mut graph = minimal_valid_graph();
        // Change end node to invalid result
        graph.nodes[1].config = serde_json::json!({ "result": "invalid_result" });
        let errors = validate_graph(&graph);
        assert!(
            errors.iter().any(|e| e.code == "invalid_end_result"),
            "Expected invalid_end_result error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_graph_id_validation_function() {
        assert!(is_valid_graph_id("frontend-smoke-loop"));
        assert!(is_valid_graph_id("release-check"));
        assert!(is_valid_graph_id("ab"));
        assert!(!is_valid_graph_id("a")); // too short (must be >=2)
        assert!(!is_valid_graph_id("")); // empty
        assert!(!is_valid_graph_id("A-graph")); // uppercase
        assert!(!is_valid_graph_id("1-graph")); // starts with digit
        assert!(!is_valid_graph_id("graph_with_underscore")); // underscore
        assert!(!is_valid_graph_id(&"a".repeat(65))); // too long
    }
}

// ─── Run State Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod run_state_tests {
    use super::super::pregel::{checkpoint_config, checkpoint_metadata, PregelWrite};
    use super::super::run_state::*;
    use super::super::types::*;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    fn minimal_graph() -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "test-graph".to_string(),
            scope: TaskGraphScope::Project,
            title: "Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![
                super::super::types::TaskGraphNode {
                    id: "start".to_string(),
                    node_type: super::super::types::NodeType::Start,
                    label: "Start".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({}),
                    pins: vec![],
                },
                super::super::types::TaskGraphNode {
                    id: "end-success".to_string(),
                    node_type: super::super::types::NodeType::End,
                    label: "End".to_string(),
                    description: None,
                    position: None,
                    config: serde_json::json!({ "result": "succeeded" }),
                    pins: vec![],
                },
            ],
            edges: vec![super::super::types::TaskGraphEdge {
                id: "start__end".to_string(),
                from: "start".to_string(),
                to: "end-success".to_string(),
                kind: super::super::types::EdgeKind::Exec,
                label: None,
                source_handle: None,
                target_handle: None,
                from_pin: None,
                to_pin: None,
            }],
            layout: None,
        }
    }

    fn create_test_run(tmp: &TempDir) -> TaskGraphRun {
        let graph = minimal_graph();
        let graph_ref = GraphRef {
            scope: TaskGraphScope::Project,
            id: "test-graph".to_string(),
            version: 1,
        };
        create_run(
            tmp.path(),
            "test-project",
            graph_ref,
            &graph,
            serde_json::json!({"ticket_refs": ["000028"]}),
        )
        .unwrap()
    }

    // ── Run creation ─────────────────────────────────────────────────────────

    #[test]
    fn test_create_run_generates_directory_structure() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        assert!(run.id.starts_with("run-"));
        assert_eq!(run.project, "test-project");
        assert_eq!(run.status, RunStatus::Pending);

        // Verify directory structure
        let run_dir = tmp
            .path()
            .join("runtime/task_graph_runs/test-project")
            .join(&run.id);
        assert!(run_dir.join("run.json").exists());
        assert!(run_dir.join("graph.snapshot.json").exists());
        assert!(run_dir.join("graph.compiled.json").exists());
        assert!(run_dir.join("nodes/start.json").exists());
        assert!(run_dir.join("nodes/end-success.json").exists());
        assert!(run_dir.join("logs").is_dir());
        assert!(run_dir.join("artifacts").is_dir());
        assert!(run_dir.join("checkpoints").is_dir());
    }

    #[test]
    fn test_create_run_freezes_snapshot() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let detail = read_run_detail(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(detail.graph_snapshot.id, "test-graph");
        assert_eq!(detail.graph_snapshot.nodes.len(), 2);
    }

    #[test]
    fn test_create_run_initializes_nodes_as_idle() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let detail = read_run_detail(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(detail.nodes.len(), 2);
        for node in &detail.nodes {
            assert_eq!(node.status, NodeRunStatus::Idle);
        }
    }

    #[test]
    fn test_create_run_stores_input_in_context() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        assert_eq!(
            run.context.input,
            serde_json::json!({"ticket_refs": ["000028"]})
        );
    }

    #[test]
    fn test_pending_pregel_writes_roundtrip_and_clear() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);
        let writes = vec![PregelWrite {
            task_id: "task-000001-start-pull-start".to_string(),
            source_node_id: "start".to_string(),
            channel: "branch:to:end-success".to_string(),
            value: serde_json::json!("start"),
        }];

        write_pending_pregel_writes(tmp.path(), "test-project", &run.id, &writes).unwrap();
        let loaded = read_pending_pregel_writes(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].task_id, writes[0].task_id);

        clear_pending_pregel_writes(tmp.path(), "test-project", &run.id).unwrap();
        let loaded = read_pending_pregel_writes(tmp.path(), "test-project", &run.id).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_pregel_checkpoint_tuple_prefers_latest_superstep_checkpoint() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);
        let parent_checkpoint = run.pregel_checkpoint.clone().unwrap();
        let mut checkpoint = parent_checkpoint.clone();
        checkpoint.id = "pregel-checkpoint-000001".to_string();
        checkpoint.superstep = 1;
        checkpoint.channel_values.insert(
            "branch:to:end-success".to_string(),
            serde_json::json!("start"),
        );
        checkpoint
            .channel_versions
            .insert("branch:to:end-success".to_string(), 1);
        checkpoint.updated_channels = vec!["branch:to:end-success".to_string()];
        let parent_config = checkpoint_config(&run.id, "", parent_checkpoint.id.clone());
        let metadata = checkpoint_metadata("loop", 1, Some(&parent_config));
        let superstep_checkpoint = SuperstepCheckpoint {
            id: "checkpoint-000001".to_string(),
            run_id: run.id.clone(),
            superstep: 1,
            status: SuperstepStatus::Succeeded,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            completed_at: Some("2026-05-15T00:00:01Z".to_string()),
            graph_revision_before: 0,
            graph_revision_after: 0,
            mutation_batch_id: None,
            ready_nodes: vec!["start".to_string()],
            waiting_nodes: Vec::new(),
            node_statuses: BTreeMap::new(),
            context: run.context.clone(),
            pending_writes: Vec::new(),
            pending_effects: Vec::new(),
            pregel_checkpoint: Some(checkpoint.clone()),
            pregel_parent_config: Some(parent_config.clone()),
            pregel_checkpoint_metadata: Some(metadata.clone()),
            message: Some("checkpoint tuple test".to_string()),
        };
        write_superstep_checkpoint(tmp.path(), "test-project", &run.id, &superstep_checkpoint)
            .unwrap();
        let pending = vec![PregelWrite {
            task_id: "task-000002-end-success-pull-branch-to-end-success".to_string(),
            source_node_id: "end-success".to_string(),
            channel: "__end__".to_string(),
            value: serde_json::json!("succeeded"),
        }];
        write_pending_pregel_writes(tmp.path(), "test-project", &run.id, &pending).unwrap();

        let tuple = read_pregel_checkpoint_tuple(tmp.path(), "test-project", &run.id, "")
            .unwrap()
            .unwrap();

        assert_eq!(tuple.config.thread_id, run.id);
        assert_eq!(tuple.config.checkpoint_ns, "");
        assert_eq!(tuple.config.checkpoint_id, checkpoint.id);
        assert_eq!(tuple.checkpoint.superstep, 1);
        assert_eq!(tuple.parent_config, Some(parent_config));
        assert_eq!(tuple.metadata, metadata);
        assert_eq!(tuple.pending_writes.len(), 1);
        assert_eq!(tuple.pending_writes[0].task_id, pending[0].task_id);
    }

    // ── List runs ────────────────────────────────────────────────────────────

    #[test]
    fn test_list_runs_empty() {
        let tmp = TempDir::new().unwrap();
        let runs = list_runs(tmp.path(), "test-project").unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn test_list_runs_after_creation() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let runs = list_runs(tmp.path(), "test-project").unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);
        assert_eq!(runs[0].status, RunStatus::Pending);
    }

    #[test]
    fn test_create_queued_run_sets_queue_metadata() {
        let tmp = TempDir::new().unwrap();
        let graph = minimal_graph();
        let graph_ref = GraphRef {
            scope: TaskGraphScope::Project,
            id: "test-graph".to_string(),
            version: 1,
        };
        let queued = create_queued_run(
            tmp.path(),
            "test-project",
            graph_ref,
            &graph,
            serde_json::json!({}),
            "2026-05-16T00:30:00Z".to_string(),
        )
        .unwrap();

        assert_eq!(queued.status, RunStatus::Queued);
        assert!(queued.queued_at.is_some());
        assert_eq!(
            queued.queue_deadline_at.as_deref(),
            Some("2026-05-16T00:30:00Z")
        );

        let runs = list_runs(tmp.path(), "test-project").unwrap();
        assert_eq!(runs[0].status, RunStatus::Queued);
        assert!(runs[0].queued_at.is_some());
        assert_eq!(
            runs[0].queue_deadline_at.as_deref(),
            Some("2026-05-16T00:30:00Z")
        );
    }

    #[test]
    fn test_status_transition_queued_to_pending_and_failed() {
        let tmp = TempDir::new().unwrap();
        let graph = minimal_graph();
        let graph_ref = GraphRef {
            scope: TaskGraphScope::Project,
            id: "test-graph".to_string(),
            version: 1,
        };
        let queued = create_queued_run(
            tmp.path(),
            "test-project",
            graph_ref,
            &graph,
            serde_json::json!({}),
            "2026-05-16T00:30:00Z".to_string(),
        )
        .unwrap();

        let pending =
            update_run_status(tmp.path(), "test-project", &queued.id, RunStatus::Pending).unwrap();
        assert_eq!(pending.status, RunStatus::Pending);

        let queued = create_queued_run(
            tmp.path(),
            "test-project",
            GraphRef {
                scope: TaskGraphScope::Project,
                id: "test-graph".to_string(),
                version: 1,
            },
            &graph,
            serde_json::json!({}),
            "2026-05-16T00:30:00Z".to_string(),
        )
        .unwrap();
        let failed =
            update_run_status(tmp.path(), "test-project", &queued.id, RunStatus::Failed).unwrap();
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.completed_at.is_some());
    }

    #[test]
    fn test_queued_run_timeout_event_is_durable() {
        let tmp = TempDir::new().unwrap();
        let graph = minimal_graph();
        let queued = create_queued_run(
            tmp.path(),
            "test-project",
            GraphRef {
                scope: TaskGraphScope::Project,
                id: "test-graph".to_string(),
                version: 1,
            },
            &graph,
            serde_json::json!({}),
            "2026-05-16T00:30:00Z".to_string(),
        )
        .unwrap();

        update_run_status(tmp.path(), "test-project", &queued.id, RunStatus::Failed).unwrap();
        append_run_event(
            tmp.path(),
            "test-project",
            &queued.id,
            0,
            "run_queue_timeout",
            None,
            "Queued run exceeded max queue wait and was failed",
            serde_json::json!({ "queue_deadline_at": queued.queue_deadline_at }),
        )
        .unwrap();

        let events = list_run_events(tmp.path(), "test-project", &queued.id).unwrap();
        assert!(events.iter().any(|event| event.kind == "run_queue_timeout"));
    }

    // ── Read run ─────────────────────────────────────────────────────────────

    #[test]
    fn test_read_run_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = read_run_detail(tmp.path(), "test-project", "nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::RunNotFound { project, run_id } => {
                assert_eq!(project, "test-project");
                assert_eq!(run_id, "nonexistent");
            }
            other => panic!("Expected RunNotFound, got {:?}", other),
        }
    }

    // ── Status transitions ───────────────────────────────────────────────────

    #[test]
    fn test_status_transition_pending_to_running() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let updated =
            update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        assert_eq!(updated.status, RunStatus::Running);
        assert!(updated.started_at.is_some());
    }

    #[test]
    fn test_status_transition_running_to_succeeded() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        let updated =
            update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Succeeded).unwrap();
        assert_eq!(updated.status, RunStatus::Succeeded);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_status_transition_running_to_paused() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        let updated =
            update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Paused).unwrap();
        assert_eq!(updated.status, RunStatus::Paused);
    }

    #[test]
    fn test_status_transition_paused_to_running() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Paused).unwrap();
        let updated =
            update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        assert_eq!(updated.status, RunStatus::Running);
    }

    #[test]
    fn test_invalid_status_transition() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        // Pending -> Succeeded is invalid (must go through Running)
        let result = update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Succeeded);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskGraphError::InvalidRunTransition { from, to } => {
                assert_eq!(from, "pending");
                assert_eq!(to, "succeeded");
            }
            other => panic!("Expected InvalidRunTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_backward_transition() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();
        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Succeeded).unwrap();

        // Succeeded -> Running is invalid
        let result = update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running);
        assert!(result.is_err());
    }

    // ── Node state ───────────────────────────────────────────────────────────

    #[test]
    fn test_update_node_state() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let node_state = TaskGraphRunNode {
            node_id: "start".to_string(),
            status: NodeRunStatus::Running,
            started_at: Some("2026-05-09T01:00:00Z".to_string()),
            completed_at: None,
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: None,
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        };

        update_node_state(tmp.path(), "test-project", &run.id, &node_state).unwrap();

        let detail = read_run_detail(tmp.path(), "test-project", &run.id).unwrap();
        let start_node = detail.nodes.iter().find(|n| n.node_id == "start").unwrap();
        assert_eq!(start_node.status, NodeRunStatus::Running);
        assert_eq!(
            start_node.started_at.as_deref(),
            Some("2026-05-09T01:00:00Z")
        );
    }

    #[test]
    fn test_update_node_state_with_error() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let node_state = TaskGraphRunNode {
            node_id: "start".to_string(),
            status: NodeRunStatus::Failed,
            started_at: Some("2026-05-09T01:00:00Z".to_string()),
            completed_at: Some("2026-05-09T01:00:05Z".to_string()),
            duration_ms: Some(5000),
            iteration: None,
            exit_code: Some(1),
            error: Some(NodeError {
                code: "exec_failed".to_string(),
                message: "Process exited with code 1".to_string(),
            }),
            output_artifact: None,
            log_tail: Some("error: something went wrong".to_string()),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        };

        update_node_state(tmp.path(), "test-project", &run.id, &node_state).unwrap();

        let detail = read_run_detail(tmp.path(), "test-project", &run.id).unwrap();
        let start_node = detail.nodes.iter().find(|n| n.node_id == "start").unwrap();
        assert_eq!(start_node.status, NodeRunStatus::Failed);
        assert!(start_node.error.is_some());
        assert_eq!(start_node.error.as_ref().unwrap().code, "exec_failed");
    }

    // ── Log append ───────────────────────────────────────────────────────────

    #[test]
    fn test_append_node_log() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        append_node_log(tmp.path(), "test-project", &run.id, "start", "Line 1").unwrap();
        append_node_log(tmp.path(), "test-project", &run.id, "start", "Line 2").unwrap();

        let run_dir = tmp
            .path()
            .join("runtime/task_graph_runs/test-project")
            .join(&run.id);
        let log_content = fs::read_to_string(run_dir.join("logs/start.log")).unwrap();
        assert!(log_content.contains("Line 1"));
        assert!(log_content.contains("Line 2"));
        assert_eq!(log_content.lines().count(), 2);
    }

    // ── Artifact write ───────────────────────────────────────────────────────

    #[test]
    fn test_write_artifact_json() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let artifact = write_artifact(
            tmp.path(),
            "test-project",
            &run.id,
            "plan-output",
            r#"{"steps": ["step1", "step2"]}"#,
            ArtifactContentType::Json,
        )
        .unwrap();

        assert_eq!(artifact.id, "plan-output");
        assert_eq!(artifact.path, "artifacts/plan-output.json");
        assert_eq!(artifact.content_type, ArtifactContentType::Json);

        let run_dir = tmp
            .path()
            .join("runtime/task_graph_runs/test-project")
            .join(&run.id);
        assert!(run_dir.join("artifacts/plan-output.json").exists());
    }

    #[test]
    fn test_write_artifact_markdown() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let artifact = write_artifact(
            tmp.path(),
            "test-project",
            &run.id,
            "fix-suggestion",
            "# Fix\n\nApply patch X.",
            ArtifactContentType::Markdown,
        )
        .unwrap();

        assert_eq!(artifact.path, "artifacts/fix-suggestion.md");
        assert_eq!(artifact.content_type, ArtifactContentType::Markdown);
    }

    // ── Branch decision ──────────────────────────────────────────────────────

    #[test]
    fn test_record_branch_decision() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let decision = BranchDecision {
            node_id: "failure-branch".to_string(),
            selected_rule_id: "has-failures".to_string(),
            selected_edge_id: "failure-branch__fix-loop__has-failures".to_string(),
            evaluated_at: "2026-05-09T01:03:00Z".to_string(),
        };

        record_branch_decision(tmp.path(), "test-project", &run.id, decision).unwrap();

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(loaded.context.branch_decisions.len(), 1);
        assert_eq!(
            loaded.context.branch_decisions[0].selected_rule_id,
            "has-failures"
        );
    }

    #[test]
    fn test_record_multiple_branch_decisions() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        for i in 0..3 {
            let decision = BranchDecision {
                node_id: format!("branch-{}", i),
                selected_rule_id: "rule-a".to_string(),
                selected_edge_id: format!("edge-{}", i),
                evaluated_at: format!("2026-05-09T01:0{}:00Z", i),
            };
            record_branch_decision(tmp.path(), "test-project", &run.id, decision).unwrap();
        }

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(loaded.context.branch_decisions.len(), 3);
    }

    // ── Loop iteration ───────────────────────────────────────────────────────

    #[test]
    fn test_record_loop_iteration() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let state = LoopIterationState {
            loop_node_id: "fix-loop".to_string(),
            current_iteration: 1,
            max_iterations: 3,
            exit_reason: None,
            history: vec![LoopIterationEntry {
                iteration: 1,
                started_at: "2026-05-09T01:03:01Z".to_string(),
                completed_at: None,
                result: LoopIterationResult::Continued,
            }],
        };

        record_loop_iteration(tmp.path(), "test-project", &run.id, state).unwrap();

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(loaded.context.loop_iterations.len(), 1);
        assert_eq!(loaded.context.loop_iterations[0].current_iteration, 1);
    }

    #[test]
    fn test_record_loop_iteration_upsert() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        // Record iteration 1
        let state1 = LoopIterationState {
            loop_node_id: "fix-loop".to_string(),
            current_iteration: 1,
            max_iterations: 3,
            exit_reason: None,
            history: vec![LoopIterationEntry {
                iteration: 1,
                started_at: "2026-05-09T01:03:01Z".to_string(),
                completed_at: Some("2026-05-09T01:03:30Z".to_string()),
                result: LoopIterationResult::Continued,
            }],
        };
        record_loop_iteration(tmp.path(), "test-project", &run.id, state1).unwrap();

        // Upsert with iteration 2
        let state2 = LoopIterationState {
            loop_node_id: "fix-loop".to_string(),
            current_iteration: 2,
            max_iterations: 3,
            exit_reason: None,
            history: vec![
                LoopIterationEntry {
                    iteration: 1,
                    started_at: "2026-05-09T01:03:01Z".to_string(),
                    completed_at: Some("2026-05-09T01:03:30Z".to_string()),
                    result: LoopIterationResult::Continued,
                },
                LoopIterationEntry {
                    iteration: 2,
                    started_at: "2026-05-09T01:03:31Z".to_string(),
                    completed_at: None,
                    result: LoopIterationResult::Continued,
                },
            ],
        };
        record_loop_iteration(tmp.path(), "test-project", &run.id, state2).unwrap();

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        // Should still be 1 entry (upserted, not appended)
        assert_eq!(loaded.context.loop_iterations.len(), 1);
        assert_eq!(loaded.context.loop_iterations[0].current_iteration, 2);
        assert_eq!(loaded.context.loop_iterations[0].history.len(), 2);
    }

    #[test]
    fn test_record_loop_exit_reason() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        let state = LoopIterationState {
            loop_node_id: "fix-loop".to_string(),
            current_iteration: 3,
            max_iterations: 3,
            exit_reason: Some("max_iterations_reached".to_string()),
            history: vec![
                LoopIterationEntry {
                    iteration: 1,
                    started_at: "2026-05-09T01:03:01Z".to_string(),
                    completed_at: Some("2026-05-09T01:03:30Z".to_string()),
                    result: LoopIterationResult::Continued,
                },
                LoopIterationEntry {
                    iteration: 2,
                    started_at: "2026-05-09T01:03:31Z".to_string(),
                    completed_at: Some("2026-05-09T01:04:00Z".to_string()),
                    result: LoopIterationResult::Continued,
                },
                LoopIterationEntry {
                    iteration: 3,
                    started_at: "2026-05-09T01:04:01Z".to_string(),
                    completed_at: Some("2026-05-09T01:04:30Z".to_string()),
                    result: LoopIterationResult::Exited,
                },
            ],
        };
        record_loop_iteration(tmp.path(), "test-project", &run.id, state).unwrap();

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(
            loaded.context.loop_iterations[0].exit_reason.as_deref(),
            Some("max_iterations_reached")
        );
    }

    // ── Node output ──────────────────────────────────────────────────────────

    #[test]
    fn test_set_node_output() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        set_node_output(
            tmp.path(),
            "test-project",
            &run.id,
            "start",
            serde_json::json!({"plan": "do things"}),
        )
        .unwrap();

        let loaded = read_run(tmp.path(), "test-project", &run.id).unwrap();
        assert!(loaded.context.node_outputs.contains_key("start"));
        assert_eq!(
            loaded.context.node_outputs["start"],
            serde_json::json!({"plan": "do things"})
        );
    }

    // ── Human gate pause ─────────────────────────────────────────────────────

    #[test]
    fn test_set_run_paused() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();

        let paused = RunPaused {
            node_id: "human-approval".to_string(),
            reason: "waiting_for_human_gate".to_string(),
            actions: vec![
                PausedAction {
                    id: "resume".to_string(),
                    label: "继续".to_string(),
                    result: "resume".to_string(),
                },
                PausedAction {
                    id: "reject".to_string(),
                    label: "终止".to_string(),
                    result: "cancel".to_string(),
                },
            ],
        };

        let updated = set_run_paused(tmp.path(), "test-project", &run.id, paused).unwrap();
        assert_eq!(updated.status, RunStatus::Paused);
        assert!(updated.paused.is_some());
        assert_eq!(updated.paused.as_ref().unwrap().node_id, "human-approval");
        assert_eq!(updated.paused.as_ref().unwrap().actions.len(), 2);
    }

    // ── Full detail aggregation ──────────────────────────────────────────────

    #[test]
    fn test_read_run_detail_aggregates_all() {
        let tmp = TempDir::new().unwrap();
        let run = create_test_run(&tmp);

        // Update run to running
        update_run_status(tmp.path(), "test-project", &run.id, RunStatus::Running).unwrap();

        // Update start node
        let node_state = TaskGraphRunNode {
            node_id: "start".to_string(),
            status: NodeRunStatus::Succeeded,
            started_at: Some("2026-05-09T01:00:00Z".to_string()),
            completed_at: Some("2026-05-09T01:00:01Z".to_string()),
            duration_ms: Some(1000),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some("done".to_string()),
            child_run_id: None,
            runtime: None,
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        };
        update_node_state(tmp.path(), "test-project", &run.id, &node_state).unwrap();

        // Set node output
        set_node_output(
            tmp.path(),
            "test-project",
            &run.id,
            "start",
            serde_json::json!({"ok": true}),
        )
        .unwrap();

        // Read full detail
        let detail = read_run_detail(tmp.path(), "test-project", &run.id).unwrap();
        assert_eq!(detail.run.status, RunStatus::Running);
        assert_eq!(detail.graph_snapshot.id, "test-graph");
        assert_eq!(detail.nodes.len(), 2);

        let start = detail.nodes.iter().find(|n| n.node_id == "start").unwrap();
        assert_eq!(start.status, NodeRunStatus::Succeeded);
        assert_eq!(start.log_tail.as_deref(), Some("done"));

        assert!(detail.run.context.node_outputs.contains_key("start"));
    }
}
