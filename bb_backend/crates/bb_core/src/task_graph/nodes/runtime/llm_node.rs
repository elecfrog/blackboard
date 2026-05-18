use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::agent_session::{
    self, AgentResultStatus, AgentSessionParent, AgentSessionSummary, AgentTurnRequest,
    CreateAgentSession,
};
use crate::fs_util::resolve_slash;
use crate::skills;
use crate::task_graph::definition::types::{
    LlmConfig, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::llm::{resolve_llm_invocation, ResolvedLlmInvocation};
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::runtime::{
    artifact_content_for_llm_config, capture_runtime_output, output_value_for_llm_config,
    run_is_cancelled, run_runtime_command, runtime_failure_message, tail_str, write_node_artifact,
};
use crate::task_graph::runtime_concurrency;

// ─── LLM Node ────────────────────────────────────────────────────────────────

pub fn execute_llm_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;

    let config: LlmConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let mut invocation = resolve_llm_invocation(opts, &config, &run.context)?;
    let exec_runtime = invocation.runtime.clone();
    let exec_agent = invocation.agent.clone();
    let exec_model = invocation.model.clone();

    let start_time = Utc::now().to_rfc3339();

    // 日志是流式的，允许直接写入
    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] LLM node starting: mode={:?} runtime={} agent={}",
            start_time, invocation.run_as, invocation.runtime, invocation.agent
        ),
    )?;

    // 立即写入 running 状态的 node_state，让前端 SSE 能实时看到节点正在执行
    let running_state = TaskGraphRunNode {
        node_id: node.id.clone(),
        status: NodeRunStatus::Running,
        started_at: Some(start_time.clone()),
        completed_at: None,
        duration_ms: None,
        iteration: None,
        exit_code: None,
        error: None,
        output_artifact: None,
        log_tail: Some(format!("LLM node starting: runtime={}", invocation.runtime)),
        child_run_id: None,
        runtime: Some(exec_runtime.clone()),
        agent: Some(exec_agent.clone()),
        model: exec_model.clone(),
        agent_session_id: None,
        agent_session: None,
    };
    run_state::update_node_state(ws, project, run_id, &running_state)?;

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!("[rendered_prompt]\n{}", invocation.prompt),
    )?;

    if opts.dry_run {
        let end_time = Utc::now().to_rfc3339();
        let dry_output = serde_json::json!({ "dry_run": true, "prompt": invocation.prompt });
        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            output: Some(dry_output),
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(0),
                iteration: None,
                exit_code: Some(0),
                error: None,
                output_artifact: None,
                log_tail: Some("[dry_run] skipped execution".to_string()),
                child_run_id: None,
                runtime: Some(exec_runtime),
                agent: Some(exec_agent),
                model: exec_model,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: None,
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        });
    }

    let _runtime_permit =
        if let Some(limit) = runtime_concurrency::configured_limit(ws, &invocation.runtime) {
            run_state::append_node_log(
                ws,
                project,
                run_id,
                &node.id,
                &format!(
                    "[{}] waiting for runtime concurrency permit: runtime={} limit={}",
                    Utc::now().to_rfc3339(),
                    invocation.runtime,
                    limit
                ),
            )?;
            let (permit, info) = runtime_concurrency::acquire(ws, &invocation.runtime, limit);
            run_state::append_node_log(
                ws,
                project,
                run_id,
                &node.id,
                &format!(
                    "[{}] acquired runtime concurrency permit: runtime={} limit={} waited_ms={}",
                    Utc::now().to_rfc3339(),
                    invocation.runtime,
                    info.limit,
                    info.waited_ms
                ),
            )?;
            Some(permit)
        } else {
            None
        };

    prepare_node_skills(opts, project, run_id, &node.id, &mut invocation)?;

    if runtime_uses_agent_session(&invocation.runtime) {
        return execute_agent_session_node(
            opts, node, run, _edge_map, &config, invocation, start_time,
        );
    }

    // Build and run the runtime command
    let start_instant = Instant::now();
    let output = run_runtime_command(opts, &invocation, project, &node.id);
    let duration_ms = start_instant.elapsed().as_millis() as u64;
    let end_time = Utc::now().to_rfc3339();

    match output {
        Ok(cmd_output) => {
            let stdout = String::from_utf8_lossy(&cmd_output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&cmd_output.stderr).to_string();
            let capture = capture_runtime_output(&invocation.runtime, &stdout, &stderr);
            let exit_code = cmd_output.status.code();

            if run_is_cancelled(ws, project, run_id) {
                let node_state = TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    started_at: Some(start_time),
                    completed_at: Some(end_time),
                    duration_ms: Some(duration_ms),
                    iteration: None,
                    exit_code,
                    error: Some(NodeError {
                        code: "cancelled".to_string(),
                        message: "Run was cancelled during execution".to_string(),
                    }),
                    output_artifact: None,
                    log_tail: Some(tail_str(&capture.log, 4096)),
                    child_run_id: None,
                    runtime: Some(exec_runtime),
                    agent: Some(exec_agent),
                    model: exec_model,
                    agent_session_id: None,
                    agent_session: None,
                };
                return Ok(NodeOutcome {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    output: None,
                    node_state,
                    side_effects: vec![],
                    child_run_id: None,
                    end_result: None,
                    control: vec![],
                    graph_mutations: vec![],
                });
            }

            let success = cmd_output.status.success() && capture.error_message.is_none();

            run_state::append_node_log(
                ws,
                project,
                run_id,
                &node.id,
                &format!("[{end_time}] exit_code={exit_code:?} success={success}"),
            )?;
            if !capture.log.trim().is_empty() {
                run_state::append_node_log(
                    ws,
                    project,
                    run_id,
                    &node.id,
                    &tail_str(&capture.log, 2048),
                )?;
            }

            if !success {
                let tail = runtime_failure_message(&capture, exit_code);
                let node_state = TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    started_at: Some(start_time),
                    completed_at: Some(end_time),
                    duration_ms: Some(duration_ms),
                    iteration: None,
                    exit_code,
                    error: Some(NodeError {
                        code: "runtime_failed".to_string(),
                        message: tail.clone(),
                    }),
                    output_artifact: None,
                    log_tail: Some(tail),
                    child_run_id: None,
                    runtime: Some(exec_runtime),
                    agent: Some(exec_agent),
                    model: exec_model,
                    agent_session_id: None,
                    agent_session: None,
                };
                return Ok(NodeOutcome {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Failed,
                    output: None,
                    node_state,
                    side_effects: vec![],
                    child_run_id: None,
                    end_result: None,
                    control: vec![],
                    graph_mutations: vec![],
                });
            }

            // Success path
            let output_value =
                output_value_for_llm_config(&config, &capture.parse_source, &capture.artifact);
            let artifact_content =
                artifact_content_for_llm_config(&config, &capture.artifact, &output_value);

            let artifact =
                write_node_artifact(ws, project, run_id, &node.id, &artifact_content, &config)?;

            let log_tail = tail_str(&capture.log, 4096);

            Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                output: Some(output_value),
                node_state: TaskGraphRunNode {
                    node_id: node.id.clone(),
                    status: NodeRunStatus::Succeeded,
                    started_at: Some(start_time),
                    completed_at: Some(end_time),
                    duration_ms: Some(duration_ms),
                    iteration: None,
                    exit_code,
                    error: None,
                    output_artifact: artifact,
                    log_tail: Some(log_tail),
                    child_run_id: None,
                    runtime: Some(exec_runtime),
                    agent: Some(exec_agent),
                    model: exec_model,
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
        Err(e) => {
            let node_state = TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(duration_ms),
                iteration: None,
                exit_code: None,
                error: Some(NodeError {
                    code: "spawn_failed".to_string(),
                    message: e.to_string(),
                }),
                output_artifact: None,
                log_tail: Some(e.to_string()),
                child_run_id: None,
                runtime: Some(exec_runtime),
                agent: Some(exec_agent),
                model: exec_model,
                agent_session_id: None,
                agent_session: None,
            };
            Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                output: None,
                node_state,
                side_effects: vec![],
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_agent_session_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    _run: &TaskGraphRun,
    _edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
    config: &LlmConfig,
    invocation: ResolvedLlmInvocation,
    start_time: String,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;
    let exec_runtime = invocation.runtime.clone();
    let exec_agent = invocation.agent.clone();
    let exec_model = invocation.model.clone();
    let max_attempts = config.retry.effective_max_attempts();
    let backoff_ms = config.retry.effective_backoff_ms();
    let overall_started = Instant::now();
    let mut attempt = 1;

    loop {
        let session_title = if max_attempts > 1 {
            format!("{} (attempt {attempt}/{max_attempts})", node.label)
        } else {
            node.label.clone()
        };
        let session = agent_session::create_session(
            ws,
            CreateAgentSession {
                project: project.clone(),
                title: Some(session_title),
                runtime: exec_runtime.clone(),
                agent: exec_agent.clone(),
                model: exec_model.clone(),
                variant: invocation.variant.clone(),
                parent: Some(AgentSessionParent::TaskGraphNode {
                    run_id: run_id.clone(),
                    node_id: node.id.clone(),
                }),
            },
        )
        .map_err(agent_session_error)?;

        run_state::append_node_log(
            ws,
            project,
            run_id,
            &node.id,
            &format!(
                "[{}] AgentSession attempt {}/{} session={}",
                Utc::now().to_rfc3339(),
                attempt,
                max_attempts,
                session.id
            ),
        )?;

        let running_state = TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Running,
            started_at: Some(start_time.clone()),
            completed_at: None,
            duration_ms: None,
            iteration: None,
            exit_code: None,
            error: None,
            output_artifact: None,
            log_tail: Some(format!(
                "AgentSession attempt {attempt}/{max_attempts}: runtime={exec_runtime}"
            )),
            child_run_id: None,
            runtime: Some(exec_runtime.clone()),
            agent: Some(exec_agent.clone()),
            model: exec_model.clone(),
            agent_session_id: Some(session.id.clone()),
            agent_session: Some(agent_session::session_summary(&session)),
        };
        run_state::update_node_state(ws, project, run_id, &running_state)?;

        let outcome = agent_session::run_turn(
            AgentTurnRequest {
                workspace_root: ws.clone(),
                scripts_dir: opts.scripts_dir.clone(),
                execution_root: ws.clone(),
                project: project.clone(),
                session_id: session.id.clone(),
                runtime: exec_runtime.clone(),
                agent: exec_agent.clone(),
                model: exec_model.clone(),
                variant: invocation.variant.clone(),
                continue_provider_session_id: None,
                prompt: invocation.prompt.clone(),
                codex_path: opts.codex_path.clone(),
                codex_config_args: invocation.codex_config_args.clone(),
                codebuddy_path: opts.codebuddy_path.clone(),
                codebuddy_mcp_config_content: invocation.codebuddy_mcp_config_content.clone(),
                codebuddy_settings_json: invocation.codebuddy_settings_json.clone(),
                opencode_path: opts.opencode_path.clone(),
                opencode_config_content: invocation.opencode_config_content.clone(),
                pi_path: opts.pi_path.clone(),
                pi_mcp_config_content: invocation.pi_mcp_config_content.clone(),
                tool_policy: invocation.tool_policy.clone(),
                custom_env: invocation.custom_env.clone(),
                custom_args: invocation.custom_args.clone(),
                timeout: opts.node_timeout,
                startup_timeout: agent_session::effective_agent_startup_timeout(opts.node_timeout),
            },
            || run_is_cancelled(ws, project, run_id),
        )
        .map_err(agent_session_error)?;

        let end_time = Utc::now().to_rfc3339();
        let duration_ms = overall_started.elapsed().as_millis() as u64;
        let exit_code = outcome.exit_code;
        let session_summary = outcome.session.clone();

        if run_is_cancelled(ws, project, run_id)
            || outcome.result.status == AgentResultStatus::Cancelled
        {
            let node_state = node_state_with_session(
                node,
                NodeRunStatus::Failed,
                start_time,
                end_time,
                duration_ms,
                exit_code,
                Some(NodeError {
                    code: "cancelled".to_string(),
                    message: "Run was cancelled during execution".to_string(),
                }),
                None,
                Some("AgentSession cancelled".to_string()),
                exec_runtime,
                exec_agent,
                exec_model,
                session.id,
                session_summary,
            );
            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                output: None,
                node_state,
                side_effects: vec![],
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            });
        }

        if outcome.result.status == AgentResultStatus::Completed {
            let output_value =
                output_value_for_llm_config(config, &outcome.parse_source, &outcome.artifact);
            let artifact_content =
                artifact_content_for_llm_config(config, &outcome.artifact, &output_value);
            let artifact =
                write_node_artifact(ws, project, run_id, &node.id, &artifact_content, config)?;

            return Ok(NodeOutcome {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                output: Some(output_value),
                node_state: node_state_with_session(
                    node,
                    NodeRunStatus::Succeeded,
                    start_time,
                    end_time,
                    duration_ms,
                    exit_code,
                    None,
                    artifact,
                    None,
                    exec_runtime,
                    exec_agent,
                    exec_model,
                    session.id,
                    session_summary,
                ),
                side_effects: vec![],
                child_run_id: None,
                end_result: None,
                control: vec![],
                graph_mutations: vec![],
            });
        }

        let message = outcome
            .result
            .error
            .clone()
            .unwrap_or_else(|| tail_str(&outcome.log, 2048));
        if attempt < max_attempts && is_transient_agent_failure(outcome.result.status, &message) {
            let sleep_ms = retry_backoff_ms(backoff_ms, attempt);
            run_state::append_node_log(
                ws,
                project,
                run_id,
                &node.id,
                &format!(
                    "[{}] transient AgentSession failure on attempt {}/{}; retrying in {}ms: {}",
                    Utc::now().to_rfc3339(),
                    attempt,
                    max_attempts,
                    sleep_ms,
                    tail_str(&message, 512)
                ),
            )?;
            if sleep_ms > 0 {
                thread::sleep(Duration::from_millis(sleep_ms));
            }
            attempt += 1;
            continue;
        }

        let log_tail = if attempt > 1 {
            format!("{message}\ntransient_retry_attempts={attempt}/{max_attempts}")
        } else {
            message.clone()
        };
        let node_state = node_state_with_session(
            node,
            NodeRunStatus::Failed,
            start_time,
            end_time,
            duration_ms,
            exit_code,
            Some(NodeError {
                code: "runtime_failed".to_string(),
                message,
            }),
            None,
            Some(log_tail),
            exec_runtime,
            exec_agent,
            exec_model,
            session.id,
            session_summary,
        );
        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            output: None,
            node_state,
            side_effects: vec![],
            child_run_id: None,
            end_result: None,
            control: vec![],
            graph_mutations: vec![],
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn node_state_with_session(
    node: &TaskGraphNode,
    status: NodeRunStatus,
    start_time: String,
    end_time: String,
    duration_ms: u64,
    exit_code: Option<i32>,
    error: Option<NodeError>,
    output_artifact: Option<run_state::OutputArtifact>,
    log_tail: Option<String>,
    runtime: String,
    agent: String,
    model: Option<String>,
    session_id: String,
    session: AgentSessionSummary,
) -> TaskGraphRunNode {
    TaskGraphRunNode {
        node_id: node.id.clone(),
        status,
        started_at: Some(start_time),
        completed_at: Some(end_time),
        duration_ms: Some(duration_ms),
        iteration: None,
        exit_code,
        error,
        output_artifact,
        log_tail,
        child_run_id: None,
        runtime: Some(runtime),
        agent: Some(agent),
        model,
        agent_session_id: Some(session_id),
        agent_session: Some(session),
    }
}

fn prepare_node_skills(
    opts: &RunnerOptions,
    project: &str,
    run_id: &str,
    node_id: &str,
    invocation: &mut ResolvedLlmInvocation,
) -> Result<(), TaskGraphError> {
    if invocation.skills.is_empty() {
        return Ok(());
    }

    if invocation.runtime == "pi" {
        let snapshot_parent = opts
            .workspace_root
            .join("runtime")
            .join("task_graph_runs")
            .join(project)
            .join(run_id)
            .join("skill_snapshots");
        let label = format!("{run_id}-{node_id}");
        let snapshot = skills::create_skill_snapshot(
            &opts.workspace_root,
            &invocation.skills,
            &snapshot_parent,
            &label,
        )
        .map_err(|source| TaskGraphError::Io {
            path: snapshot_parent,
            source,
        })?;

        invocation.custom_args.push("--no-skills".to_string());
        invocation.custom_args.push("--skill".to_string());
        invocation.custom_args.push(resolve_slash(&snapshot.root));
        return Ok(());
    }

    skills::inject_skills_for_runtime(
        &opts.workspace_root,
        &invocation.runtime,
        &invocation.skills,
        &opts.workspace_root,
    )
    .map_err(|source| TaskGraphError::Io {
        path: opts.workspace_root.join("skills"),
        source,
    })
}

fn runtime_uses_agent_session(runtime: &str) -> bool {
    matches!(runtime, "opencode" | "codex" | "codebuddy" | "pi")
}

fn is_transient_agent_failure(status: AgentResultStatus, message: &str) -> bool {
    if status == AgentResultStatus::Timeout {
        return true;
    }
    if status != AgentResultStatus::Failed {
        return false;
    }

    let lower = message.to_ascii_lowercase();
    [
        "api_error",
        "output new_sensitive",
        "rate limit",
        "ratelimit",
        "too many requests",
        "temporarily",
        "temporary",
        "timeout",
        "timed out",
        "overloaded",
        "busy",
        "connection reset",
        "connection refused",
        "network",
        "econnreset",
        "etimedout",
        "epipe",
        "500",
        "502",
        "503",
        "504",
        "429",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn retry_backoff_ms(base_ms: u64, attempt: u32) -> u64 {
    base_ms.saturating_mul(u64::from(attempt))
}

fn agent_session_error(err: agent_session::AgentSessionError) -> TaskGraphError {
    TaskGraphError::Io {
        path: PathBuf::from("agent_session"),
        source: io::Error::other(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_transient_agent_failures() {
        assert!(is_transient_agent_failure(
            AgentResultStatus::Timeout,
            "agent session timed out"
        ));
        assert!(is_transient_agent_failure(
            AgentResultStatus::Failed,
            r#"{"type":"api_error","message":"output new_sensitive (1027)"}"#
        ));
        assert!(is_transient_agent_failure(
            AgentResultStatus::Failed,
            "provider returned 503"
        ));
        assert!(!is_transient_agent_failure(
            AgentResultStatus::Failed,
            "schema validation failed"
        ));
        assert!(!is_transient_agent_failure(
            AgentResultStatus::Cancelled,
            "agent session was cancelled"
        ));
    }
}
