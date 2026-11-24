//! pearl --- what the command line asks for, as a value.
//!
//! The arguments decide one thing: open the surface, or run one task and exit.
//! That decision is made here rather than inside `main` so a story can read it
//! without starting a terminal, and so the binary stays a switch over a plan.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pearl_agent::{Agent, AgentError, Policy};
use pearl_core::Queue;
use pearl_provider::Provider;
use pearl_session::SessionStore;

/// What the arguments ask pearl to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Open the live surface over the queue. This is what no subcommand means:
    /// the surface is the program, and `run` is the one task you fire and
    /// forget.
    Surface,
    /// Run one task end to end, print what it said, and exit.
    Run {
        /// The task to carry out.
        task: String,
        /// The model to use for this request.
        model: String,
        /// Where the session is written.
        session: PathBuf,
    },
}

#[derive(Parser)]
#[command(name = "pearl", version, about = "A queue-centric agent development environment.")]
struct Cli {
    /// What to do. With no subcommand, `pearl` opens the surface.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run one task end to end against the model.
    Run {
        /// The task to carry out.
        task: String,
        /// The model to use for this request.
        #[arg(long, default_value = "deepseek-chat")]
        model: String,
        /// Where the session is written.
        #[arg(long, default_value = "pearl-session.jsonl")]
        session: PathBuf,
    },
    /// Open the queue as a live flow surface (the default).
    Queue,
}

/// Read the arguments into the plan they describe. A usage error, `--help` and
/// `--version` all come back as a `clap::Error`, which the binary prints or
/// exits on rather than deciding here.
pub fn plan(args: impl IntoIterator<Item = String>) -> Result<Plan, clap::Error> {
    let cli = Cli::try_parse_from(args)?;
    Ok(match cli.command {
        Some(Command::Run { task, model, session }) => Plan::Run { task, model, session },
        Some(Command::Queue) | None => Plan::Surface,
    })
}

/// Start the runner over a queue the caller already holds, and return the halt
/// switch and the task it runs on.
///
/// The caller passes the queue it keeps, not a copy: the surface edits the
/// tree the runner drains, and the failure mode this signature makes
/// impossible is two queues that never see each other's work.
/// The confinement every runner works under. One declaration, so the surface
/// names the same ground the runner is held to.
pub const RUNNER_POLICY: Policy = Policy::WorkspaceWrite;

pub fn spawn_runner<P: Provider + Send + Sync + 'static>(
    queue: Queue,
    provider: P,
    store: SessionStore,
    settings: pearl_core::Settings,
    ops: pearl_tools::Ops,
    gauge: pearl_core::Gauge,
) -> (tokio::sync::watch::Sender<bool>, tokio::task::JoinHandle<Result<usize, AgentError>>) {
    let model = settings.model();
    let mut runner = Agent::new(queue, provider, store, model)
        .with_settings(settings)
        .with_tools(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")), RUNNER_POLICY)
        .with_ops(ops)
        .with_gauge(gauge);
    let (halt, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(async move { runner.serve(rx).await });
    (halt, handle)
}
