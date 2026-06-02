use anyhow::{Context, Result};
use bb_core::task_graph::{self, GraphRef, RunOutcome, TaskGraphDefinition};
use bb_core::{InboxNote, ProjectEntry, Workspace};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::http::task_graph::runner_config::{
    build_runner_opts, read_runner_feishu, resolve_overrides_from_profile, RunnerOverrides,
};

// ---------------------------------------------------------------------------
// Public options (from CLI)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DaemonOptions {
    pub graph: String,
    pub agent: String,
    pub model: Option<String>,
    pub opencode: Option<String>,
    pub codex: Option<String>,
    pub codebuddy: Option<String>,
    pub pi: Option<String>,
    pub interval_seconds: u64,
    pub max_dispatch_per_scan: usize,
    pub node_timeout_seconds: Option<u64>,
    pub run_timeout_seconds: Option<u64>,
    pub project: Option<String>,
    pub once: bool,
    pub dry_run: bool,
    pub watch: bool,
    pub schedules: bool,
    pub retry_failed: bool,
}

// ---------------------------------------------------------------------------
// Graph reference parsing
// ---------------------------------------------------------------------------

/// Parse a `"<scope>/<id>"` string into `(scope, id)`.
fn parse_graph_ref(graph: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = graph.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        anyhow::bail!(
            "invalid --graph format: '{graph}'. Expected '<scope>/<id>', e.g. 'system/inbox-batch-cleanup' or 'project/my-graph'"
        );
    }
    let scope = parts[0];
    if scope != "system" && scope != "project" {
        anyhow::bail!("invalid graph scope: '{scope}'. Must be 'system' or 'project'");
    }
    Ok((scope, parts[1]))
}

/// Load a `TaskGraphDefinition` by scope and id.
fn load_graph(
    workspace: &Workspace,
    scope: &str,
    id: &str,
    project: &str,
) -> Result<TaskGraphDefinition> {
    match scope {
        "system" => task_graph::read_system_graph(workspace.root(), id)
            .with_context(|| format!("load system graph '{id}'")),
        "project" => task_graph::read_project_graph(workspace.root(), project, id)
            .with_context(|| format!("load project graph '{id}' for project '{project}'")),
        _ => anyhow::bail!("unsupported graph scope: {scope}"),
    }
}

// ---------------------------------------------------------------------------
// Core graph execution
// ---------------------------------------------------------------------------

/// Result of a single graph run: run id, terminal outcome, and wall-clock elapsed.
#[derive(Debug, Clone)]
struct GraphRunResult {
    run_id: String,
    outcome: RunOutcome,
    elapsed: Duration,
}

/// Create a Task Graph run and execute it to completion.
fn run_graph(
    workspace: &Workspace,
    options: &DaemonOptions,
    graph: &TaskGraphDefinition,
    graph_ref: GraphRef,
    project: &str,
    input: Value,
) -> Result<GraphRunResult> {
    // Validate graph before running
    let pre_run_errors = task_graph::validate_pre_run(graph, workspace.root(), project);
    if !pre_run_errors.is_empty() {
        let msgs: Vec<String> = pre_run_errors
            .iter()
            .map(|e| format!("[{}] {}: {}", e.code, e.path, e.message))
            .collect();
        anyhow::bail!(
            "graph pre-run validation failed ({} errors):\n  {}",
            msgs.len(),
            msgs.join("\n  ")
        );
    }

    let run = task_graph::create_run(workspace.root(), project, graph_ref, graph, input)
        .with_context(|| format!("create run for project '{project}'"))?;

    eprintln!(
        "bb-daemon run created: id={} project={} graph={}",
        run.id, project, options.graph
    );

    let overrides = RunnerOverrides {
        model: options.model.clone(),
        dry_run: options.dry_run,
        codex_path: options.codex.clone(),
        codebuddy_path: options.codebuddy.clone(),
        opencode_path: options.opencode.clone(),
        pi_path: options.pi.clone(),
        agent: Some(options.agent.clone()),
        node_timeout_secs: options.node_timeout_seconds,
        run_timeout_secs: options.run_timeout_seconds,
        ..Default::default()
    };
    // Merge with Agent Profile defaults (CLI takes precedence)
    let overrides = resolve_overrides_from_profile(workspace.root(), &overrides);

    // Inject skills from Agent Profile into the working directory (Ticket #000050)
    if !overrides.skills.is_empty() {
        let runtime = overrides
            .agent
            .as_deref()
            .and_then(|agent_id| {
                bb_core::agents_registry::list_agents(workspace.root())
                    .ok()
                    .and_then(|reg| reg.agents.into_iter().find(|a| a.id == agent_id))
                    .and_then(|a| a.runtime)
            })
            .unwrap_or_else(|| "opencode".to_string());
        let cwd = workspace.root();
        if let Err(e) = bb_core::skills::inject_skills_for_runtime(
            workspace.root(),
            &runtime,
            &overrides.skills,
            cwd,
        ) {
            eprintln!("bb-daemon warning: failed to inject skills: {e}");
        }
    }

    let opts = build_runner_opts(
        workspace.root(),
        project.to_string(),
        run.id.clone(),
        Some(&overrides),
    );

    let started = Instant::now();
    let outcome = task_graph::execute_run(&opts)
        .with_context(|| format!("execute run {} for project '{}'", run.id, project))?;

    Ok(GraphRunResult {
        run_id: run.id.clone(),
        outcome,
        elapsed: started.elapsed(),
    })
}

// ---------------------------------------------------------------------------
// Feishu 终态通知（088）。唯一允许调度飞书通知的位置。
// ---------------------------------------------------------------------------

/// 按 run 终态调度一次非阻塞飞书通知。
/// `join_on_exit=true`（direct-run/single-run，进程将立即退出）时用 channel
/// `recv_timeout` 有界等待后台发送完成，避免分离线程被进程退出杀掉而丢通知；
/// `false`（daemon 常驻 loop）时 detach。任何错误只脱敏 warn，不改变 run 结果。
pub(crate) fn spawn_feishu_notification(
    root: &Path,
    project: &str,
    graph_id: &str,
    run_id: &str,
    outcome: &RunOutcome,
    elapsed: Duration,
    join_on_exit: bool,
) {
    use bb_core::feishu::{FeishuConfig, FeishuNotification, FeishuTerminal};

    // S3：Cancelled 显式跳过；其余映射为白名单终态标签（不取 message/node_id）。
    let terminal = match outcome {
        RunOutcome::Succeeded => FeishuTerminal::Succeeded,
        RunOutcome::Paused { .. } => FeishuTerminal::Paused,
        RunOutcome::Failed { .. } => FeishuTerminal::Failed,
        RunOutcome::Cancelled => return,
    };

    let toml_cfg = read_runner_feishu(root);
    let cfg = match FeishuConfig::from_toml_and_env(toml_cfg.as_ref()) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return, // 未启用
        Err(err) => {
            eprintln!("bb-daemon feishu config invalid, skip notification: {err}");
            return;
        }
    };

    let detail_url = cfg.detail_url(project, run_id);
    let notification = FeishuNotification {
        project: project.to_string(),
        graph_id: graph_id.to_string(),
        run_id: run_id.to_string(),
        terminal,
        elapsed,
        detail_url,
    };
    let join_timeout = cfg.connect_timeout + cfg.read_timeout + Duration::from_secs(1);

    let spawned = thread::Builder::new()
        .name("bb-feishu-notify".to_string())
        .spawn(move || {
            if let Err(err) = bb_core::feishu::send_notification(&cfg, &notification) {
                eprintln!("bb-daemon feishu push failed: {err}");
            }
        });

    match spawned {
        Ok(handle) => {
            if join_on_exit {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = handle.join();
                    let _ = tx.send(());
                });
                if rx.recv_timeout(join_timeout).is_err() {
                    eprintln!("bb-daemon feishu push not confirmed within join timeout on exit");
                }
            }
            // daemon 常驻模式：detach（不 join）。
        }
        Err(err) => {
            eprintln!("bb-daemon skip feishu push: spawn failed: {err}");
        }
    }
}

// ---------------------------------------------------------------------------
// Handoff state persistence (for watch mode deduplication)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize, Serialize)]
struct HandoffState {
    entries: BTreeMap<String, HandoffStateEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct HandoffStateEntry {
    content_hash: String,
    status: String,
    attempts: u32,
    updated_at: String,
    run_id: Option<String>,
    error: Option<String>,
}

struct Dispatch {
    project: String,
    note: InboxNote,
    content_hash: String,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(workspace: Workspace, options: DaemonOptions) -> Result<()> {
    let parsed_graph = if options.schedules {
        None
    } else {
        Some(parse_graph_ref(&options.graph)?)
    };

    eprintln!(
        "bb-daemon starting: root={} graph={} agent={} watch={}",
        workspace.root().display(),
        options.graph,
        options.agent,
        options.watch,
    );

    if options.schedules {
        loop {
            let count = crate::http::task_graph::dispatch_due_schedules(&workspace)
                .map_err(|err| anyhow::anyhow!(err.0.to_string()))?;
            if options.once {
                eprintln!("bb-daemon schedule scan complete: dispatched={count}");
                return Ok(());
            }
            if count == 0 {
                eprintln!(
                    "bb-daemon schedule idle: interval={}s",
                    options.interval_seconds
                );
            } else {
                eprintln!("bb-daemon schedule dispatched: {count}");
            }
            thread::sleep(Duration::from_secs(options.interval_seconds.max(1)));
        }
    }

    let (scope, graph_id) = parsed_graph.expect("non-schedule daemon mode parses graph");

    if options.watch {
        // Watch mode: poll inbox for new notes
        let state_path = state_path(workspace.root(), &options.agent, graph_id);
        let mut state = read_state(&state_path)?;

        loop {
            let count = scan_inbox(
                &workspace,
                &options,
                scope,
                graph_id,
                &state_path,
                &mut state,
            )?;

            if options.once {
                eprintln!("bb-daemon scan complete: dispatched={count}");
                return Ok(());
            }

            if count == 0 {
                eprintln!(
                    "bb-daemon idle: no new work for graph={} agent={} interval={}s",
                    options.graph, options.agent, options.interval_seconds
                );
            }
            thread::sleep(Duration::from_secs(options.interval_seconds.max(1)));
        }
    } else {
        // Single-run mode: execute graph once per project
        let projects = candidate_projects(&workspace, &options)?;
        let mut any_failed = false;

        for project in &projects {
            let graph = load_graph(&workspace, scope, graph_id, &project.name)?;
            let graph_ref = GraphRef {
                scope: graph.scope,
                id: graph.id.clone(),
                version: graph.version,
            };

            eprintln!(
                "bb-daemon dispatch: graph={} project={} agent={}",
                options.graph, project.name, options.agent
            );

            let input = serde_json::json!({
                "project": project.name,
            });

            match run_graph(
                &workspace,
                &options,
                &graph,
                graph_ref,
                &project.name,
                input,
            ) {
                Ok(result) => {
                    // direct-run/single-run：进程将退出，join 等待通知发送完成。
                    spawn_feishu_notification(
                        workspace.root(),
                        &project.name,
                        &graph.id,
                        &result.run_id,
                        &result.outcome,
                        result.elapsed,
                        true,
                    );
                    match result.outcome {
                        RunOutcome::Succeeded => {
                            eprintln!("bb-daemon run succeeded: project={}", project.name);
                        }
                        RunOutcome::Paused { node_id } => {
                            eprintln!(
                                "bb-daemon run paused at human gate: project={} node={}",
                                project.name, node_id
                            );
                        }
                        RunOutcome::Failed { node_id, message } => {
                            eprintln!(
                                "bb-daemon run failed: project={} node={} error={}",
                                project.name, node_id, message
                            );
                            any_failed = true;
                        }
                        RunOutcome::Cancelled => {
                            eprintln!("bb-daemon run cancelled: project={}", project.name);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("bb-daemon run error: project={} error={}", project.name, e);
                    any_failed = true;
                }
            }

            if options.once {
                break;
            }
        }

        if any_failed {
            anyhow::bail!("one or more graph runs failed");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Watch-triggered mode (inbox scanning)
// ---------------------------------------------------------------------------

fn scan_inbox(
    workspace: &Workspace,
    options: &DaemonOptions,
    scope: &str,
    graph_id: &str,
    state_path: &Path,
    state: &mut HandoffState,
) -> Result<usize> {
    let projects = candidate_projects(workspace, options)?;
    let mut dispatched = 0;

    for project in projects {
        let graph = load_graph(workspace, scope, graph_id, &project.name)?;
        let graph_ref = GraphRef {
            scope: graph.scope,
            id: graph.id.clone(),
            version: graph.version,
        };

        let board = workspace
            .open_project(&project.name)
            .with_context(|| format!("open project {}", project.name))?;
        for entry in board
            .list_notes()
            .with_context(|| format!("list inbox notes for {}", project.name))?
        {
            let note = board
                .read_note(&entry.name)
                .with_context(|| format!("read inbox note {}/{}", project.name, entry.name))?;
            let dispatch = Dispatch {
                project: project.name.clone(),
                content_hash: stable_hash(&note.content),
                note,
            };
            if should_skip(&dispatch, state, options.retry_failed) {
                continue;
            }

            eprintln!(
                "bb-daemon dispatch: graph={} project={} note={} agent={} hash={}",
                options.graph,
                dispatch.project,
                dispatch.note.name,
                options.agent,
                dispatch.content_hash
            );

            if options.dry_run {
                eprintln!(
                    "bb-daemon dry-run: would create run for note {}",
                    dispatch.note.name
                );
                record_result(state, &dispatch, "dry_run", None, None);
                write_state(state_path, state)?;
                dispatched += 1;
                if options.max_dispatch_per_scan > 0 && dispatched >= options.max_dispatch_per_scan
                {
                    return Ok(dispatched);
                }
                continue;
            }

            // Build graph input with inbox note context
            let input = serde_json::json!({
                "project": dispatch.project,
                "note_name": dispatch.note.name,
                "note_content": dispatch.note.content,
            });

            let result = run_graph(
                workspace,
                options,
                &graph,
                graph_ref.clone(),
                &dispatch.project,
                input,
            );

            match &result {
                Ok(run_result) => {
                    // watch/daemon loop：进程继续存活，detach（不 join）。
                    spawn_feishu_notification(
                        workspace.root(),
                        &dispatch.project,
                        &graph.id,
                        &run_result.run_id,
                        &run_result.outcome,
                        run_result.elapsed,
                        false,
                    );
                    match &run_result.outcome {
                        RunOutcome::Succeeded => {
                            record_result(state, &dispatch, "completed", None, None);
                        }
                        RunOutcome::Paused { node_id } => {
                            record_result(
                                state,
                                &dispatch,
                                "paused",
                                None,
                                Some(format!("paused at gate: {node_id}")),
                            );
                        }
                        RunOutcome::Failed { node_id, message } => {
                            record_result(
                                state,
                                &dispatch,
                                "failed",
                                None,
                                Some(format!("node={node_id}: {message}")),
                            );
                        }
                        RunOutcome::Cancelled => {
                            record_result(state, &dispatch, "cancelled", None, None);
                        }
                    }
                }
                Err(e) => {
                    record_result(state, &dispatch, "failed", None, Some(e.to_string()));
                }
            }

            write_state(state_path, state)?;
            dispatched += 1;
            if options.max_dispatch_per_scan > 0 && dispatched >= options.max_dispatch_per_scan {
                return Ok(dispatched);
            }
        }
    }

    Ok(dispatched)
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

fn candidate_projects(workspace: &Workspace, options: &DaemonOptions) -> Result<Vec<ProjectEntry>> {
    let mut projects = workspace.list_projects().context("list projects")?;
    if let Some(project) = &options.project {
        projects.retain(|entry| entry.name == *project);
        if projects.is_empty() {
            workspace
                .open_project(project)
                .with_context(|| format!("project not found: {project}"))?;
        }
    }
    Ok(projects)
}

fn should_skip(dispatch: &Dispatch, state: &HandoffState, retry_failed: bool) -> bool {
    let key = state_key(&dispatch.project, &dispatch.note.name);
    let Some(entry) = state.entries.get(&key) else {
        return false;
    };
    if entry.content_hash != dispatch.content_hash {
        return false;
    }
    if entry.status == "dry_run" {
        return false;
    }
    if entry.status == "failed" {
        return !retry_failed;
    }
    // Skip completed, paused, cancelled
    true
}

fn record_result(
    state: &mut HandoffState,
    dispatch: &Dispatch,
    status: &str,
    run_id: Option<String>,
    error: Option<String>,
) {
    let key = state_key(&dispatch.project, &dispatch.note.name);
    let attempts = state
        .entries
        .get(&key)
        .map_or(1, |entry| entry.attempts.saturating_add(1));
    state.entries.insert(
        key,
        HandoffStateEntry {
            content_hash: dispatch.content_hash.clone(),
            status: status.to_string(),
            attempts,
            updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            run_id,
            error,
        },
    );
}

fn state_key(project: &str, note_name: &str) -> String {
    format!("{project}/{note_name}")
}

fn state_path(root: &Path, agent: &str, graph_id: &str) -> PathBuf {
    root.join("runtime").join(format!(
        "{}-{}-state.json",
        sanitize_state_name(agent),
        sanitize_state_name(graph_id)
    ))
}

fn read_state(path: &Path) -> Result<HandoffState> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).context("parse daemon handoff state"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(HandoffState::default()),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn write_state(path: &Path, state: &HandoffState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, content).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))
}

fn stable_hash(content: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn sanitize_state_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    if out.is_empty() {
        "agent".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_changes_with_content() {
        assert_eq!(stable_hash("same"), stable_hash("same"));
        assert_ne!(stable_hash("same"), stable_hash("different"));
    }

    #[test]
    fn parse_graph_ref_valid() {
        let (scope, id) = parse_graph_ref("system/inbox-batch-cleanup").unwrap();
        assert_eq!(scope, "system");
        assert_eq!(id, "inbox-batch-cleanup");
    }

    #[test]
    fn parse_graph_ref_project() {
        let (scope, id) = parse_graph_ref("project/my-graph").unwrap();
        assert_eq!(scope, "project");
        assert_eq!(id, "my-graph");
    }

    #[test]
    fn parse_graph_ref_invalid_format() {
        assert!(parse_graph_ref("no-slash").is_err());
        assert!(parse_graph_ref("/no-scope").is_err());
        assert!(parse_graph_ref("system/").is_err());
    }

    #[test]
    fn parse_graph_ref_invalid_scope() {
        assert!(parse_graph_ref("unknown/some-id").is_err());
    }
}
