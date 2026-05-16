use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::task_graph::definition::types::{
    EdgeKind, NodePin, PinCategory, PinDirection, PinValueType, TaskGraphEdge, TaskGraphError,
    TaskGraphNode,
};
use crate::task_graph::nodes::{eval, kb_staging};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, ArtifactContentType, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::topology::{GraphMutationOp, GraphMutationRequest};

const RUNTIME_NAME: &str = "llm_mutation";
const DEFAULT_RUNTIME: &str = "opencode";
const DEFAULT_AGENT: &str = "native";

#[derive(Debug, Clone, Deserialize)]
struct LlmMutationConfig {
    mode: LlmMutationMode,
    #[serde(default)]
    inputs: Option<Value>,
    target_node_id: String,
    #[serde(default)]
    generated: GeneratedNodeConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LlmMutationMode {
    ScoutFanout,
    WriterFanout,
}

#[derive(Debug, Clone, Deserialize)]
struct GeneratedNodeConfig {
    #[serde(default = "default_generated_runtime")]
    runtime: String,
    #[serde(default = "default_generated_agent")]
    agent: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default = "default_generated_artifact_type")]
    output_artifact_type: String,
}

impl Default for GeneratedNodeConfig {
    fn default() -> Self {
        Self {
            runtime: default_generated_runtime(),
            agent: default_generated_agent(),
            model: None,
            variant: None,
            output_artifact_type: default_generated_artifact_type(),
        }
    }
}

#[derive(Debug, Clone)]
struct FanoutItem {
    id: String,
    scope: Option<String>,
    goal: Option<String>,
    page_ids: Vec<String>,
    target_paths: Vec<String>,
}

pub(crate) fn execute_llm_mutation_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;
    let start_time = Utc::now().to_rfc3339();
    let started = Instant::now();

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!("[{}] llm_mutation node starting", start_time),
    )?;
    run_state::update_node_state(
        ws,
        project,
        run_id,
        &node_state(
            node,
            NodeRunStatus::Running,
            start_time.clone(),
            None,
            None,
            None,
            None,
            Some("llm_mutation node starting".to_string()),
        ),
    )?;

    let config: LlmMutationConfig =
        match serde_json::from_value::<LlmMutationConfig>(node.config.clone()) {
            Ok(config) => config,
            Err(source) => {
                return failed_outcome(
                    node,
                    start_time,
                    started.elapsed().as_millis() as u64,
                    "invalid_config",
                    format!("LLM mutation node config parse error: {source}"),
                );
            }
        };

    let inputs = eval::resolve_node_inputs(
        config.inputs.as_ref(),
        project,
        ws,
        &opts.scripts_dir,
        &run.context,
    );
    let source_node_id = node.id.clone();

    let plan = match mutation_plan_input(&inputs, &opts.workspace_root) {
        Ok(plan) => plan,
        Err(message) => {
            return failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_mutation_plan",
                message,
            );
        }
    };

    let items = match fanout_items(&config.mode, &plan) {
        Ok(items) => items,
        Err(message) => {
            return failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_mutation_plan",
                message,
            );
        }
    };
    let requests = build_fanout_mutations(node, &config, &source_node_id, &items);
    let artifact_value = json!({
        "artifact_type": "topology_mutation",
        "mode": mode_name(config.mode),
        "source_node_id": source_node_id,
        "target_node_id": config.target_node_id,
        "generated_node_count": items.len(),
        "request_count": requests.len(),
        "requests": requests,
    });
    let artifact_content = serde_json::to_string_pretty(&artifact_value)
        .unwrap_or_else(|_| artifact_value.to_string());
    let artifact = run_state::write_artifact(
        ws,
        project,
        run_id,
        &format!("{}-mutation-artifact", node.id),
        &artifact_content,
        ArtifactContentType::Json,
    )?;

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] llm_mutation node generated {} topology mutation request(s)",
            Utc::now().to_rfc3339(),
            requests.len()
        ),
    )?;

    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Succeeded,
        output: Some(artifact_value),
        node_state: node_state(
            node,
            NodeRunStatus::Succeeded,
            start_time,
            Some(Utc::now().to_rfc3339()),
            Some(started.elapsed().as_millis() as u64),
            Some(0),
            None,
            Some(format!(
                "generated {} topology mutation request(s)",
                requests.len()
            )),
        )
        .with_artifact(artifact),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: requests,
    })
}

fn mutation_plan_input(
    inputs: &eval::NodeInputs,
    workspace_root: &std::path::Path,
) -> Result<Value, String> {
    let plan = inputs
        .get("plan_input")
        .or_else(|| inputs.get("plan"))
        .ok_or_else(|| "llm_mutation inputs.plan_input or inputs.plan is required".to_string())?;
    normalize_plan_input(plan, workspace_root)
}

fn normalize_plan_input(plan: &Value, workspace_root: &std::path::Path) -> Result<Value, String> {
    if let Some(path) = plan
        .get("artifact_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = kb_staging::resolve_path(path, workspace_root);
        let from_file = kb_staging::read_json_file(&path).map_err(|source| {
            format!(
                "failed to read mutation plan artifact `{}`: {source}",
                path.display()
            )
        })?;
        return Ok(unwrap_plan_data(&from_file));
    }

    Ok(unwrap_plan_data(plan))
}

fn unwrap_plan_data(plan: &Value) -> Value {
    plan.get("data").cloned().unwrap_or_else(|| plan.clone())
}

fn fanout_items(mode: &LlmMutationMode, plan: &Value) -> Result<Vec<FanoutItem>, String> {
    let items = fanout_values(*mode, plan)?;
    if items.is_empty() {
        return Err(format!(
            "llm_mutation {} plan produced zero fanout items",
            mode_name(*mode)
        ));
    }

    let mut seen = BTreeSet::new();
    let mut parsed = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| format!("fanout item at index {index} is missing id"))?
            .to_string();
        if !seen.insert(id.clone()) {
            return Err(format!("duplicate fanout item id `{id}`"));
        }
        parsed.push(FanoutItem {
            id,
            scope: string_field(item, "scope"),
            goal: string_field(item, "goal"),
            page_ids: string_array(item, "page_ids"),
            target_paths: string_array(item, "target_paths"),
        });
    }
    Ok(parsed)
}

fn fanout_values(mode: LlmMutationMode, plan: &Value) -> Result<Vec<Value>, String> {
    let array_key = match mode {
        LlmMutationMode::ScoutFanout => "scouts",
        LlmMutationMode::WriterFanout => "writers",
    };
    if let Some(items) = plan.get(array_key).and_then(Value::as_array) {
        return Ok(items.clone());
    }

    let nested_key = match mode {
        LlmMutationMode::ScoutFanout => "scout_plan",
        LlmMutationMode::WriterFanout => "writer_plan",
    };
    if let Some(items) = plan
        .get(nested_key)
        .and_then(|plan| plan.get(array_key))
        .and_then(Value::as_array)
    {
        return Ok(items.clone());
    }

    Err(format!(
        "llm_mutation {} plan must contain a non-empty `{array_key}` array",
        mode_name(mode)
    ))
}

fn build_fanout_mutations(
    node: &TaskGraphNode,
    config: &LlmMutationConfig,
    source_node_id: &str,
    items: &[FanoutItem],
) -> Vec<GraphMutationRequest> {
    let mut requests = Vec::with_capacity(items.len() * 3);
    for item in items {
        requests.push(GraphMutationRequest {
            id: format!("node-{}", item.id),
            source_task_id: String::new(),
            source_node_id: node.id.clone(),
            op: GraphMutationOp::AddNode {
                node: generated_node(config, item),
            },
            reason: Some(format!(
                "Generate {} node `{}`",
                mode_name(config.mode),
                item.id
            )),
        });
        requests.push(GraphMutationRequest {
            id: format!("edge-{source_node_id}-to-{}", item.id),
            source_task_id: String::new(),
            source_node_id: node.id.clone(),
            op: GraphMutationOp::AddEdge {
                edge: exec_edge(
                    format!("edge-{source_node_id}-to-{}", item.id),
                    source_node_id.to_string(),
                    item.id.clone(),
                ),
            },
            reason: Some(format!(
                "Schedule dynamic node `{}` after mutation build",
                item.id
            )),
        });
        requests.push(GraphMutationRequest {
            id: format!("edge-{}-to-{}", item.id, config.target_node_id),
            source_task_id: String::new(),
            source_node_id: node.id.clone(),
            op: GraphMutationOp::AddEdge {
                edge: exec_edge(
                    format!("edge-{}-to-{}", item.id, config.target_node_id),
                    item.id.clone(),
                    config.target_node_id.clone(),
                ),
            },
            reason: Some(format!(
                "Route dynamic node `{}` into `{}`",
                item.id, config.target_node_id
            )),
        });
    }
    requests
}

fn generated_node(config: &LlmMutationConfig, item: &FanoutItem) -> TaskGraphNode {
    let mut node_config = serde_json::Map::new();
    node_config.insert("runtime".to_string(), json!(config.generated.runtime));
    node_config.insert("agent".to_string(), json!(config.generated.agent));
    if let Some(model) = &config.generated.model {
        node_config.insert("model".to_string(), json!(model));
    }
    if let Some(variant) = &config.generated.variant {
        node_config.insert("variant".to_string(), json!(variant));
    }
    node_config.insert("inputs".to_string(), generated_inputs(config.mode));
    node_config.insert(
        "output".to_string(),
        json!({
            "artifact_type": config.generated.output_artifact_type,
            "required": true,
        }),
    );
    node_config.insert(
        "prompt".to_string(),
        json!({
            "mode": "inline",
            "template": generated_prompt(config.mode, item),
        }),
    );

    TaskGraphNode {
        id: item.id.clone(),
        node_type: crate::task_graph::definition::types::NodeType::Llm,
        label: title_from_id(&item.id),
        description: item.scope.clone(),
        position: None,
        config: Value::Object(node_config),
        pins: vec![
            exec_pin("exec_in", PinDirection::In, true),
            exec_pin("exec_out", PinDirection::Out, false),
            NodePin {
                id: "output".to_string(),
                label: "Output".to_string(),
                direction: PinDirection::Out,
                category: PinCategory::Data,
                value_type: Some(pin_value_type(&config.generated.output_artifact_type)),
                required: false,
            },
        ],
    }
}

fn generated_prompt(mode: LlmMutationMode, item: &FanoutItem) -> String {
    match mode {
        LlmMutationMode::ScoutFanout => format!(
            "你是 KB Wiki Workflow 的动态 scout 节点。只输出 JSON，不要 Markdown。\n\nIntake: {{{{inputs.intake}}}}\nExisting Truth: {{{{inputs.truth}}}}\nmodule_root={{{{inputs.module_root}}}}\nkb_output_dir={{{{inputs.kb_output_dir}}}}\nstaging_dir={{{{inputs.staging_dir}}}}\n\nscope: {}\ngoal: {}\n\n任务：按 goal 扫描源码、已有文档和相关约束，返回结构化 findings。每条发现必须尽量包含 source anchors。你不需要写 .staging 文件，merge-scans 会把你的 output 落盘到 staging_dir/scans。输出 JSON schema: {{\"findings\":[{{\"title\":string,\"summary\":string,\"source_anchors\":string[]}}],\"risks\":string[],\"coverage_notes\":string[]}}。",
            item.scope.as_deref().unwrap_or("unspecified"),
            item.goal.as_deref().unwrap_or("scan assigned scope")
        ),
        LlmMutationMode::WriterFanout => format!(
            "你是 KB Wiki Workflow 的动态 writer 节点。输出目标 Markdown 内容或写作摘要，不要输出无关解释。\n\nManifest Ref: {{{{inputs.manifest}}}}\nWiki Plan: {{{{inputs.wiki_plan}}}}\nWriter Plan: {{{{inputs.writer_plan}}}}\nkb_output_dir={{{{inputs.kb_output_dir}}}}\nstaging_dir={{{{inputs.staging_dir}}}}\nmanifest_path={{{{inputs.manifest_path}}}}\nwiki_plan_path={{{{inputs.wiki_plan_path}}}}\nwriter_plan_path={{{{inputs.writer_plan_path}}}}\nlanguage={{{{inputs.language}}}}\n\npage_ids: {}\ntarget_paths: {}\ngoal: {}\n\n任务：必须先读取 manifest_path、wiki_plan_path、writer_plan_path，以及 staging_dir/scans 下的相关 scout 文件，把这些 .staging 文件作为事实源。基于 wiki plan 的分工写入对应 KB 页面，遵守 H2 契约和 source anchors 规则。target_paths 若为相对路径，必须解析到 kb_output_dir 下。完成后返回 JSON schema: {{\"written\":string[],\"summary\":string,\"risks\":string[]}}。",
            serde_json::to_string(&item.page_ids).unwrap_or_else(|_| "[]".to_string()),
            serde_json::to_string(&item.target_paths).unwrap_or_else(|_| "[]".to_string()),
            item.goal.as_deref().unwrap_or("write assigned wiki pages")
        ),
    }
}

fn generated_inputs(mode: LlmMutationMode) -> Value {
    match mode {
        LlmMutationMode::ScoutFanout => json!({
            "intake": "{{nodes.intake-validate.output}}",
            "truth": "{{nodes.scan-existing-truth.output}}",
            "module_root": "{{nodes.intake-validate.output.module_root}}",
            "kb_output_dir": "{{nodes.intake-validate.output.kb_output_dir}}",
            "staging_dir": "{{nodes.intake-validate.output.kb_output_dir}}/.staging",
        }),
        LlmMutationMode::WriterFanout => json!({
            "manifest": "{{nodes.merge-scans.output}}",
            "wiki_plan": "{{nodes.wiki-plan-final.output.data}}",
            "writer_plan": "{{nodes.writer-plan-final.output.data}}",
            "kb_output_dir": "{{nodes.intake-validate.output.kb_output_dir}}",
            "staging_dir": "{{nodes.merge-scans.output.staging.dir}}",
            "manifest_path": "{{nodes.merge-scans.output.staging.manifest_path}}",
            "wiki_plan_path": "{{nodes.wiki-plan-final.output.artifact_path}}",
            "writer_plan_path": "{{nodes.writer-plan-final.output.artifact_path}}",
            "language": "{{nodes.intake-validate.output.language}}",
        }),
    }
}

fn exec_edge(id: String, from: String, to: String) -> TaskGraphEdge {
    TaskGraphEdge {
        id,
        from,
        to,
        kind: EdgeKind::Exec,
        label: None,
        from_pin: Some("exec_out".to_string()),
        to_pin: Some("exec_in".to_string()),
        source_handle: None,
        target_handle: None,
    }
}

fn exec_pin(id: &str, direction: PinDirection, required: bool) -> NodePin {
    NodePin {
        id: id.to_string(),
        label: match direction {
            PinDirection::In => "In".to_string(),
            PinDirection::Out => "Out".to_string(),
        },
        direction,
        category: PinCategory::Exec,
        value_type: None,
        required,
    }
}

fn pin_value_type(artifact_type: &str) -> PinValueType {
    match artifact_type {
        "markdown" => PinValueType::Markdown,
        "json" => PinValueType::Json,
        _ => PinValueType::Any,
    }
}

fn node_state(
    node: &TaskGraphNode,
    status: NodeRunStatus,
    started_at: String,
    completed_at: Option<String>,
    duration_ms: Option<u64>,
    exit_code: Option<i32>,
    error: Option<NodeError>,
    log_tail: Option<String>,
) -> TaskGraphRunNode {
    TaskGraphRunNode {
        node_id: node.id.clone(),
        status,
        started_at: Some(started_at),
        completed_at,
        duration_ms,
        iteration: None,
        exit_code,
        error,
        output_artifact: None,
        log_tail,
        child_run_id: None,
        runtime: Some(RUNTIME_NAME.to_string()),
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    }
}

trait WithArtifact {
    fn with_artifact(self, artifact: run_state::OutputArtifact) -> Self;
}

impl WithArtifact for TaskGraphRunNode {
    fn with_artifact(mut self, artifact: run_state::OutputArtifact) -> Self {
        self.output_artifact = Some(artifact);
        self
    }
}

fn failed_outcome(
    node: &TaskGraphNode,
    start_time: String,
    duration_ms: u64,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Result<NodeOutcome, TaskGraphError> {
    let message = message.into();
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        output: None,
        node_state: node_state(
            node,
            NodeRunStatus::Failed,
            start_time,
            Some(Utc::now().to_rfc3339()),
            Some(duration_ms),
            Some(1),
            Some(NodeError {
                code: code.into(),
                message: message.clone(),
            }),
            Some(message),
        ),
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
        control: vec![],
        graph_mutations: vec![],
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn title_from_id(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut title = first.to_uppercase().to_string();
                    title.push_str(chars.as_str());
                    title
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn mode_name(mode: LlmMutationMode) -> &'static str {
    match mode {
        LlmMutationMode::ScoutFanout => "scout_fanout",
        LlmMutationMode::WriterFanout => "writer_fanout",
    }
}

fn default_generated_runtime() -> String {
    DEFAULT_RUNTIME.to_string()
}

fn default_generated_agent() -> String {
    DEFAULT_AGENT.to_string()
}

fn default_generated_artifact_type() -> String {
    "json".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scout_fanout_plan_builds_deterministic_mutation_requests() {
        let inputs = eval::NodeInputs::from_iter([(
            "plan".to_string(),
            json!({
                "scouts": [
                    {
                        "id": "scout-structure",
                        "scope": "structure",
                        "goal": "scan structure"
                    }
                ]
            }),
        )]);
        let config = LlmMutationConfig {
            mode: LlmMutationMode::ScoutFanout,
            inputs: None,
            target_node_id: "merge-scans".to_string(),
            generated: GeneratedNodeConfig {
                runtime: "opencode".to_string(),
                agent: "native".to_string(),
                model: Some("minimax-cn-coding-plan/MiniMax-M2.7-highspeed".to_string()),
                variant: Some("high".to_string()),
                output_artifact_type: "json".to_string(),
            },
        };
        let node = TaskGraphNode {
            id: "scout-mutation".to_string(),
            node_type: crate::task_graph::definition::types::NodeType::LlmMutation,
            label: "Scout Mutation".to_string(),
            description: None,
            position: None,
            config: json!({}),
            pins: vec![],
        };

        let plan = mutation_plan_input(&inputs, std::path::Path::new(".")).unwrap();
        let items = fanout_items(&config.mode, &plan).unwrap();
        let requests = build_fanout_mutations(&node, &config, &node.id, &items);

        assert_eq!(requests.len(), 3);
        match &requests[0].op {
            GraphMutationOp::AddNode { node } => {
                assert_eq!(node.id, "scout-structure");
                assert_eq!(
                    node.config["model"],
                    "minimax-cn-coding-plan/MiniMax-M2.7-highspeed"
                );
                assert_eq!(node.config["variant"], "high");
            }
            other => panic!("expected add_node, got {other:?}"),
        }
        match &requests[1].op {
            GraphMutationOp::AddEdge { edge } => {
                assert_eq!(edge.from, "scout-mutation");
                assert_eq!(edge.to, "scout-structure");
            }
            other => panic!("expected add_edge, got {other:?}"),
        }
        match &requests[2].op {
            GraphMutationOp::AddEdge { edge } => {
                assert_eq!(edge.from, "scout-structure");
                assert_eq!(edge.to, "merge-scans");
            }
            other => panic!("expected add_edge, got {other:?}"),
        }
    }

    #[test]
    fn scout_fanout_rejects_single_object_plan() {
        let inputs = eval::NodeInputs::from_iter([(
            "plan".to_string(),
            json!({
                "id": "scout-config",
                "scope": "topology_mutation.rs",
                "goal": "scan config contracts"
            }),
        )]);

        let plan = mutation_plan_input(&inputs, std::path::Path::new(".")).unwrap();
        let error = fanout_items(&LlmMutationMode::ScoutFanout, &plan).unwrap_err();
        assert!(error.contains("non-empty `scouts` array"));
    }

    #[test]
    fn writer_generated_node_receives_manifest_context() {
        let config = LlmMutationConfig {
            mode: LlmMutationMode::WriterFanout,
            inputs: None,
            target_node_id: "repair-loop".to_string(),
            generated: GeneratedNodeConfig::default(),
        };
        let item = FanoutItem {
            id: "writer-overview".to_string(),
            scope: None,
            goal: Some("write overview".to_string()),
            page_ids: vec!["overview".to_string()],
            target_paths: vec!["overview.md".to_string()],
        };

        let node = generated_node(&config, &item);

        assert_eq!(
            node.config["inputs"]["manifest"],
            "{{nodes.merge-scans.output}}"
        );
        assert_eq!(
            node.config["inputs"]["writer_plan_path"],
            "{{nodes.writer-plan-final.output.artifact_path}}"
        );
        assert_eq!(
            node.config["inputs"]["kb_output_dir"],
            "{{nodes.intake-validate.output.kb_output_dir}}"
        );
    }

    #[test]
    fn mutation_plan_input_reads_artifact_path_wrapper() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("writer-plan.json");
        kb_staging::write_json_file(
            &path,
            &json!({
                "writer_plan": {
                    "writers": [
                        {
                            "id": "writer-overview",
                            "goal": "write overview"
                        }
                    ]
                }
            }),
        )
        .unwrap();
        let inputs = eval::NodeInputs::from_iter([(
            "plan_input".to_string(),
            json!({
                "artifact_path": path.display().to_string(),
                "artifact_type": "json"
            }),
        )]);

        let plan = mutation_plan_input(&inputs, tmp.path()).unwrap();
        let items = fanout_items(&LlmMutationMode::WriterFanout, &plan).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "writer-overview");
    }
}
