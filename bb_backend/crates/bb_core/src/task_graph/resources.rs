//! Graph-level resource resolution.
//!
//! Resources are declared at the `TaskGraphDefinition` level and resolved once
//! at run start. The resolved snapshot is stored in `RunContext.resources` and
//! remains frozen for the entire run lifetime.

use std::collections::BTreeMap;
use std::path::Path;

use crate::task_graph::definition::types::{GraphResource, TaskGraphDefinition};
use crate::task_graph::nodes::eval;
use crate::task_graph::run_state::RunContext;

/// Resolve all graph-level resources using the current run context.
///
/// Template expressions in resource values (e.g. `{{env.workspace}}`, `{{inputs.xxx}}`)
/// are resolved against the provided context. The result is a frozen BTreeMap that
/// should be stored in `RunContext.resources`.
pub fn resolve_graph_resources(
    graph: &TaskGraphDefinition,
    project: &str,
    workspace_root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
) -> BTreeMap<String, serde_json::Value> {
    let Some(resources) = &graph.resources else {
        return BTreeMap::new();
    };

    let mut resolved = BTreeMap::new();
    for (id, resource) in resources {
        let value = resolve_resource_value(resource, project, workspace_root, scripts_dir, context);
        resolved.insert(id.clone(), value);
    }
    resolved
}

/// Resolve a single resource value, recursively processing template expressions.
fn resolve_resource_value(
    resource: &GraphResource,
    project: &str,
    workspace_root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
) -> serde_json::Value {
    resolve_value_tree(&resource.value, project, workspace_root, scripts_dir, context)
}

/// Recursively resolve template expressions in a JSON value tree.
fn resolve_value_tree(
    value: &serde_json::Value,
    project: &str,
    workspace_root: &Path,
    scripts_dir: &Path,
    context: &RunContext,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(raw) => {
            let trimmed = raw.trim();
            // 如果整个字符串是一个 mustache 表达式，尝试保留原始类型
            if let Some(expr) = full_mustache_expr(trimmed) {
                if let Some(resolved) = eval::resolve_template_expr_as_value(
                    expr,
                    project,
                    workspace_root,
                    scripts_dir,
                    context,
                ) {
                    return resolved;
                }
            }
            // 否则做字符串级别的模板渲染
            serde_json::Value::String(eval::render_prompt_template(
                raw,
                project,
                workspace_root,
                scripts_dir,
                context,
                None,
            ))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|v| resolve_value_tree(v, project, workspace_root, scripts_dir, context))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        resolve_value_tree(v, project, workspace_root, scripts_dir, context),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn full_mustache_expr(raw: &str) -> Option<&str> {
    if raw.starts_with("{{") && raw.ends_with("}}") {
        return Some(raw.trim_start_matches("{{").trim_end_matches("}}").trim());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_graph::definition::types::{GraphResource, PinValueType, TaskGraphDefinition, TaskGraphScope};
    use serde_json::json;

    fn minimal_graph_with_resources(
        resources: BTreeMap<String, GraphResource>,
    ) -> TaskGraphDefinition {
        TaskGraphDefinition {
            schema_version: 1,
            id: "test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            resources: Some(resources),
            nodes: vec![],
            edges: vec![],
            layout: None,
        }
    }

    fn empty_context_with_input(input: serde_json::Value) -> RunContext {
        RunContext {
            input,
            node_outputs: serde_json::Map::new(),
            branch_decisions: Vec::new(),
            loop_iterations: Vec::new(),
            loop_stack: Vec::new(),
            completed_branches: std::collections::HashMap::new(),
            resources: BTreeMap::new(),
        }
    }

    #[test]
    fn resolves_static_resource() {
        let mut resources = BTreeMap::new();
        resources.insert(
            "config".to_string(),
            GraphResource {
                value_type: PinValueType::Json,
                description: None,
                value: json!({ "key": "value", "count": 42 }),
            },
        );
        let graph = minimal_graph_with_resources(resources);
        let ctx = empty_context_with_input(json!({}));
        let tmp = std::env::temp_dir();

        let resolved = resolve_graph_resources(&graph, "test-project", &tmp, &tmp, &ctx);

        assert_eq!(resolved["config"], json!({ "key": "value", "count": 42 }));
    }

    #[test]
    fn resolves_template_in_resource_value() {
        let mut resources = BTreeMap::new();
        resources.insert(
            "paths".to_string(),
            GraphResource {
                value_type: PinValueType::Json,
                description: None,
                value: json!({
                    "workspace": "{{env.workspace}}",
                    "project": "{{env.project}}"
                }),
            },
        );
        let graph = minimal_graph_with_resources(resources);
        let ctx = empty_context_with_input(json!({}));
        let tmp = std::env::temp_dir();

        let resolved = resolve_graph_resources(&graph, "my-project", &tmp, &tmp, &ctx);

        let paths = &resolved["paths"];
        assert_eq!(paths["project"], "my-project");
        // workspace 应该被解析为 tmp 路径
        assert!(!paths["workspace"].as_str().unwrap().contains("{{"));
    }

    #[test]
    fn resolves_inputs_template_in_resource() {
        let mut resources = BTreeMap::new();
        resources.insert(
            "bundle".to_string(),
            GraphResource {
                value_type: PinValueType::Json,
                description: None,
                value: json!({
                    "ticket": "{{inputs.ticket_path}}"
                }),
            },
        );
        let graph = minimal_graph_with_resources(resources);
        let ctx = empty_context_with_input(json!({ "ticket_path": "/path/to/ticket.md" }));
        let tmp = std::env::temp_dir();

        let resolved = resolve_graph_resources(&graph, "test", &tmp, &tmp, &ctx);

        assert_eq!(resolved["bundle"]["ticket"], "/path/to/ticket.md");
    }

    #[test]
    fn empty_resources_returns_empty_map() {
        let graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".to_string(),
            scope: TaskGraphScope::Project,
            title: "Test".to_string(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            resources: None,
            nodes: vec![],
            edges: vec![],
            layout: None,
        };
        let ctx = empty_context_with_input(json!({}));
        let tmp = std::env::temp_dir();

        let resolved = resolve_graph_resources(&graph, "test", &tmp, &tmp, &ctx);

        assert!(resolved.is_empty());
    }
}
