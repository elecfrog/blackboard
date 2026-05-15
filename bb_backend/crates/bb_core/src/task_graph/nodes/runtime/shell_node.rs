use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::task_graph::definition::types::{
    ShellConfig, ShellPermission, TaskGraphEdge, TaskGraphError, TaskGraphNode,
};
use crate::task_graph::nodes::navigation::resolve_next_nodes;
use crate::task_graph::pregel::outcome::NodeOutcome;
use crate::task_graph::pregel::runner::RunnerOptions;
use crate::task_graph::run_state::{
    self, ArtifactContentType, NodeError, NodeRunStatus, TaskGraphRun, TaskGraphRunNode,
};
use crate::task_graph::runtime::{run_is_cancelled, strip_ansi_codes, tail_str};

const SHELL_RUNTIME_NAME: &str = "shell";
const LOG_CHUNK_BYTES: usize = 8192;
const OUTPUT_TAIL_CHARS: usize = 4096;

struct PreparedShell {
    cwd: PathBuf,
}

#[derive(Debug, Clone)]
struct ShellFailure {
    code: String,
    message: String,
}

struct ShellCapture {
    bytes: Vec<u8>,
    max_bytes: usize,
    truncated: bool,
}

impl ShellCapture {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            truncated: false,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        let remaining = self.max_bytes.saturating_sub(self.bytes.len());
        if chunk.len() > remaining {
            self.truncated = true;
        }
        if remaining > 0 {
            self.bytes
                .extend_from_slice(&chunk[..std::cmp::min(remaining, chunk.len())]);
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }
}

#[derive(Clone)]
struct ShellLogContext {
    workspace_root: PathBuf,
    project: String,
    run_id: String,
    node_id: String,
    strip_ansi: bool,
}

struct ShellProcessResult {
    status: ExitStatus,
    timed_out: bool,
    cancelled: bool,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

pub(crate) fn execute_shell_node(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    run: &TaskGraphRun,
    edge_map: &HashMap<String, Vec<&TaskGraphEdge>>,
) -> Result<NodeOutcome, TaskGraphError> {
    let ws = &opts.workspace_root;
    let project = &opts.project;
    let run_id = &opts.run_id;

    let config: ShellConfig =
        serde_json::from_value(node.config.clone()).map_err(|e| TaskGraphError::Parse {
            path: std::path::PathBuf::from(format!("node:{}", node.id)),
            source: e,
        })?;

    let start_time = Utc::now().to_rfc3339();
    let started = Instant::now();

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] shell node starting: command={} args={:?}",
            start_time, config.command, config.args
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
        log_tail: Some(format!("shell node starting: {}", config.command)),
        child_run_id: None,
        runtime: Some(SHELL_RUNTIME_NAME.to_string()),
        agent: None,
        model: None,
        agent_session_id: None,
        agent_session: None,
    };
    run_state::update_node_state(ws, project, run_id, &running_state)?;

    if opts.dry_run {
        let output = shell_output_json(
            true,
            &config,
            &config.cwd,
            None,
            0,
            false,
            false,
            "",
            "",
            false,
            false,
        );
        let next = resolve_next_nodes(edge_map, &node.id, node, &run.context, None)?;
        return Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            next_nodes: next,
            output: Some(output),
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                started_at: Some(start_time),
                completed_at: Some(Utc::now().to_rfc3339()),
                duration_ms: Some(0),
                iteration: None,
                exit_code: Some(0),
                error: None,
                output_artifact: None,
                log_tail: Some("[dry_run] skipped shell execution".to_string()),
                child_run_id: None,
                runtime: Some(SHELL_RUNTIME_NAME.to_string()),
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: None,
            end_result: None,
        });
    }

    let prepared = match prepare_shell(ws, &config) {
        Ok(prepared) => prepared,
        Err(failure) => {
            return shell_failed_outcome(
                ws,
                project,
                run_id,
                node,
                &config,
                start_time,
                started.elapsed().as_millis() as u64,
                None,
                false,
                false,
                "",
                "",
                false,
                false,
                failure,
            );
        }
    };

    if let Some(failure) = permission_failure(&config) {
        return shell_failed_outcome(
            ws,
            project,
            run_id,
            node,
            &config,
            start_time,
            started.elapsed().as_millis() as u64,
            None,
            false,
            false,
            "",
            "",
            false,
            false,
            failure,
        );
    }

    let mut command = Command::new(&config.command);
    command.args(&config.args);
    command.current_dir(&prepared.cwd);
    command.envs(&config.env);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let process_result = match command.spawn() {
        Ok(child) => wait_for_shell_process(opts, node, child, &config)?,
        Err(source) => {
            return shell_failed_outcome(
                ws,
                project,
                run_id,
                node,
                &config,
                start_time,
                started.elapsed().as_millis() as u64,
                None,
                false,
                false,
                "",
                "",
                false,
                false,
                ShellFailure {
                    code: "spawn_failed".to_string(),
                    message: source.to_string(),
                },
            );
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let exit_code = process_result.status.code();
    let success = exit_code
        .map(|code| config.expected_exit_codes.contains(&code))
        .unwrap_or(false)
        && !process_result.timed_out
        && !process_result.cancelled;

    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!(
            "[{}] exit_code={:?} success={} timed_out={} cancelled={}",
            Utc::now().to_rfc3339(),
            exit_code,
            success,
            process_result.timed_out,
            process_result.cancelled
        ),
    )?;

    let output = shell_output_json(
        success,
        &config,
        &prepared.cwd.display().to_string(),
        exit_code,
        duration_ms,
        process_result.timed_out,
        process_result.cancelled,
        &process_result.stdout,
        &process_result.stderr,
        process_result.stdout_truncated,
        process_result.stderr_truncated,
    );

    let artifact = write_shell_artifact(
        ws,
        project,
        run_id,
        &node.id,
        &output,
        &process_result.stdout,
        &process_result.stderr,
        None,
    )?;

    let log_tail = combined_tail(&process_result.stdout, &process_result.stderr);
    let end_time = Utc::now().to_rfc3339();

    if success {
        let next = resolve_next_nodes(edge_map, &node.id, node, &run.context, None)?;
        Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Succeeded,
            next_nodes: next,
            output: Some(output),
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Succeeded,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(duration_ms),
                iteration: None,
                exit_code,
                error: None,
                output_artifact: Some(artifact),
                log_tail: Some(log_tail),
                child_run_id: None,
                runtime: Some(SHELL_RUNTIME_NAME.to_string()),
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: None,
            end_result: None,
        })
    } else {
        let failure = if process_result.cancelled {
            ShellFailure {
                code: "cancelled".to_string(),
                message: "Run was cancelled during shell execution".to_string(),
            }
        } else if process_result.timed_out {
            ShellFailure {
                code: "timeout".to_string(),
                message: format!("Shell command exceeded {} ms", config.timeout_ms),
            }
        } else {
            ShellFailure {
                code: "exit_code".to_string(),
                message: format!("Shell command exited with code {:?}", exit_code),
            }
        };
        Ok(NodeOutcome {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            next_nodes: vec![],
            output: Some(output),
            node_state: TaskGraphRunNode {
                node_id: node.id.clone(),
                status: NodeRunStatus::Failed,
                started_at: Some(start_time),
                completed_at: Some(end_time),
                duration_ms: Some(duration_ms),
                iteration: None,
                exit_code,
                error: Some(NodeError {
                    code: failure.code,
                    message: failure.message,
                }),
                output_artifact: Some(artifact),
                log_tail: Some(log_tail),
                child_run_id: None,
                runtime: Some(SHELL_RUNTIME_NAME.to_string()),
                agent: None,
                model: None,
                agent_session_id: None,
                agent_session: None,
            },
            side_effects: vec![],
            child_run_id: None,
            end_result: None,
        })
    }
}

fn prepare_shell(
    workspace_root: &Path,
    config: &ShellConfig,
) -> Result<PreparedShell, ShellFailure> {
    let cwd = PathBuf::from(config.cwd.trim());
    if cwd.is_absolute() {
        return Err(ShellFailure {
            code: "invalid_cwd".to_string(),
            message: "Shell cwd must be relative to the workspace root".to_string(),
        });
    }

    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|source| ShellFailure {
            code: "invalid_workspace".to_string(),
            message: source.to_string(),
        })?;
    let joined = canonical_root.join(cwd);
    let canonical_cwd = joined.canonicalize().map_err(|source| ShellFailure {
        code: "invalid_cwd".to_string(),
        message: source.to_string(),
    })?;

    if !canonical_cwd.starts_with(&canonical_root) {
        return Err(ShellFailure {
            code: "invalid_cwd".to_string(),
            message: "Shell cwd escapes the workspace root".to_string(),
        });
    }

    Ok(PreparedShell { cwd: canonical_cwd })
}

fn permission_failure(config: &ShellConfig) -> Option<ShellFailure> {
    let command = command_name(&config.command);
    let args: Vec<String> = config.args.iter().map(|arg| arg.to_lowercase()).collect();

    if is_network_operation(&command, &args) && config.permission != ShellPermission::Network {
        return Some(ShellFailure {
            code: "permission_denied".to_string(),
            message: format!(
                "Shell permission {:?} does not allow network command '{}'",
                config.permission, config.command
            ),
        });
    }

    if is_git_write_operation(&command, &args)
        && !matches!(
            config.permission,
            ShellPermission::GitWrite | ShellPermission::Network
        )
    {
        return Some(ShellFailure {
            code: "permission_denied".to_string(),
            message: format!(
                "Shell permission {:?} does not allow git write command '{}'",
                config.permission, config.command
            ),
        });
    }

    if is_project_write_operation(&command, &args)
        && matches!(config.permission, ShellPermission::ReadOnly)
    {
        return Some(ShellFailure {
            code: "permission_denied".to_string(),
            message: format!(
                "Shell permission {:?} does not allow project write command '{}'",
                config.permission, config.command
            ),
        });
    }

    None
}

fn command_name(command: &str) -> String {
    Path::new(command)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .trim()
        .to_lowercase()
}

fn first_arg(args: &[String]) -> Option<&str> {
    args.first().map(String::as_str)
}

fn is_network_operation(command: &str, args: &[String]) -> bool {
    if matches!(
        command,
        "curl" | "wget" | "ssh" | "scp" | "sftp" | "rsync" | "ping" | "ftp" | "telnet" | "gh"
    ) {
        return true;
    }

    match (command, first_arg(args)) {
        ("git", Some("push" | "pull" | "fetch" | "clone" | "ls-remote")) => true,
        ("npm" | "pnpm" | "yarn", Some("install" | "add" | "update" | "audit" | "publish")) => true,
        ("pip" | "pip3", Some("install")) => true,
        ("cargo", Some("install" | "publish" | "search" | "owner" | "login")) => true,
        ("uv", Some("add" | "sync" | "pip")) => true,
        _ => false,
    }
}

fn is_git_write_operation(command: &str, args: &[String]) -> bool {
    if command != "git" {
        return false;
    }
    matches!(
        first_arg(args),
        Some(
            "add"
                | "am"
                | "apply"
                | "bisect"
                | "branch"
                | "checkout"
                | "cherry-pick"
                | "clean"
                | "commit"
                | "merge"
                | "mv"
                | "rebase"
                | "reset"
                | "restore"
                | "revert"
                | "rm"
                | "stash"
                | "switch"
                | "tag"
        )
    )
}

fn is_project_write_operation(command: &str, _args: &[String]) -> bool {
    if matches!(
        command,
        "rm" | "rmdir" | "del" | "erase" | "mv" | "move" | "cp" | "copy" | "mkdir" | "touch"
    ) {
        return true;
    }

    false
}

fn wait_for_shell_process(
    opts: &RunnerOptions,
    node: &TaskGraphNode,
    mut child: Child,
    config: &ShellConfig,
) -> Result<ShellProcessResult, TaskGraphError> {
    let log_context = ShellLogContext {
        workspace_root: opts.workspace_root.clone(),
        project: opts.project.clone(),
        run_id: opts.run_id.clone(),
        node_id: node.id.clone(),
        strip_ansi: config.capture.strip_ansi,
    };
    let stdout_capture = Arc::new(Mutex::new(ShellCapture::new(config.capture.max_bytes)));
    let stderr_capture = Arc::new(Mutex::new(ShellCapture::new(config.capture.max_bytes)));

    let stdout_reader = child.stdout.take().map(|stdout| {
        spawn_pipe_reader(
            stdout,
            "stdout",
            stdout_capture.clone(),
            log_context.clone(),
        )
    });
    let stderr_reader = child.stderr.take().map(|stderr| {
        spawn_pipe_reader(
            stderr,
            "stderr",
            stderr_capture.clone(),
            log_context.clone(),
        )
    });

    let deadline = Instant::now() + Duration::from_millis(config.timeout_ms);
    let mut timed_out = false;
    let mut cancelled = false;

    let status = loop {
        if run_is_cancelled(&opts.workspace_root, &opts.project, &opts.run_id) {
            cancelled = true;
            let _ = child.kill();
            break child.wait().map_err(|source| TaskGraphError::Io {
                path: opts.workspace_root.clone(),
                source,
            })?;
        }

        if let Some(status) = child.try_wait().map_err(|source| TaskGraphError::Io {
            path: opts.workspace_root.clone(),
            source,
        })? {
            break status;
        }

        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().map_err(|source| TaskGraphError::Io {
                path: opts.workspace_root.clone(),
                source,
            })?;
        }

        thread::sleep(Duration::from_millis(100));
    };

    join_reader(stdout_reader);
    join_reader(stderr_reader);

    let stdout = stdout_capture.lock().unwrap().text();
    let stderr = stderr_capture.lock().unwrap().text();
    let stdout_truncated = stdout_capture.lock().unwrap().truncated;
    let stderr_truncated = stderr_capture.lock().unwrap().truncated;

    Ok(ShellProcessResult {
        status,
        timed_out,
        cancelled,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
    label: &'static str,
    capture: Arc<Mutex<ShellCapture>>,
    context: ShellLogContext,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; LOG_CHUNK_BYTES];
        loop {
            let read = match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => break,
            };
            let chunk = &buffer[..read];
            let text = String::from_utf8_lossy(chunk);
            let text = if context.strip_ansi {
                strip_ansi_codes(&text)
            } else {
                text.to_string()
            };
            capture.lock().unwrap().push(text.as_bytes());
            if !text.trim().is_empty() {
                let _ = run_state::append_node_log(
                    &context.workspace_root,
                    &context.project,
                    &context.run_id,
                    &context.node_id,
                    &format!("[{label}] {text}"),
                );
            }
        }
    })
}

fn join_reader(handle: Option<thread::JoinHandle<()>>) {
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

#[allow(clippy::too_many_arguments)]
fn shell_output_json(
    ok: bool,
    config: &ShellConfig,
    cwd: &str,
    exit_code: Option<i32>,
    duration_ms: u64,
    timed_out: bool,
    cancelled: bool,
    stdout: &str,
    stderr: &str,
    stdout_truncated: bool,
    stderr_truncated: bool,
) -> serde_json::Value {
    serde_json::json!({
        "ok": ok,
        "command": &config.command,
        "args": &config.args,
        "cwd": cwd,
        "exit_code": exit_code,
        "duration_ms": duration_ms,
        "timed_out": timed_out,
        "cancelled": cancelled,
        "stdout_tail": tail_str(stdout, OUTPUT_TAIL_CHARS),
        "stderr_tail": tail_str(stderr, OUTPUT_TAIL_CHARS),
        "stdout_truncated": stdout_truncated,
        "stderr_truncated": stderr_truncated,
    })
}

fn write_shell_artifact(
    ws: &Path,
    project: &str,
    run_id: &str,
    node_id: &str,
    output: &serde_json::Value,
    stdout: &str,
    stderr: &str,
    error: Option<&ShellFailure>,
) -> Result<run_state::OutputArtifact, TaskGraphError> {
    let payload = serde_json::json!({
        "output": output,
        "stdout": stdout,
        "stderr": stderr,
        "error": error.map(|err| serde_json::json!({
            "code": &err.code,
            "message": &err.message,
        })),
    });
    let content = serde_json::to_string_pretty(&payload).expect("shell artifact serializes");
    run_state::write_artifact(
        ws,
        project,
        run_id,
        &format!("{}-shell-output", node_id),
        &content,
        ArtifactContentType::Json,
    )
}

#[allow(clippy::too_many_arguments)]
fn shell_failed_outcome(
    ws: &Path,
    project: &str,
    run_id: &str,
    node: &TaskGraphNode,
    config: &ShellConfig,
    start_time: String,
    duration_ms: u64,
    exit_code: Option<i32>,
    timed_out: bool,
    cancelled: bool,
    stdout: &str,
    stderr: &str,
    stdout_truncated: bool,
    stderr_truncated: bool,
    failure: ShellFailure,
) -> Result<NodeOutcome, TaskGraphError> {
    run_state::append_node_log(
        ws,
        project,
        run_id,
        &node.id,
        &format!("[{}] {}", failure.code, failure.message),
    )?;
    let output = shell_output_json(
        false,
        config,
        &config.cwd,
        exit_code,
        duration_ms,
        timed_out,
        cancelled,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    );
    let artifact = write_shell_artifact(
        ws,
        project,
        run_id,
        &node.id,
        &output,
        stdout,
        stderr,
        Some(&failure),
    )?;
    let log_tail = if stdout.is_empty() && stderr.is_empty() {
        failure.message.clone()
    } else {
        combined_tail(stdout, stderr)
    };
    Ok(NodeOutcome {
        node_id: node.id.clone(),
        status: NodeRunStatus::Failed,
        next_nodes: vec![],
        output: Some(output),
        node_state: TaskGraphRunNode {
            node_id: node.id.clone(),
            status: NodeRunStatus::Failed,
            started_at: Some(start_time),
            completed_at: Some(Utc::now().to_rfc3339()),
            duration_ms: Some(duration_ms),
            iteration: None,
            exit_code,
            error: Some(NodeError {
                code: failure.code,
                message: failure.message,
            }),
            output_artifact: Some(artifact),
            log_tail: Some(log_tail),
            child_run_id: None,
            runtime: Some(SHELL_RUNTIME_NAME.to_string()),
            agent: None,
            model: None,
            agent_session_id: None,
            agent_session: None,
        },
        side_effects: vec![],
        child_run_id: None,
        end_result: None,
    })
}

fn combined_tail(stdout: &str, stderr: &str) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => tail_str(stdout, OUTPUT_TAIL_CHARS),
        (true, false) => tail_str(stderr, OUTPUT_TAIL_CHARS),
        (false, false) => tail_str(&format!("{stdout}\n{stderr}"), OUTPUT_TAIL_CHARS),
    }
}
