//! pearl --- the command line: shape a queue, run it, watch it flow.
//!
//! The model is chosen per request and the key resolved at use. DeepSeek is the
//! first endpoint; the queue is the subject you shape while the agent works.
//!
//! The surface and the runner are the same session: both hold the same queue,
//! the surface edits it and the runner drains it. Where a request goes and who
//! pays for it are read per request, so a model chosen at the keyboard reaches
//! its own endpoint -- and a local server that wants no key is not a reason to
//! refuse to run.

use std::path::PathBuf;

use anyhow::{Context, Result};
use pearl_agent::{Agent, Policy};
use pearl_core::Queue;
use pearl_provider::Houses;
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
    let house = pearl_provider::auth::provider_name(&pearl_provider::auth::host_of(&base_url()));
    let settings = pearl::knobs("deepseek-chat", &house);
    let cred = credential();
    let gauge = pearl_core::Gauge::new();
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut app = App::new(queue.clone())
        .with_settings(settings.clone())
        .with_launch(base_url())
        .with_gauge(gauge.clone())
        .with_place(root, pearl::RUNNER_POLICY);
    if let Some(c) = &cred {
        app = app.with_provider(c.provider.clone());
    }
    // The list says which rows can be paid for. Asking is the provider's
    // business, so the surface is handed the question rather than the answer.
    app = app.with_key_probe(|endpoint| Houses::new(endpoint.to_string(), String::new).credential().is_some());
    let path = PathBuf::from("pearl-session.jsonl");
    let store = SessionStore::open(&path).context("opening the session")?;
    // The aside reads the log; it never appends, so a clone of the store is a
    // second reader rather than a second writer.
    let mut aside = Agent::new(queue.clone(), houses(&settings), store.clone(), settings.model())
        .with_settings(settings.clone());
    // One ledger of takeable writes: the runner records, the keyboard undoes.
    let ops = pearl_tools::Ops::new();
    let halter = pearl::spawn_runner(
        queue.clone(), houses(&settings), store.clone(), settings.clone(), ops.clone(), gauge.clone(),
    )
    .0;
    let undo_ops = ops.clone();
    let undo = move || undo_ops.revert_last().map(|o| o.unwrap_or_default()).map_err(|e| e.to_string());
    let answer = move |question: &str| -> Result<String, String> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(aside.btw(question))
        })
        .map_err(|e| e.to_string())
    };
    let saved = settings.clone();
    let save = move || {
        saved
            .save(pearl::config_path())
            .map(|()| pearl::config_path().display().to_string())
            .map_err(|e| e.to_string())
    };
    let ran = pearl_tui::run_with_save(app, answer, undo, save).context("running the queue surface");
    let _ = halter.send(true);
    ran
}

/// The router the loop asks through. DeepSeek unless `PEARL_BASE_URL` names
/// another OpenAI-compatible one, and whichever endpoint the chosen candidate
/// names once the human chooses one: the endpoint is part of the choice, so it
/// is read per request rather than fixed at launch.
fn houses(settings: &pearl_core::Settings) -> Houses<impl Fn() -> String + use<>> {
    let live = settings.clone();
    Houses::new(base_url(), move || live.endpoint())
}

/// The endpoint's base URL, whether or not a key was found for it.
fn base_url() -> String {
    match std::env::var("PEARL_BASE_URL") {
        Ok(url) if !url.is_empty() => url,
        // DeepSeek takes its requests at `/chat/completions` on either base;
        // the bare one is what they document first, and what the picker's
        // published list names, so the launch endpoint is not offered twice.
        _ => String::from("https://api.deepseek.com"),
    }
}

/// Resolve the key at use: the environment for this run, else the authinfo line
/// that names this machine. The env var is the house's own -- DEEPSEEK_API_KEY,
/// OPENAI_API_KEY -- so one file can serve several houses without a rename.
fn credential() -> Option<pearl_provider::auth::Credential> {
    let host = pearl_provider::auth::host_of(&base_url());
    let var = format!("{}_API_KEY", pearl_provider::auth::provider_name(&host).to_uppercase());
    let from_env = std::env::var(&var).ok();
    let path = pearl_provider::auth::default_path()?;
    pearl_provider::auth::resolve(&host, from_env, &path)
}

async fn run(task: String, model: String, session: PathBuf) -> Result<()> {
    let house = pearl_provider::auth::provider_name(&pearl_provider::auth::host_of(&base_url()));
    let settings = pearl::knobs(&model, &house);
    let provider = houses(&settings);
    let store = SessionStore::open(&session).context("opening the session")?;
    let queue = Queue::new();
    queue.append(task);
    let mut agent = Agent::new(queue, provider, store, model)
        .with_settings(settings.clone())
        .with_tools(std::env::current_dir()?, Policy::WorkspaceWrite);
    agent.run().await.context("running the queue")?;
    if let Some(last) = agent.session().read()?.last() {
        println!("{}", last.content);
    }
    Ok(())
}
