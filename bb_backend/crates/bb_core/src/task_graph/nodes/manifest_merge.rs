use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::task_graph::definition::types::{TaskGraphError, TaskGraphNode};
use crate::task_graph::nodes::{eval, kb_staging};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode};

#[derive(Debug, Clone, Deserialize)]
struct ManifestMergeConfig {
    #[serde(default)]
    inputs: Option<Value>,
}

pub(crate) fn execute_manifest_merge_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: ManifestMergeConfig =
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
    let Some(kb_output_dir) = kb_staging::kb_output_dir_from_inputs(&inputs, &opts.workspace_root)
    else {
        return Ok(failed_outcome(
            node,
            "missing_kb_output_dir",
            "manifest_merge requires kb_output_dir before it can write .staging",
        ));
    };
    let staging_dir = kb_staging::staging_dir(&kb_output_dir);
    let scans_dir = staging_dir.join("scans");
    let scout_outputs = inputs
        .get("scout_outputs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| scout_outputs(run));
    let scan_paths = write_scan_outputs(&scans_dir, &scout_outputs)?;
    let source_anchors = capped(source_anchors(&scout_outputs), 240);
    let facts = capped_strings(facts(&scout_outputs), 500);
    let gaps = capped_strings(gaps(&inputs, &scout_outputs), 200);
    let module_sections = module_sections(inputs.get("truth"), &source_anchors);
    let module_map = module_map(&inputs, &scout_outputs, &scan_paths);

    let manifest = json!({
        "manifest_version": 1,
        "generated_at": Utc::now().to_rfc3339(),
        "kb_output_dir": kb_staging::path_string(&kb_output_dir),
        "staging_dir": kb_staging::path_string(&staging_dir),
        "source_anchors": source_anchors,
        "module_sections": module_sections,
        "facts": facts,
        "gaps": gaps,
        "scan_paths": scan_paths,
        "writer_inputs": {
            "intake": inputs.get("intake").cloned().unwrap_or(Value::Null),
            "truth": inputs.get("truth").cloned().unwrap_or(Value::Null),
            "scout_plan": inputs.get("scout_plan").cloned().unwrap_or(Value::Null),
            "scout_output_node_ids": scout_outputs.keys().cloned().collect::<Vec<_>>(),
        }
    });
    let manifest_path = staging_dir.join("wiki-manifest.json");
    let module_map_path = staging_dir.join("module-map.json");
    kb_staging::write_json_file(&manifest_path, &manifest)?;
    kb_staging::write_json_file(&module_map_path, &module_map)?;

    let output = manifest_ref(
        &kb_output_dir,
        &staging_dir,
        &scans_dir,
        &manifest_path,
        &module_map_path,
        &manifest,
    );

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
            log_tail: Some("Merged scout outputs into manifest".to_string()),
            child_run_id: None,
            runtime: Some("manifest_merge".to_string()),
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

fn failed_outcome(node: &TaskGraphNode, code: &str, message: &str) -> NodeOutcome {
    let now = Utc::now().to_rfc3339();
    NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        output: None,
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: Some(1),
            error: Some(NodeError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            output_artifact: None,
            log_tail: Some(message.to_string()),
            child_run_id: None,
            runtime: Some("manifest_merge".to_string()),
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
    }
}

fn scout_outputs(run: &TaskGraphRun) -> serde_json::Map<String, Value> {
    let mut outputs = serde_json::Map::new();
    let mut keys = run
        .context
        .node_outputs
        .keys()
        .filter(|key| key.starts_with("scout-"))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if key == "scout-planner" || key == "scout-mutation" {
            continue;
        }
        if let Some(value) = run.context.node_outputs.get(&key) {
            outputs.insert(key, value.clone());
        }
    }
    outputs
}

fn write_scan_outputs(
    scans_dir: &std::path::Path,
    scout_outputs: &serde_json::Map<String, Value>,
) -> Result<serde_json::Map<String, Value>, TaskGraphError> {
    let mut paths = serde_json::Map::new();
    for (node_id, output) in scout_outputs {
        let filename = format!("{}.json", kb_staging::sanitize_filename(node_id));
        let path = scans_dir.join(filename);
        kb_staging::write_json_file(&path, output)?;
        paths.insert(node_id.clone(), json!(kb_staging::path_string(&path)));
    }
    Ok(paths)
}

fn manifest_ref(
    kb_output_dir: &std::path::Path,
    staging_dir: &std::path::Path,
    scans_dir: &std::path::Path,
    manifest_path: &std::path::Path,
    module_map_path: &std::path::Path,
    manifest: &Value,
) -> Value {
    let anchors = manifest
        .get("source_anchors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let facts = manifest
        .get("facts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let gaps = manifest
        .get("gaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let module_sections = manifest
        .get("module_sections")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "manifest_version": manifest.get("manifest_version").cloned().unwrap_or(json!(1)),
        "staging": {
            "kb_output_dir": kb_staging::path_string(kb_output_dir),
            "dir": kb_staging::path_string(staging_dir),
            "scans_dir": kb_staging::path_string(scans_dir),
            "manifest_path": kb_staging::path_string(manifest_path),
            "module_map_path": kb_staging::path_string(module_map_path),
            "scan_paths": manifest.get("scan_paths").cloned().unwrap_or_else(|| json!({})),
        },
        "counts": {
            "source_anchors": anchors.len(),
            "facts": facts.len(),
            "gaps": gaps.len(),
            "module_sections": module_sections.len(),
        },
        "module_sections": module_sections.into_iter().take(20).collect::<Vec<_>>(),
        "preview": {
            "source_anchors": anchors.into_iter().take(20).collect::<Vec<_>>(),
            "facts": facts.into_iter().take(20).collect::<Vec<_>>(),
            "gaps": gaps.into_iter().take(20).collect::<Vec<_>>(),
        }
    })
}

fn module_map(
    inputs: &eval::NodeInputs,
    scout_outputs: &serde_json::Map<String, Value>,
    scan_paths: &serde_json::Map<String, Value>,
) -> Value {
    let intake = inputs.get("intake").cloned().unwrap_or(Value::Null);
    let truth = inputs.get("truth").cloned().unwrap_or(Value::Null);
    let module_name = truth
        .get("module")
        .and_then(|module| module.get("name"))
        .cloned()
        .or_else(|| intake.get("module_name").cloned())
        .unwrap_or(Value::Null);
    let module_root = truth
        .get("module")
        .and_then(|module| module.get("root"))
        .cloned()
        .or_else(|| intake.get("module_root").cloned())
        .unwrap_or(Value::Null);
    json!({
        "module_name": module_name,
        "module_root": module_root,
        "generated_at": Utc::now().to_rfc3339(),
        "scanner_count": scout_outputs.len(),
        "scan_paths": scan_paths,
        "scan_coverage": scout_outputs
            .keys()
            .map(|node_id| (node_id.clone(), Value::String("COMPLETE".to_string())))
            .collect::<serde_json::Map<String, Value>>(),
    })
}

fn source_anchors(scout_outputs: &serde_json::Map<String, Value>) -> Vec<Value> {
    let mut anchors = Vec::new();
    for (node_id, output) in scout_outputs {
        for finding in output
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let title = string_field(finding, "title").unwrap_or_else(|| node_id.clone());
            let reason = string_field(finding, "summary").unwrap_or_else(|| title.clone());
            for anchor in finding
                .get("source_anchors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                anchors.push(json!({
                    "id": format!("{}:{}", node_id, anchors.len() + 1),
                    "path": anchor,
                    "symbol": title,
                    "reason": reason,
                }));
            }
        }
    }
    anchors
}

fn facts(scout_outputs: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut facts = Vec::new();
    for (node_id, output) in scout_outputs {
        for finding in output
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(title) = string_field(finding, "title") {
                let summary = string_field(finding, "summary").unwrap_or_default();
                facts.push(format!("{node_id}: {title} - {summary}"));
            }
        }
    }
    facts
}

fn gaps(inputs: &eval::NodeInputs, scout_outputs: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut gaps = Vec::new();
    for key in [
        "scout_structure",
        "scout_interfaces",
        "scout_internals",
        "scout_infra",
        "scout_tests",
        "scout_existing_wiki",
    ] {
        if inputs.get(key).map(Value::is_null).unwrap_or(true) {
            gaps.push(format!("missing {key} output"));
        }
    }
    for (node_id, output) in scout_outputs {
        collect_string_array(output, "risks", &mut gaps, node_id);
        collect_string_array(output, "coverage_notes", &mut gaps, node_id);
    }
    gaps
}

fn module_sections(truth: Option<&Value>, anchors: &[Value]) -> Vec<Value> {
    let required_h2 = truth
        .and_then(|value| value.get("contracts"))
        .and_then(|contracts| contracts.get("required_h2"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let anchor_ids = anchors
        .iter()
        .take(20)
        .filter_map(|anchor| anchor.get("id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    required_h2
        .iter()
        .enumerate()
        .filter_map(|(index, title)| {
            let title = title.as_str()?;
            Some(json!({
                "id": section_id(title, index),
                "title": title,
                "required": true,
                "source_anchor_ids": anchor_ids,
            }))
        })
        .collect()
}

fn collect_string_array(output: &Value, field: &str, target: &mut Vec<String>, node_id: &str) {
    for value in output
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        target.push(format!("{node_id}: {value}"));
    }
}

fn capped<T>(values: Vec<T>, max: usize) -> Vec<T> {
    values.into_iter().take(max).collect()
}

fn capped_strings(mut values: Vec<String>, max: usize) -> Vec<String> {
    if values.len() > max {
        values.truncate(max);
        values.push(format!("truncated: kept first {max} items"));
    }
    values
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn section_id(title: &str, index: usize) -> String {
    let mut id = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    if id.is_empty() {
        format!("section-{}", index + 1)
    } else {
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_merge_collects_scout_findings() {
        let outputs = [(
            "scout-structure".to_string(),
            json!({
                "findings": [{
                    "title": "GraphRevision",
                    "summary": "revision model",
                    "source_anchors": ["topology/mod.rs:22"]
                }],
                "risks": ["risk one"]
            }),
        )]
        .into_iter()
        .collect();
        let anchors = source_anchors(&outputs);
        let facts = facts(&outputs);

        assert_eq!(outputs.len(), 1);
        assert_eq!(anchors[0]["path"], "topology/mod.rs:22");
        assert!(facts[0].contains("GraphRevision"));
    }
}
