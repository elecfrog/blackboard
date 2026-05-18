use anyhow::{bail, Context};
use bb_core::Workspace;
use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Parser)]
#[command(name = "bb", about = "Blackboard daemon-client CLI")]
struct Cli {
    /// Blackboard root directory (the folder containing `projects/`).
    /// Defaults to daemon-side workspace discovery.
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize a writable Blackboard data root from a bundled seed.
    Init {
        /// Seed workspace root. Accepts a `.bb`/`.bb_template` data root or a repo root containing one.
        #[arg(long)]
        seed: PathBuf,
    },
    /// Compatibility wrapper for `bb-daemon stdio`.
    Stdio,
    /// Compatibility wrapper for `bb-daemon serve`.
    Http {
        /// Address to bind, for example 127.0.0.1:3001.
        #[arg(long, default_value = "127.0.0.1:3001")]
        addr: SocketAddr,
        /// Directory containing built frontend assets.
        #[arg(long)]
        static_dir: Option<PathBuf>,
    },
    /// Compatibility wrapper for `bb-daemon run`.
    Daemon {
        /// Task Graph to execute, in "<scope>/<id>" format.
        #[arg(long, default_value = "system/inbox-batch-cleanup")]
        graph: String,
        /// Agent identity for MCP tool environment.
        #[arg(long, default_value = "bb-pm")]
        agent: String,
        /// Model to use.
        #[arg(long)]
        model: Option<String>,
        /// Override the OpenCode command/path.
        #[arg(long)]
        opencode: Option<String>,
        /// Override the Codex command/path.
        #[arg(long)]
        codex: Option<String>,
        /// Override the CodeBuddy command/path.
        #[arg(long)]
        codebuddy: Option<String>,
        /// Override the Pi command/path.
        #[arg(long)]
        pi: Option<String>,
        /// Poll interval in seconds.
        #[arg(long, default_value_t = 15)]
        interval_seconds: u64,
        /// Maximum dispatches per scan pass. Use 0 for no limit.
        #[arg(long, default_value_t = 1)]
        max_dispatch_per_scan: usize,
        /// Override per-node TaskGraph timeout in seconds.
        #[arg(long)]
        node_timeout_seconds: Option<u64>,
        /// Override the entire TaskGraph run timeout in seconds.
        #[arg(long)]
        run_timeout_seconds: Option<u64>,
        /// Restrict scanning to one project.
        #[arg(long)]
        project: Option<String>,
        /// Run one pass and exit.
        #[arg(long)]
        once: bool,
        /// Dry-run mode: create the run but skip actual node execution.
        #[arg(long)]
        dry_run: bool,
        /// Enable watch mode: poll inbox for new notes and trigger graph runs.
        #[arg(long)]
        watch: bool,
        /// Enable stateless schedule dispatcher mode.
        #[arg(long)]
        schedules: bool,
        /// Retry entries whose current content hash already failed once.
        #[arg(long)]
        retry_failed: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Commands::Init { seed } = &cli.command {
        let target = match &cli.root {
            Some(root) => root.clone(),
            None => std::env::current_dir().context("failed to resolve current directory")?,
        };
        let workspace = Workspace::init_from_seed(target, seed)
            .context("failed to initialize Blackboard workspace from seed")?;
        eprintln!(
            "bb workspace initialized at {} from {}",
            bb_core::path_to_string(workspace.root()),
            bb_core::path_to_string(seed)
        );
        return Ok(());
    }

    let args = daemon_args(&cli);
    let program = resolve_daemon_program();
    eprintln!(
        "bb: forwarding to {} {}",
        program.display(),
        join_args_for_log(&args)
    );
    let status = Command::new(&program)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| {
            format!(
                "failed to launch bb-daemon compatibility target at {}",
                program.display()
            )
        })?;
    if !status.success() {
        bail!("bb-daemon exited with status {status}");
    }
    Ok(())
}

fn daemon_args(cli: &Cli) -> Vec<OsString> {
    let mut args = Vec::new();
    if let Some(root) = cli.root.as_ref() {
        args.push("--root".into());
        args.push(root.as_os_str().to_os_string());
    }

    match &cli.command {
        Commands::Init { .. } => unreachable!("init does not delegate to bb-daemon"),
        Commands::Stdio => args.push("stdio".into()),
        Commands::Http { addr, static_dir } => {
            args.push("serve".into());
            args.push("--addr".into());
            args.push(addr.to_string().into());
            if let Some(static_dir) = static_dir {
                args.push("--static-dir".into());
                args.push(static_dir.as_os_str().to_os_string());
            }
        }
        Commands::Daemon {
            graph,
            agent,
            model,
            opencode,
            codex,
            codebuddy,
            pi,
            interval_seconds,
            max_dispatch_per_scan,
            node_timeout_seconds,
            run_timeout_seconds,
            project,
            once,
            dry_run,
            watch,
            schedules,
            retry_failed,
        } => {
            args.push("run".into());
            push_pair(&mut args, "--graph", graph);
            push_pair(&mut args, "--agent", agent);
            push_optional(&mut args, "--model", model.as_ref());
            push_optional(&mut args, "--opencode", opencode.as_ref());
            push_optional(&mut args, "--codex", codex.as_ref());
            push_optional(&mut args, "--codebuddy", codebuddy.as_ref());
            push_optional(&mut args, "--pi", pi.as_ref());
            push_pair(
                &mut args,
                "--interval-seconds",
                &interval_seconds.to_string(),
            );
            push_pair(
                &mut args,
                "--max-dispatch-per-scan",
                &max_dispatch_per_scan.to_string(),
            );
            if let Some(value) = node_timeout_seconds {
                push_pair(&mut args, "--node-timeout-seconds", &value.to_string());
            }
            if let Some(value) = run_timeout_seconds {
                push_pair(&mut args, "--run-timeout-seconds", &value.to_string());
            }
            push_optional(&mut args, "--project", project.as_ref());
            push_flag(&mut args, "--once", *once);
            push_flag(&mut args, "--dry-run", *dry_run);
            push_flag(&mut args, "--watch", *watch);
            push_flag(&mut args, "--schedules", *schedules);
            push_flag(&mut args, "--retry-failed", *retry_failed);
        }
    }

    args
}

fn push_pair(args: &mut Vec<OsString>, key: &str, value: &str) {
    args.push(key.into());
    args.push(value.into());
}

fn push_optional(args: &mut Vec<OsString>, key: &str, value: Option<&String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        push_pair(args, key, value);
    }
}

fn push_flag(args: &mut Vec<OsString>, flag: &str, enabled: bool) {
    if enabled {
        args.push(flag.into());
    }
}

fn resolve_daemon_program() -> PathBuf {
    let exe_name = if cfg!(windows) {
        "bb-daemon.exe"
    } else {
        "bb-daemon"
    };
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join(exe_name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from(exe_name)
}

fn join_args_for_log(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}
