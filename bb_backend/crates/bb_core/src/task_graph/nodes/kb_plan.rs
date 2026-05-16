use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::task_graph::definition::types::{TaskGraphError, TaskGraphNode};
use crate::task_graph::nodes::{eval, kb_staging};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{NodeRunStatus, TaskGraphRun, TaskGraphRunNode};

#[derive(Debug, Clone, Deserialize)]
struct KbPlanConfig {
    mode: KbPlanMode,
    #[serde(default)]
    inputs: Option<Value>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KbPlanMode {
    WikiPlan,
    WriterPlan,
}

pub(crate) fn execute_kb_plan_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: KbPlanConfig =
        serde_json::from_value(node.config.clone()).map_err(|source| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source,
        })?;
    let inputs = eval::resolve_node_inputs(
        config.inputs.as_ref(),
        &opts.project,
        &opts.workspace_root,
        &opts.scripts_dir,
        &run.context,
    );

    let output = match config.mode {
        KbPlanMode::WikiPlan => wiki_plan(&inputs, &opts.workspace_root)?,
        KbPlanMode::WriterPlan => writer_plan(&inputs, &opts.workspace_root)?,
    };

    let now = Utc::now().to_rfc3339();
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(output),
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: Some(0),
            error: None,
            output_artifact: None,
            log_tail: Some(format!(
                "Generated {} deterministically",
                mode_name(config.mode)
            )),
            child_run_id: None,
            runtime: Some("kb_plan".to_string()),
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

fn wiki_plan(
    inputs: &eval::NodeInputs,
    workspace_root: &std::path::Path,
) -> Result<Value, TaskGraphError> {
    let manifest_input = inputs.get("manifest").unwrap_or(&Value::Null);
    let manifest = read_manifest(manifest_input)?;
    let kb_output_dir = kb_staging::kb_output_dir_from_inputs(inputs, workspace_root)
        .unwrap_or_else(|| manifest_kb_output_dir(manifest_input, workspace_root));
    let staging_dir = kb_staging::staging_dir(&kb_output_dir);
    let wiki_plan_path = staging_dir.join("wiki-plan.json");
    let manifest_path = manifest_path(manifest_input);
    let language =
        kb_staging::string_input(inputs, "language").unwrap_or_else(|| "zh-CN".to_string());
    let h2_contract = h2_contract(&manifest);
    let source_anchor_ids = source_anchor_ids(&manifest);

    let pages = vec![
        page(
            "index",
            "index.md",
            "Topology Mutation KB Index",
            &h2_contract,
            &source_anchor_ids,
        ),
        page(
            "topology-mutation",
            "topology-mutation.md",
            "Topology Mutation Runtime",
            &h2_contract,
            &source_anchor_ids,
        ),
    ];

    let output = json!({
        "wiki_plan_version": 1,
        "kb_output_dir": kb_staging::path_string(&kb_output_dir),
        "language": language,
        "staging": {
            "dir": kb_staging::path_string(&staging_dir),
            "manifest_path": manifest_path,
            "wiki_plan_path": kb_staging::path_string(&wiki_plan_path),
            "scan_paths": manifest_input
                .get("staging")
                .and_then(|staging| staging.get("scan_paths"))
                .cloned()
                .unwrap_or_else(|| json!({})),
        },
        "documents": document_refs(manifest_input),
        "pages": pages,
        "writer_batches": [
            {
                "id": "writer-index",
                "page_ids": ["index"],
                "goal": "write the KB index and navigation summary"
            },
            {
                "id": "writer-topology-mutation",
                "page_ids": ["topology-mutation"],
                "goal": "write the topology mutation implementation page with source anchors"
            }
        ]
    });
    kb_staging::write_json_file(&wiki_plan_path, &output)?;
    Ok(output)
}

fn writer_plan(
    inputs: &eval::NodeInputs,
    workspace_root: &std::path::Path,
) -> Result<Value, TaskGraphError> {
    let wiki_plan = inputs.get("wiki_plan").unwrap_or(&Value::Null);
    let manifest_input = inputs.get("manifest").unwrap_or(&Value::Null);
    let kb_output_dir = kb_staging::kb_output_dir_from_inputs(inputs, workspace_root)
        .unwrap_or_else(|| manifest_kb_output_dir(manifest_input, workspace_root));
    let staging_dir = kb_staging::staging_dir(&kb_output_dir);
    let writer_plan_path = staging_dir.join("writer-plan.json");
    let pages = wiki_plan
        .get("pages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut writers = Vec::new();
    for page in pages {
        let Some(page_id) = page.get("id").and_then(Value::as_str) else {
            continue;
        };
        let path = page
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("index.md");
        let title = page.get("title").and_then(Value::as_str).unwrap_or(page_id);
        writers.push(json!({
            "id": format!("writer-{}", page_id.replace('_', "-")),
            "page_ids": [page_id],
            "target_paths": [path],
            "goal": format!("Write `{title}` at `{path}` using .staging manifest, wiki_plan, h2_contract, and source anchors."),
        }));
    }

    let output = json!({
        "staging": {
            "dir": kb_staging::path_string(&staging_dir),
            "manifest_path": manifest_path(manifest_input),
            "wiki_plan_path": kb_staging::nested_string(Some(wiki_plan), &["staging", "wiki_plan_path"]),
            "writer_plan_path": kb_staging::path_string(&writer_plan_path),
            "scan_paths": manifest_input
                .get("staging")
                .and_then(|staging| staging.get("scan_paths"))
                .cloned()
                .unwrap_or_else(|| json!({})),
        },
        "documents": document_refs(manifest_input),
        "writer_plan": {
            "writers": writers
        }
    });
    kb_staging::write_json_file(&writer_plan_path, &output)?;
    Ok(output)
}

fn page(id: &str, path: &str, title: &str, h2_contract: &[String], anchors: &[String]) -> Value {
    json!({
        "id": id,
        "path": path,
        "title": title,
        "h2_contract": h2_contract,
        "source_anchor_ids": anchors,
        "dependencies": [],
    })
}

fn h2_contract(manifest: &Value) -> Vec<String> {
    let sections = manifest
        .get("module_sections")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut values = sections
        .iter()
        .filter_map(|section| section.get("title").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .take(12)
        .collect::<Vec<_>>();
    if values.is_empty() {
        values = vec![
            "Overview".to_string(),
            "Core Data Structures".to_string(),
            "Mutation Operations".to_string(),
            "Superstep Semantics".to_string(),
            "Revision And Checkpoint Migration".to_string(),
            "Conflict Handling".to_string(),
            "Runtime Nodes".to_string(),
            "Tests".to_string(),
            "Risks".to_string(),
        ];
    }
    values
}

fn source_anchor_ids(manifest: &Value) -> Vec<String> {
    manifest
        .get("source_anchors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|anchor| anchor.get("id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .take(20)
        .collect()
}

fn read_manifest(manifest_input: &Value) -> Result<Value, TaskGraphError> {
    if let Some(path) = manifest_input
        .get("staging")
        .and_then(|staging| staging.get("manifest_path"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
    {
        return kb_staging::read_json_file(&path);
    }
    Ok(manifest_input.clone())
}

fn manifest_path(manifest_input: &Value) -> Option<String> {
    manifest_input
        .get("staging")
        .and_then(|staging| staging.get("manifest_path"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn manifest_kb_output_dir(
    manifest_input: &Value,
    workspace_root: &std::path::Path,
) -> std::path::PathBuf {
    manifest_input
        .get("staging")
        .and_then(|staging| staging.get("kb_output_dir"))
        .and_then(Value::as_str)
        .map(|raw| kb_staging::resolve_path(raw, workspace_root))
        .unwrap_or_else(|| workspace_root.join(".bb_template/runtime/kb-wiki-output"))
}

fn document_refs(manifest_input: &Value) -> Vec<Value> {
    let mut refs = Vec::new();
    if let Some(path) = manifest_path(manifest_input) {
        refs.push(json!({
            "role": "wiki_manifest",
            "path": path,
        }));
    }
    if let Some(path) = manifest_input
        .get("staging")
        .and_then(|staging| staging.get("module_map_path"))
        .and_then(Value::as_str)
    {
        refs.push(json!({
            "role": "module_map",
            "path": path,
        }));
    }
    if let Some(scan_paths) = manifest_input
        .get("staging")
        .and_then(|staging| staging.get("scan_paths"))
        .and_then(Value::as_object)
    {
        for (node_id, path) in scan_paths {
            if let Some(path) = path.as_str() {
                refs.push(json!({
                    "role": "scan",
                    "node_id": node_id,
                    "path": path,
                }));
            }
        }
    }
    refs
}

fn mode_name(mode: KbPlanMode) -> &'static str {
    match mode {
        KbPlanMode::WikiPlan => "wiki_plan",
        KbPlanMode::WriterPlan => "writer_plan",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_plan_uses_wiki_pages() {
        let inputs = eval::NodeInputs::from_iter([(
            "wiki_plan".to_string(),
            json!({
                "pages": [
                    {
                        "id": "topology-mutation",
                        "path": "topology-mutation.md",
                        "title": "Topology Mutation Runtime"
                    }
                ]
            }),
        )]);

        let tmp = tempfile::TempDir::new().unwrap();
        let output = writer_plan(&inputs, tmp.path()).unwrap();

        assert_eq!(
            output["writer_plan"]["writers"][0]["id"],
            "writer-topology-mutation"
        );
        assert_eq!(
            output["writer_plan"]["writers"][0]["target_paths"][0],
            "topology-mutation.md"
        );
        assert!(tmp
            .path()
            .join(".bb_template/runtime/kb-wiki-output/.staging/writer-plan.json")
            .exists());
    }
}
