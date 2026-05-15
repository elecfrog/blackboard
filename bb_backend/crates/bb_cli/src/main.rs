#![recursion_limit = "512"]

mod codex_client;
mod daemon;
mod http;
mod mcp_handler;
mod mcp_tools;
mod stdio;

use anyhow::Context;
use bb_core::Workspace;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "bb", about = "Blackboard — local multi-agent coordination CLI")]
struct Cli {
    /// Blackboard root directory (the folder containing `projects/`).
    /// Defaults to discovery from the current working directory.
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
    /// Run MCP tools over rmcp stdio transport.
    Stdio,
    /// Run the HTTP API, including Web REST routes and remote MCP /mcp.
    Http {
        /// Address to bind, for example 127.0.0.1:3001.
        #[arg(long, default_value = "127.0.0.1:3001")]
        addr: SocketAddr,
        /// Directory containing built frontend assets. When set, non-API routes
        /// are served from this directory with an SPA index.html fallback.
        #[arg(long)]
        static_dir: Option<PathBuf>,
    },
    /// Run a Task Graph as a headless CLI runner.
    Daemon {
        /// Task Graph to execute, in "<scope>/<id>" format.
        /// Scope is "system" or "project". Examples:
        ///   --graph system/inbox-batch-cleanup
        ///   --graph project/my-custom-graph
        #[arg(long, default_value = "system/inbox-batch-cleanup")]
        graph: String,
        /// Agent identity for MCP tool environment (must exist in agents.toml, or "bb-pm" default).
        #[arg(long, default_value = "bb-pm")]
        agent: String,
        /// Model to use (provider/model format, e.g. "minimax-cn-coding-plan/MiniMax-M2.7-highspeed").
        #[arg(long)]
        model: Option<String>,
        /// OpenCode executable path.
        #[arg(long, default_value = "opencode")]
        opencode: String,
        /// Codex executable path.
        #[arg(
            long,
            default_value = "/Applications/Codex.app/Contents/Resources/codex"
        )]
        codex: String,
        /// CodeBuddy executable path.
        #[arg(long, default_value = "codebuddy")]
        codebuddy: String,
        /// Poll interval in seconds (used in watch mode).
        #[arg(long, default_value_t = 15)]
        interval_seconds: u64,
        /// Maximum dispatches per scan pass. Use 0 for no limit.
        #[arg(long, default_value_t = 1)]
        max_dispatch_per_scan: usize,
        /// Timeout for the entire Task Graph run in seconds.
        #[arg(long, default_value_t = 1800)]
        run_timeout_seconds: u64,
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
        /// Retry entries whose current content hash already failed once (watch mode).
        #[arg(long)]
        retry_failed: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let workspace = match cli.root {
        Some(root) => Workspace::open(root).context("failed to open Blackboard workspace")?,
        None => Workspace::discover().context("failed to discover Blackboard workspace")?,
    };

    // Emit the discovered project list on stderr so operators and agents can
    // sanity-check project routing without polluting the stdio JSON-RPC
    // channel.
    match workspace.list_projects() {
        Ok(projects) if !projects.is_empty() => {
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            eprintln!(
                "bb workspace {} projects: {}",
                bb_core::path_to_string(workspace.root()),
                names.join(", ")
            );
        }
        Ok(_) => {
            eprintln!(
                "bb workspace {} has no projects yet",
                bb_core::path_to_string(workspace.root())
            );
        }
        Err(err) => {
            eprintln!("bb failed to enumerate projects: {err}");
        }
    }

    match cli.command {
        Commands::Init { .. } => unreachable!("init exits before opening runtime workspace"),
        Commands::Stdio => stdio::run(workspace).await,
        Commands::Http { addr, static_dir } => {
            let mcp_host = if addr.ip().is_unspecified() {
                "127.0.0.1".to_string()
            } else {
                addr.ip().to_string()
            };
            let mcp_host = if mcp_host.contains(':') {
                format!("[{mcp_host}]")
            } else {
                mcp_host
            };
            let mcp_url = format!("http://{mcp_host}:{}/mcp", addr.port());
            http::serve(
                workspace,
                addr,
                http::HttpServeOptions {
                    static_dir,
                    mcp_url: Some(mcp_url),
                },
            )
            .await
        }
        Commands::Daemon {
            graph,
            agent,
            model,
            opencode,
            codex,
            codebuddy,
            interval_seconds,
            max_dispatch_per_scan,
            run_timeout_seconds,
            project,
            once,
            dry_run,
            watch,
            schedules,
            retry_failed,
        } => daemon::run(
            workspace,
            daemon::DaemonOptions {
                graph,
                agent,
                model,
                opencode,
                codex,
                codebuddy,
                interval_seconds,
                max_dispatch_per_scan,
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
