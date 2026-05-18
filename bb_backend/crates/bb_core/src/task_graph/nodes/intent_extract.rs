use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::task_graph::definition::types::{TaskGraphError, TaskGraphNode};
use crate::task_graph::nodes::eval;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{NodeRunStatus, TaskGraphRun, TaskGraphRunNode};

#[derive(Debug, Clone, Deserialize)]
struct IntentExtractConfig {
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    inputs: Option<Value>,
    #[serde(default = "default_language")]
    language: String,
}

pub fn execute_intent_extract_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
) -> Result<NodeOutcome, TaskGraphError> {
    let config: IntentExtractConfig =
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
    let intent = string_value(inputs.get("request"))
        .or_else(|| string_value(inputs.get("intent")))
        .or_else(|| string_value(run.context.input.get("request")))
        .or_else(|| string_value(run.context.input.get("intent")))
        .unwrap_or_default();
    let draft = inputs.get("draft").cloned().unwrap_or(Value::Null);

    let paths = extract_windows_paths(&intent);
    if config.mode == "intent_gate" || config.mode == "code_search" {
        let output = intent_gate_output(&config, &intent, &draft, &paths);
        return Ok(successful_outcome(
            &node.id,
            output,
            "Intent gate normalized route from original input",
        ));
    }

    let output_path = path_after_marker(&intent, "输出到")
        .or_else(|| path_after_marker(&intent, "输出至"))
        .or_else(|| path_after_marker(&intent, "Output to"))
        .or_else(|| path_after_marker(&intent, "output to"))
        .or_else(|| {
            if paths.len() >= 2 {
                paths.last().cloned()
            } else {
                None
            }
        });
    let module_root = paths
        .iter()
        .find(|path| Some((*path).as_str()) != output_path.as_deref())
        .cloned()
        .or_else(|| string_field(&draft, "module_root"));
    let kb_output_dir = output_path.or_else(|| string_field(&draft, "kb_output_dir"));
    let extra_source_dirs = merged_extra_source_dirs(&draft, &paths, &module_root, &kb_output_dir);
    let source_dirs = source_dirs(&module_root, &extra_source_dirs);

    let mut missing = Vec::new();
    if module_root.as_deref().unwrap_or_default().is_empty() {
        missing.push("module_root".to_string());
    }
    if kb_output_dir.as_deref().unwrap_or_default().is_empty() {
        missing.push("kb_output_dir".to_string());
    }

    let output = json!({
        "request": string_field(&draft, "request").unwrap_or_else(|| intent.clone()),
        "module_name": string_field(&draft, "module_name")
            .or_else(|| infer_module_name(&intent)),
        "module_root": module_root,
        "kb_output_dir": kb_output_dir,
        "extra_source_dirs": extra_source_dirs,
        "source_dirs": source_dirs,
        "language": string_field(&draft, "language").unwrap_or(config.language),
        "ok": missing.is_empty(),
        "missing": missing,
        "contracts": {
            "source_anchor_required": true,
            "module_h2_contract": true
        },
        "setup_actions": [],
        "assumptions": array_field(&draft, "assumptions"),
        "confidence": number_field(&draft, "confidence").unwrap_or(1.0),
        "path_lock": {
            "source": "intent_extract",
            "candidates": paths
        }
    });

    Ok(successful_outcome(
        &node.id,
        output,
        "Intent paths locked from original input",
    ))
}

fn default_mode() -> String {
    "kb_wiki".to_string()
}

fn default_language() -> String {
    "zh-CN".to_string()
}

fn successful_outcome(node_id: &str, output: Value, log_tail: &str) -> NodeOutcome {
    let now = Utc::now().to_rfc3339();
    NodeOutcome {
        node_id: node_id.to_string(),
        status: NodeRunStatus::Succeeded,
        output: Some(output),
        node_state: TaskGraphRunNode {
            node_id: node_id.to_string(),
            status: NodeRunStatus::Succeeded,
            started_at: Some(now.clone()),
            completed_at: Some(now),
            duration_ms: Some(0),
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(log_tail.to_string()),
            child_run_id: None,
            runtime: None,
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

fn intent_gate_output(
    config: &IntentExtractConfig,
    intent: &str,
    draft: &Value,
    paths: &[String],
) -> Value {
    let request = string_field(draft, "request").unwrap_or_else(|| intent.to_string());
    let intent_type = string_field(draft, "intent_type")
        .or_else(|| string_field(draft, "task_type"))
        .or_else(|| infer_intent_type(intent))
        .unwrap_or_else(|| "task".to_string());
    let route = string_field(draft, "route").unwrap_or_else(|| infer_intent_route(&request));
    let target = string_field(draft, "target")
        .or_else(|| string_field(draft, "module_name"))
        .or_else(|| infer_target(intent))
        .or_else(|| paths.first().cloned());
    let mut scope_hints = string_array_field(draft, "scope_hints");
    scope_hints.extend(string_array_field(draft, "source_dirs"));
    scope_hints.extend(paths.iter().cloned());
    let scope_hints = dedupe(scope_hints);

    let mut missing = Vec::new();
    if request.trim().is_empty() {
        missing.push("request".to_string());
    }

    json!({
        "intent_type": intent_type,
        "route": route,
        "request": request,
        "normalized_request": string_field(draft, "normalized_request").unwrap_or_else(|| intent.to_string()),
        "target": target,
        "scope_hints": scope_hints,
        "constraints": array_field(draft, "constraints"),
        "language": string_field(draft, "language").unwrap_or_else(|| config.language.clone()),
        "ok": missing.is_empty(),
        "missing": missing,
        "intent_gate_contract": {
            "routes": ["simple", "complex", "needs_clarification", "unsupported"],
            "branch_key": "$.route",
            "fallback_is_failure": true
        },
        "assumptions": array_field(draft, "assumptions"),
        "confidence": number_field(draft, "confidence").unwrap_or(1.0),
        "path_lock": {
            "source": "intent_extract",
            "candidates": paths
        }
    })
}

fn string_value(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn number_field(value: &Value, field: &str) -> Option<f64> {
    value.get(field).and_then(Value::as_f64)
}

fn array_field(value: &Value, field: &str) -> Vec<Value> {
    value
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn string_array_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn merged_extra_source_dirs(
    draft: &Value,
    paths: &[String],
    module_root: &Option<String>,
    kb_output_dir: &Option<String>,
) -> Vec<String> {
    let mut values = string_array_field(draft, "extra_source_dirs");
    for path in paths {
        if Some(path.as_str()) == module_root.as_deref()
            || Some(path.as_str()) == kb_output_dir.as_deref()
        {
            continue;
        }
        values.push(path.clone());
    }
    dedupe(values)
}

fn source_dirs(module_root: &Option<String>, extra_source_dirs: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(module_root) = module_root {
        values.push(module_root.clone());
    }
    values.extend(extra_source_dirs.iter().cloned());
    dedupe(values)
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.iter().any(|existing| existing == &value) {
            deduped.push(value);
        }
    }
    deduped
}

fn path_after_marker(text: &str, marker: &str) -> Option<String> {
    let (_, tail) = text.split_once(marker)?;
    extract_windows_paths(tail).into_iter().next()
}

fn extract_windows_paths(text: &str) -> Vec<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut paths = Vec::new();
    let mut i = 0;
    while i + 2 < chars.len() {
        let (_, drive) = chars[i];
        let (_, colon) = chars[i + 1];
        let (_, slash) = chars[i + 2];
        if drive.is_ascii_alphabetic() && colon == ':' && (slash == '\\' || slash == '/') {
            let start = chars[i].0;
            let mut end = text.len();
            let mut j = i + 3;
            while j < chars.len() {
                let (idx, ch) = chars[j];
                if is_path_delimiter(ch) {
                    end = idx;
                    break;
                }
                j += 1;
            }
            let path = trim_path_tail(&text[start..end]);
            if !path.is_empty() && !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_string());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    paths
}

const fn is_path_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '，' | '。'
                | '；'
                | ';'
                | '、'
                | '"'
                | '\''
                | '`'
                | '?'
                | '*'
                | '|'
                | '<'
                | '>'
                | ')'
                | '）'
                | ']'
                | '】'
        )
}

fn trim_path_tail(path: &str) -> &str {
    path.trim_end_matches([',', ':', '：', '.'])
}

fn infer_module_name(text: &str) -> Option<String> {
    let (_, tail) = text.split_once('为')?;
    let (module, _) = tail.split_once("模块")?;
    let module = module
        .trim()
        .trim_start_matches("一个")
        .trim_start_matches("一份")
        .trim();
    if module.is_empty() {
        None
    } else {
        Some(module.to_string())
    }
}

fn infer_intent_type(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("code")
        || lower.contains("search")
        || text.contains("代码")
        || text.contains("模块")
        || text.contains("源码")
    {
        return Some("code_search".to_string());
    }
    None
}

fn infer_intent_route(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "needs_clarification".to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("unsupported") || trimmed.contains("不支持") {
        return "unsupported".to_string();
    }

    let complex_markers = [
        "复杂",
        "多个",
        "多条",
        "跨",
        "依赖",
        "并行",
        "分解",
        "拆分",
        "架构",
        "全局",
        "scout",
        "fanout",
        "parallel",
        "dependency",
    ];
    if complex_markers
        .iter()
        .any(|marker| lower.contains(marker) || trimmed.contains(marker))
    {
        return "complex".to_string();
    }

    "simple".to_string()
}

fn infer_target(text: &str) -> Option<String> {
    for marker in ["探索一下", "探索", "查一下", "看看", "了解一下", "search"] {
        let Some((_, tail)) = text.split_once(marker) else {
            continue;
        };
        let mut end = tail.len();
        for boundary in ["模块", "内容", "代码", "，", "。", ",", ".", "\n"] {
            if let Some(index) = tail.find(boundary) {
                end = end.min(index);
            }
        }
        let target = tail[..end]
            .trim()
            .trim_matches(|ch: char| ch == ':' || ch == '：' || ch.is_whitespace());
        if !target.is_empty() {
            return Some(target.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_preserves_hidden_dot_path_segment() {
        let text = "请基于 D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph 的源码，输出到 D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow。";
        let paths = extract_windows_paths(text);
        assert_eq!(
            paths,
            vec![
                "D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph",
                "D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow"
            ]
        );
        assert_eq!(
            path_after_marker(text, "输出到").as_deref(),
            Some("D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow")
        );
    }

    #[test]
    fn extract_stops_at_lossy_question_mark_delimiters() {
        let text = "?? D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\topology ?? KB wiki ? D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow-smoke????? source anchors";
        let paths = extract_windows_paths(text);
        assert_eq!(
            paths,
            vec![
                "D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\topology",
                "D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow-smoke",
            ]
        );
    }

    #[test]
    fn extract_trims_sentence_periods_without_damaging_extension() {
        let text = "Source roots: D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\topology and D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\nodes\\topology_mutation.rs. Output to D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow.";
        let paths = extract_windows_paths(text);
        assert_eq!(
            paths,
            vec![
                "D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\topology",
                "D:\\Dev\\blackboard\\bb_backend\\crates\\bb_core\\src\\task_graph\\nodes\\topology_mutation.rs",
                "D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow",
            ]
        );
        assert_eq!(
            path_after_marker(text, "Output to").as_deref(),
            Some("D:\\Dev\\blackboard\\.bb_template\\runtime\\kb-workflow")
        );
    }

    #[test]
    fn infer_target_from_module_prompt() {
        let text = "帮我探索一下 task_graph runtime 模块的内容";
        assert_eq!(infer_target(text).as_deref(), Some("task_graph runtime"));
    }

    #[test]
    fn infer_intent_route_marks_complex_prompts() {
        let text = "帮我探索这个跨多个模块的复杂依赖关系";
        assert_eq!(infer_intent_route(text), "complex");
    }
}
