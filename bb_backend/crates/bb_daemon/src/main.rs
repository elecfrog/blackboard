#![recursion_limit = "512"]

use anyhow::Context;
use bb_core::Workspace;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "bb-daemon",
    about = "Blackboard local daemon and MCP/TaskGraph runtime"
)]
struct Cli {
    /// Blackboard root directory (the folder containing `projects/`).
    /// Defaults to discovery from the current working directory.
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run the daemon HTTP API, including remote MCP /mcp.
    Serve {
        /// Address to bind, for example 127.0.0.1:3001.
        #[arg(long, default_value = "127.0.0.1:3001")]
        addr: SocketAddr,
        /// Directory containing built frontend assets.
        #[arg(long)]
        static_dir: Option<PathBuf>,
    },
    /// Run MCP tools over rmcp stdio transport.
    Stdio,
    /// Run TaskGraph watch/schedule/headless execution loops.
    Run {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let workspace = match cli.root {
        Some(root) => Workspace::open(root).context("failed to open Blackboard workspace")?,
        None => Workspace::discover().context("failed to discover Blackboard workspace")?,
    };
    log_workspace(&workspace);

    match cli.command.unwrap_or_else(|| Commands::Serve {
        addr: "127.0.0.1:3001".parse().expect("valid default addr"),
        static_dir: None,
    }) {
        Commands::Serve { addr, static_dir } => {
            let mcp_url = mcp_url_for_addr(addr);
            bb_daemon::http::serve(
                workspace,
                addr,
                bb_daemon::http::HttpServeOptions {
                    static_dir,
                    mcp_url: Some(mcp_url),
                },
            )
            .await
        }
        Commands::Stdio => bb_daemon::stdio::run(workspace).await,
        Commands::Run {
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
        } => bb_daemon::daemon::run(
            workspace,
            bb_daemon::daemon::DaemonOptions {
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
            },
        ),
    }
}

fn mcp_url_for_addr(addr: SocketAddr) -> String {
    let host = if addr.ip().is_unspecified() {
        "127.0.0.1".to_string()
    } else {
        addr.ip().to_string()
    };
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    format!("http://{host}:{}/mcp", addr.port())
}

fn log_workspace(workspace: &Workspace) {
    match workspace.list_projects() {
        Ok(projects) if !projects.is_empty() => {
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            eprintln!(
                "bb daemon workspace {} projects: {}",
                bb_core::path_to_string(workspace.root()),
                names.join(", ")
            );
        }
        Ok(_) => eprintln!(
            "bb daemon workspace {} has no projects yet",
            bb_core::path_to_string(workspace.root())
        ),
        Err(err) => eprintln!("bb daemon failed to enumerate projects: {err}"),
    }
}
