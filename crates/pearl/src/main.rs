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
    #[command(subcommand)]
    command: Command,
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
    /// Open the queue as a live flow surface.
    Queue,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { task, model, session } => run(task, model, session).await,
        Command::Queue => {
            println!("The live queue surface is the next milestone; see docs/roadmap.md.");
            Ok(())
        }
    }
}

async fn run(task: String, model: String, session: PathBuf) -> Result<()> {
    let key = std::env::var("DEEPSEEK_API_KEY")
        .context("DEEPSEEK_API_KEY is not set; the key is resolved at use, not stored")?;
    let provider = OpenAiProvider::deepseek(key);
    let store = SessionStore::open(&session).context("opening the session")?;
    let mut queue = Queue::new();
    queue.append(task);
    let mut agent = Agent::new(queue, provider, store, model);
    agent.run().await.context("running the queue")?;
    if let Some(last) = agent.store.read()?.last() {
        println!("{}", last.content);
    }
    Ok(())
}
