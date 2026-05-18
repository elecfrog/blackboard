use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::task_graph::definition::types::{
    EdgeKind, NodePin, NodeType, PinCategory, PinDirection, PinValueType, TaskGraphEdge,
    TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::runtime;
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
    #[serde(default = "default_mutation_mode")]
    mode: LlmMutationMode,
    #[serde(default)]
    inputs: Option<Value>,
    #[serde(default)]
    target_node_id: String,
    #[serde(default)]
    patch_target_source_ids: bool,
    #[serde(default)]
    generated: GeneratedNodeConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LlmMutationMode {
    Fanout,
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
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    resource_bundle: Option<Value>,
    #[serde(default)]
    output_contract: Option<Value>,
    #[serde(default)]
    lifecycle: Option<Value>,
    #[serde(default = "default_generated_artifact_type")]
    output_artifact_type: String,
    #[serde(default)]
    prompt_template: Option<String>,
    #[serde(default)]
    inputs: Option<Value>,
}

impl Default for GeneratedNodeConfig {
    fn default() -> Self {
        Self {
            runtime: default_generated_runtime(),
            agent: default_generated_agent(),
            model: None,
            variant: None,
            skills: Vec::new(),
            resource_bundle: None,
            output_contract: None,
            lifecycle: None,
            output_artifact_type: default_generated_artifact_type(),
            prompt_template: None,
            inputs: None,
        }
    }
}

#[derive(Debug, Clone)]
struct FanoutItem {
    id: String,
    title: Option<String>,
    scope: Option<String>,
    goal: Option<String>,
    scope_hints: Vec<String>,
    paths: Vec<String>,
    expected_output: Option<String>,
    page_ids: Vec<String>,
    target_paths: Vec<String>,
}

pub fn execute_llm_mutation_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
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
        &format!("[{start_time}] llm_mutation node starting"),
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

    let target_node_id = match mutation_target_node_id(node, &config, edge_map) {
        Ok(target) => target,
        Err(message) => {
            return failed_outcome(
                node,
                start_time,
                started.elapsed().as_millis() as u64,
                "invalid_mutation_target",
                message,
            );
        }
    };
    let direct_edge_id = direct_target_edge_id(edge_map, &node.id, &target_node_id);

    let plan = match plan_from_inputs_or_llm(opts, node, run, edge_map, &config, &inputs) {
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
    let requests = build_fanout_mutations(
        node,
        &config,
        &source_node_id,
        &target_node_id,
        direct_edge_id.as_deref(),
        &items,
        &plan,
    );
    let artifact_value = json!({
        "artifact_type": "topology_mutation",
        "mode": mode_name(config.mode),
        "source_node_id": source_node_id,
        "target_node_id": target_node_id,
        "merge_goal": string_field(&plan, "merge_goal"),
        "doc_target": string_field(&plan, "doc_target"),
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

fn mutation_target_node_id(
    node: &TaskGraphNode,
    config: &LlmMutationConfig,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<String, String> {
    if !config.target_node_id.trim().is_empty() {
        return Ok(config.target_node_id.trim().to_string());
    }
    let outgoing = edge_map
        .get(&node.id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|edge| edge.kind == EdgeKind::Exec)
        .collect::<Vec<_>>();
    match outgoing.as_slice() {
        [edge] => Ok(edge.to.clone()),
        [] => Err(
            "llm_mutation requires target_node_id or exactly one outgoing exec edge".to_string(),
        ),
        _ => Err(
            "llm_mutation has multiple outgoing exec edges; set target_node_id explicitly"
                .to_string(),
        ),
    }
}

fn direct_target_edge_id(
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    node_id: &str,
    target_node_id: &str,
) -> Option<String> {
    edge_map
        .get(node_id)?
        .iter()
        .find(|edge| edge.kind == EdgeKind::Exec && edge.to == target_node_id)
        .map(|edge| edge.id.clone())
}

fn plan_from_inputs_or_llm(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    config: &LlmMutationConfig,
    inputs: &eval::NodeInputs,
) -> Result<Value, String> {
    if has_mutation_plan_input(inputs) {
        return mutation_plan_input(inputs, &opts.workspace_root);
    }
    if matches!(config.mode, LlmMutationMode::Fanout) {
        return mutation_plan_from_llm(opts, node, run, edge_map);
    }
    mutation_plan_input(inputs, &opts.workspace_root)
}

fn has_mutation_plan_input(inputs: &eval::NodeInputs) -> bool {
    inputs.contains_key("plan_input")
        || inputs.contains_key("plan")
        || inputs.contains_key("scouts")
}

fn mutation_plan_from_llm(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<Value, String> {
    let mut llm_node = node.clone();
    llm_node.node_type = NodeType::Llm;
    let mut config = node.config.clone();
    if let Some(object) = config.as_object_mut() {
        object
            .entry("output".to_string())
            .or_insert_with(|| json!({ "artifact_type": "json", "required": true }));
    }
    llm_node.config = config;
    let outcome = runtime::execute_llm_node(opts, &llm_node, run, edge_map)
        .map_err(|source| format!("failed to run mutation LLM plan: {source}"))?;
    if outcome.status != NodeRunStatus::Succeeded {
        let message = outcome
            .node_state
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .or(outcome.node_state.log_tail)
            .unwrap_or_else(|| "mutation LLM plan failed".to_string());
        return Err(message);
    }
    outcome
        .output
        .map(|value| unwrap_plan_data(&value))
        .ok_or_else(|| "mutation LLM produced no plan output".to_string())
}

fn mutation_plan_input(
    inputs: &eval::NodeInputs,
    workspace_root: &std::path::Path,
) -> Result<Value, String> {
    if let Some(scouts) = inputs.get("scouts") {
        return Ok(json!({ "scouts": scouts.clone() }));
    }
    let plan = inputs
        .get("plan_input")
        .or_else(|| inputs.get("plan"))
        .ok_or_else(|| {
            "llm_mutation inputs.plan_input, inputs.plan, or inputs.scouts is required".to_string()
        })?;
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
            title: string_field(item, "title"),
            scope: string_field(item, "scope"),
            goal: string_field(item, "goal"),
            scope_hints: string_array(item, "scope_hints"),
            paths: string_array(item, "paths"),
            expected_output: string_field(item, "expected_output"),
            page_ids: string_array(item, "page_ids"),
            target_paths: string_array(item, "target_paths"),
        });
    }
    Ok(parsed)
}

fn fanout_values(mode: LlmMutationMode, plan: &Value) -> Result<Vec<Value>, String> {
    let array_key = match mode {
        LlmMutationMode::Fanout => "scouts",
        LlmMutationMode::ScoutFanout => "scouts",
        LlmMutationMode::WriterFanout => "writers",
    };
    if let Some(items) = plan.get(array_key).and_then(Value::as_array) {
        return Ok(items.clone());
    }

    let nested_key = match mode {
        LlmMutationMode::Fanout => "scout_plan",
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
    target_node_id: &str,
    direct_edge_id: Option<&str>,
    items: &[FanoutItem],
    plan: &Value,
) -> Vec<GraphMutationRequest> {
    let mut requests = Vec::with_capacity(items.len() * 3 + 2);
    if let Some(edge_id) = direct_edge_id {
        requests.push(GraphMutationRequest {
            id: format!("remove-direct-edge-{edge_id}"),
            source_task_id: String::new(),
            source_node_id: node.id.clone(),
            op: GraphMutationOp::RemoveEdge {
                edge_id: edge_id.to_string(),
            },
            reason: Some(format!(
                "Consume direct continuation edge `{edge_id}` before fanout"
            )),
        });
    }
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
            id: format!("edge-{}-to-{}", item.id, target_node_id),
            source_task_id: String::new(),
            source_node_id: node.id.clone(),
            op: GraphMutationOp::AddEdge {
                edge: exec_edge(
                    format!("edge-{}-to-{}", item.id, target_node_id),
                    item.id.clone(),
                    target_node_id.to_string(),
                ),
            },
            reason: Some(format!(
                "Route dynamic node `{}` into `{}`",
                item.id, target_node_id
            )),
        });
    }
    requests.push(GraphMutationRequest {
        id: format!("patch-{target_node_id}-scout-inputs"),
        source_task_id: String::new(),
        source_node_id: node.id.clone(),
        op: GraphMutationOp::PatchNodeConfig {
            node_id: target_node_id.to_string(),
            patch: summary_input_patch(node, config, items, plan),
        },
        reason: Some(format!(
            "Inject dynamic scout output bindings into `{target_node_id}`"
        )),
    });
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
    if !config.generated.skills.is_empty() {
        node_config.insert("skills".to_string(), json!(config.generated.skills));
    }
    if let Some(resource_bundle) = &config.generated.resource_bundle {
        node_config.insert("resource_bundle".to_string(), resource_bundle.clone());
    }
    if let Some(output_contract) = &config.generated.output_contract {
        node_config.insert("output_contract".to_string(), output_contract.clone());
    }
    if let Some(lifecycle) = &config.generated.lifecycle {
        node_config.insert("lifecycle".to_string(), lifecycle.clone());
    }
    node_config.insert("inputs".to_string(), generated_inputs(config, item));
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
            "template": generated_prompt(config, item),
        }),
    );

    let mut pins = vec![exec_pin("exec_in", PinDirection::In, true)];
    if let Some(resource_bundle_input) = generated_resource_bundle_input(config) {
        pins.push(NodePin {
            id: resource_bundle_input,
            label: "Resource Bundle".to_string(),
            direction: PinDirection::In,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Json),
            required: false,
        });
    }
    pins.push(exec_pin("exec_out", PinDirection::Out, false));
    pins.push(NodePin {
        id: "output".to_string(),
        label: "Output".to_string(),
        direction: PinDirection::Out,
        category: PinCategory::Data,
        value_type: Some(pin_value_type(&config.generated.output_artifact_type)),
        required: false,
    });

    TaskGraphNode {
        id: item.id.clone(),
        node_type: crate::task_graph::definition::types::NodeType::Llm,
        label: title_from_id(&item.id),
        description: item.scope.clone(),
        position: None,
        config: Value::Object(node_config),
        pins,
    }
}

fn generated_resource_bundle_input(config: &LlmMutationConfig) -> Option<String> {
    config
        .generated
        .resource_bundle
        .as_ref()
        .and_then(|resource_bundle| resource_bundle.get("input"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn summary_input_patch(
    node: &TaskGraphNode,
    config: &LlmMutationConfig,
    items: &[FanoutItem],
    plan: &Value,
) -> Value {
    let scout_outputs = items
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                json!(format!("{{{{nodes.{}.output}}}}", item.id)),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let mut patch = json!({
        "inputs": {
            "mutation": format!("{{{{nodes.{}.output}}}}", node.id),
            "scout_outputs": Value::Object(scout_outputs),
            "merge_goal": string_field(plan, "merge_goal").unwrap_or_default(),
            "doc_target": string_field(plan, "doc_target").unwrap_or_default(),
        }
    });

    if config.patch_target_source_ids {
        patch["source"] = json!({
            "kind": "node_outputs_by_ids",
            "ids": items.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        });
    }

    patch
}

fn generated_prompt(config: &LlmMutationConfig, item: &FanoutItem) -> String {
    if let Some(template) = config
        .generated
        .prompt_template
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return render_generated_prompt_template(template, item);
    }

    match config.mode {
        LlmMutationMode::Fanout => render_generated_prompt_template(
            "你是一个动态 scout LLM。\n\nScout id: {{item.id}}\nGoal: {{item.goal}}\nScope hints: {{item.scope_hints}}\nExpected output: {{item.expected_output}}\n\n输出 markdown findings，包含关键事实、证据位置、风险和未覆盖范围。",
            item,
        ),
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

fn render_generated_prompt_template(template: &str, item: &FanoutItem) -> String {
    template
        .replace("{{item.id}}", &item.id)
        .replace("{{item.title}}", item.title.as_deref().unwrap_or_default())
        .replace("{{item.scope}}", item.scope.as_deref().unwrap_or_default())
        .replace("{{item.goal}}", item.goal.as_deref().unwrap_or_default())
        .replace(
            "{{item.scope_hints}}",
            &serde_json::to_string(&item.scope_hints).unwrap_or_else(|_| "[]".to_string()),
        )
        .replace(
            "{{item.paths}}",
            &serde_json::to_string(&item.paths).unwrap_or_else(|_| "[]".to_string()),
        )
        .replace(
            "{{item.expected_output}}",
            item.expected_output.as_deref().unwrap_or_default(),
        )
}

fn generated_inputs(config: &LlmMutationConfig, item: &FanoutItem) -> Value {
    if let Some(inputs) = &config.generated.inputs {
        return inputs.clone();
    }
    match config.mode {
        LlmMutationMode::Fanout => json!({
            "item_id": item.id.clone(),
        }),
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

#[allow(clippy::too_many_arguments)]
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
            chars.next().map_or_else(String::new, |first| {
                let mut title = first.to_uppercase().to_string();
                title.push_str(chars.as_str());
                title
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const fn mode_name(mode: LlmMutationMode) -> &'static str {
    match mode {
        LlmMutationMode::Fanout => "fanout",
        LlmMutationMode::ScoutFanout => "scout_fanout",
        LlmMutationMode::WriterFanout => "writer_fanout",
    }
}

const fn default_mutation_mode() -> LlmMutationMode {
    LlmMutationMode::Fanout
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
            patch_target_source_ids: true,
            generated: GeneratedNodeConfig {
                runtime: "opencode".to_string(),
                agent: "native".to_string(),
                model: Some("minimax-cn-coding-plan/MiniMax-M2.7-highspeed".to_string()),
                variant: Some("high".to_string()),
                skills: Vec::new(),
                resource_bundle: Some(json!({
                    "input": "resource_bundle",
                    "required": true,
                })),
                output_contract: Some(json!({
                    "artifact_type": "json",
                    "required": true,
                })),
                lifecycle: None,
                output_artifact_type: "json".to_string(),
                prompt_template: None,
                inputs: None,
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
        let requests =
            build_fanout_mutations(&node, &config, &node.id, "merge-scans", None, &items, &plan);

        assert_eq!(requests.len(), 4);
        match &requests[0].op {
            GraphMutationOp::AddNode { node } => {
                assert_eq!(node.id, "scout-structure");
                assert_eq!(
                    node.config["model"],
                    "minimax-cn-coding-plan/MiniMax-M2.7-highspeed"
                );
                assert_eq!(node.config["variant"], "high");
                assert!(node.config.get("preset").is_none());
                assert!(node.config.get("harness").is_none());
                assert_eq!(node.config["resource_bundle"]["input"], "resource_bundle");
                assert_eq!(node.config["output_contract"]["artifact_type"], "json");
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
        match &requests[3].op {
            GraphMutationOp::PatchNodeConfig { node_id, patch } => {
                assert_eq!(node_id, "merge-scans");
                assert_eq!(
                    patch["inputs"]["scout_outputs"]["scout-structure"],
                    "{{nodes.scout-structure.output}}"
                );
                assert_eq!(patch["source"]["kind"], "node_outputs_by_ids");
                assert_eq!(patch["source"]["ids"], json!(["scout-structure"]));
            }
            other => panic!("expected patch_node_config, got {other:?}"),
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
            patch_target_source_ids: false,
            generated: GeneratedNodeConfig::default(),
        };
        let item = FanoutItem {
            id: "writer-overview".to_string(),
            title: None,
            scope: None,
            goal: Some("write overview".to_string()),
            scope_hints: vec![],
            paths: vec![],
            expected_output: None,
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

    #[test]
    fn generic_fanout_removes_direct_edge_and_patches_summary_inputs() {
        let config = LlmMutationConfig {
            mode: LlmMutationMode::Fanout,
            inputs: None,
            target_node_id: "summary".to_string(),
            patch_target_source_ids: false,
            generated: GeneratedNodeConfig {
                runtime: "opencode".to_string(),
                agent: "native".to_string(),
                model: None,
                variant: None,
                skills: Vec::new(),
                resource_bundle: None,
                output_contract: None,
                lifecycle: None,
                output_artifact_type: "markdown".to_string(),
                prompt_template: Some("Goal: {{item.goal}}".to_string()),
                inputs: None,
            },
        };
        let node = TaskGraphNode {
            id: "mutation".to_string(),
            node_type: NodeType::LlmMutation,
            label: "Mutation".to_string(),
            description: None,
            position: None,
            config: json!({}),
            pins: vec![],
        };
        let plan = json!({
            "scouts": [
                { "id": "editor", "goal": "scan editor", "scope_hints": ["editor"] }
            ],
            "merge_goal": "merge findings",
            "doc_target": "wiki/terrain.md"
        });
        let items = fanout_items(&config.mode, &plan).unwrap();
        let requests = build_fanout_mutations(
            &node,
            &config,
            &node.id,
            "summary",
            Some("mutation-summary"),
            &items,
            &plan,
        );

        assert_eq!(requests.len(), 5);
        assert!(matches!(
            requests[0].op,
            GraphMutationOp::RemoveEdge { ref edge_id } if edge_id == "mutation-summary"
        ));
        match &requests[1].op {
            GraphMutationOp::AddNode { node } => {
                assert_eq!(node.id, "editor");
                assert_eq!(node.config["output"]["artifact_type"], "markdown");
                assert_eq!(node.config["prompt"]["template"], "Goal: scan editor");
            }
            other => panic!("expected add_node, got {other:?}"),
        }
        match &requests[4].op {
            GraphMutationOp::PatchNodeConfig { node_id, patch } => {
                assert_eq!(node_id, "summary");
                assert_eq!(patch["inputs"]["merge_goal"], "merge findings");
                assert_eq!(patch["inputs"]["doc_target"], "wiki/terrain.md");
                assert_eq!(
                    patch["inputs"]["scout_outputs"]["editor"],
                    "{{nodes.editor.output}}"
                );
            }
            other => panic!("expected patch_node_config, got {other:?}"),
        }
    }
}
