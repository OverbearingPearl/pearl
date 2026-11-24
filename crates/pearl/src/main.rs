//! pearl --- the command line: shape a queue, run it, watch it flow.
//!
//! The model is chosen per request and the key resolved at use. DeepSeek is the
//! first endpoint; the queue is the subject you shape while the agent works.
//!
//! The surface and the runner are the same session: both hold the same queue,
//! the surface edits it and the runner drains it. Without the key there is no
//! runner -- the surface still edits and asides say why they cannot answer --
//! and with it, a task placed by the surface starts without anything else.

use std::path::PathBuf;

use anyhow::{Context, Result};
use pearl_agent::{Agent, Policy};
use pearl_core::Queue;
use pearl_provider::OpenAiProvider;
use pearl_session::SessionStore;
use pearl_tui::App;

#[tokio::main]
async fn main() -> Result<()> {
    let plan = match pearl::plan(std::env::args()) {
        Ok(plan) => plan,
        Err(e) => e.exit(),
    };
    match plan {
        pearl::Plan::Run { task, model, session } => run(task, model, session).await,
        pearl::Plan::Surface => surface(),
    }
}

/// Open the live surface: the default entry point.
///
/// One agent serves the queue on a background task; the surface keeps the
/// terminal and the same queue. The aside answerer is a second handle on the
/// session file -- a reader, never a writer -- because a side question must not
/// wait behind the task in flight.
fn surface() -> Result<()> {
    let queue = Queue::new();
    let app = App::new(queue.clone());
    let path = PathBuf::from("pearl-session.jsonl");
    let store = SessionStore::open(&path).context("opening the session")?;
    let key = std::env::var("DEEPSEEK_API_KEY").ok();

    // The aside reads the log; it never appends, so a clone of the store is a
    // second reader rather than a second writer.
    let mut aside = key.as_ref().map(|key| {
        Agent::new(queue.clone(), OpenAiProvider::deepseek(key.clone()), store.clone(), "deepseek-chat")
    });
    let halter = key.map(|key| {
        let provider = OpenAiProvider::deepseek(key);
        pearl::spawn_runner(queue.clone(), provider, store.clone(), "deepseek-chat").0
    });
    let answer = move |question: &str| -> Result<String, String> {
        let Some(a) = aside.as_mut() else {
            return Err(String::from("DEEPSEEK_API_KEY is not set; the key is resolved at use"));
        };
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(a.btw(question))
        })
        .map_err(|e| e.to_string())
    };
    let ran = pearl_tui::run_with(app, answer).context("running the queue surface");
    if let Some(halt) = halter {
        let _ = halt.send(true);
    }
    ran
}

async fn run(task: String, model: String, session: PathBuf) -> Result<()> {
    let key = std::env::var("DEEPSEEK_API_KEY")
        .context("DEEPSEEK_API_KEY is not set; the key is resolved at use, not stored")?;
    let provider = OpenAiProvider::deepseek(key);
    let store = SessionStore::open(&session).context("opening the session")?;
    let queue = Queue::new();
    queue.append(task);
    let mut agent = Agent::new(queue, provider, store, model)
        .with_tools(std::env::current_dir()?, Policy::WorkspaceWrite);
    agent.run().await.context("running the queue")?;
    if let Some(last) = agent.session().read()?.last() {
        println!("{}", last.content);
    }
    Ok(())
}
