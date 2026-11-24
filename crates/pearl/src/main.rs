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
    // One set of knobs for the surface, the runner and the aside: a change at
    // the keyboard is in force for the next request, not the next process.
    let settings = pearl_core::Settings::new("deepseek-chat");
    let config = config_path();
    if let Err(e) = settings.load(&config) {
        eprintln!("config {}: {e}", config.display());
    }
    let cred = credential();
    if let Some(c) = &cred {
        settings.set_models(pearl_provider::auth::models_for(&c.provider));
    }
    let key = cred.as_ref().map(|c| c.key.clone());
    let gauge = pearl_core::Gauge::new();
    let mut app = App::new(queue.clone()).with_settings(settings.clone()).with_gauge(gauge.clone());
    if let Some(c) = &cred {
        app = app.with_provider(c.provider.clone());
    }
    let path = PathBuf::from("pearl-session.jsonl");
    let store = SessionStore::open(&path).context("opening the session")?;
    // The aside reads the log; it never appends, so a clone of the store is a
    // second reader rather than a second writer.
    let mut aside = key.as_ref().map(|key| {
        Agent::new(queue.clone(), provider(key), store.clone(), settings.model())
            .with_settings(settings.clone())
    });
    // One ledger of takeable writes: the runner records, the keyboard undoes.
    let ops = pearl_tools::Ops::new();
    let halter = key.map(|key| {
        pearl::spawn_runner(queue.clone(), provider(&key), store.clone(), settings.clone(), ops.clone(), gauge.clone()).0
    });
    let undo_ops = ops.clone();
    let undo = move || undo_ops.revert_last().map(|o| o.unwrap_or_default()).map_err(|e| e.to_string());
    let answer = move |question: &str| -> Result<String, String> {
        let Some(a) = aside.as_mut() else {
            return Err(String::from("no key: set the house's API key env var or an ~/.authinfo line"));
        };
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(a.btw(question))
        })
        .map_err(|e| e.to_string())
    };
    let saved = settings.clone();
    let save = move || {
        saved
            .save(&config)
            .map(|()| config.display().to_string())
            .map_err(|e| e.to_string())
    };
    let ran = pearl_tui::run_with_save(app, answer, undo, save).context("running the queue surface");
    if let Some(halt) = halter {
        let _ = halt.send(true);
    }
    ran
}

/// The endpoint the key belongs to. DeepSeek unless `PEARL_BASE_URL` names
/// another OpenAI-compatible one, which is how a local model is asked the same
/// questions.
fn provider(key: &str) -> OpenAiProvider {
    match std::env::var("PEARL_BASE_URL") {
        Ok(url) if !url.is_empty() => OpenAiProvider::new(url, key),
        _ => OpenAiProvider::deepseek(key),
    }
}

/// The endpoint's base URL, whether or not a key was found for it.
fn base_url() -> String {
    match std::env::var("PEARL_BASE_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => String::from("https://api.deepseek.com/v1"),
    }
}

/// Resolve the key at use: the environment for this run, else the authinfo line
/// that names this machine. The env var is the house's own -- DEEPSEEK_API_KEY,
/// OPENAI_API_KEY -- so one file can serve several houses without a rename.
fn credential() -> Option<pearl_provider::auth::Credential> {
    let host = pearl_provider::auth::host_of(&base_url());
    let var = format!("{}_API_KEY", pearl_provider::auth::provider_name(&host).to_uppercase());
    let from_env = std::env::var(&var).ok().or_else(|| std::env::var("DEEPSEEK_API_KEY").ok());
    let path = pearl_provider::auth::default_path()?;
    pearl_provider::auth::resolve(&host, from_env, &path)
}

async fn run(task: String, model: String, session: PathBuf) -> Result<()> {
    let cred = credential()
        .context("no key: set the house's API key env var or an ~/.authinfo line")?;
    let provider = provider(&cred.key);
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

/// Where the kept knobs live. `PEARL_CONFIG` names another, which is how a
/// project keeps its own defaults beside the work.
fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("PEARL_CONFIG") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    PathBuf::from(home).join(".pearl").join("config")
}
