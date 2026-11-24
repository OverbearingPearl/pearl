//! pearl --- the command line: shape a queue, run it, watch it flow.
//!
//! The model is chosen per request and the key resolved at use. DeepSeek is the
//! first endpoint; the queue is the subject you shape while the agent works.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pearl_agent::Agent;
use pearl_core::Queue;
use pearl_provider::OpenAiProvider;
use pearl_session::SessionStore;

#[derive(Parser)]
#[command(name = "pearl", version, about = "A queue-centric agent development environment.")]
struct Cli {
    /// What to do. With no subcommand, `pearl` opens the surface, because the
    /// surface is the program and `run` is the one task you fire and forget.
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Run { task, model, session }) => run(task, model, session).await,
        Some(Command::Queue) | None => surface(),
    }
}

/// Open the live surface: the default entry point.
fn surface() -> Result<()> {
    let app = pearl_tui::App::new(Queue::new());
    // The aside answerer: read the persisted session as context and ask the
    // model. No key: the surface still edits; asides fail loud.
    let store = SessionStore::open("pearl-session.jsonl").context("opening the session")?;
    let mut agent = std::env::var("DEEPSEEK_API_KEY").ok().map(|key| {
        Agent::new(Queue::new(), OpenAiProvider::deepseek(key), store, "deepseek-chat")
    });
    let answer = move |question: &str| -> Result<String, String> {
        let Some(a) = agent.as_mut() else {
            return Err(String::from("DEEPSEEK_API_KEY is not set; the key is resolved at use"));
        };
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(a.btw(question))
        })
        .map_err(|e| e.to_string())
    };
    pearl_tui::run_with(app, answer).context("running the queue surface")
}

async fn run(task: String, model: String, session: PathBuf) -> Result<()> {
    let key = std::env::var("DEEPSEEK_API_KEY")
        .context("DEEPSEEK_API_KEY is not set; the key is resolved at use, not stored")?;
    let provider = OpenAiProvider::deepseek(key);
    let store = SessionStore::open(&session).context("opening the session")?;
    let mut queue = Queue::new();
    queue.append(task);
    let mut agent = Agent::new(queue, provider, store, model)
        .with_tools(std::env::current_dir()?, pearl_agent::Policy::WorkspaceWrite);
    agent.run().await.context("running the queue")?;
    if let Some(last) = agent.session().read()?.last() {
        println!("{}", last.content);
    }
    Ok(())
}
