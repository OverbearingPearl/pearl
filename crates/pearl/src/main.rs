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
use pearl_core::{Position, Queue, Request};
use pearl_provider::Houses;
use pearl_tui::App;

#[tokio::main]
async fn main() -> Result<()> {
    let plan = match pearl::plan(std::env::args()) {
        Ok(plan) => plan,
        Err(e) => e.exit(),
    };
    match plan {
        pearl::Plan::Run {
            task,
            model,
            session,
            resumed,
        } => run(task, model, session, resumed).await,
        pearl::Plan::Surface { session, resumed } => surface(session, resumed),
        pearl::Plan::Usage { session } => usage(&session),
    }
}

/// Open the live surface: the default entry point.
///
/// One agent serves the queue on a background task; the surface keeps the
/// terminal and the same queue. The aside answerer is a second handle on the
/// session file -- a reader, never a writer -- because a side question must not
/// wait behind the task in flight.
/// Print what the session log says about key use, then leave.
///
/// The report is deliberately dull: presses per key, the keys the log never
/// saw, and the groups they belong to. It never suggests a deletion -- a key
/// nobody pressed may be a key nobody could find, and only a human reading
/// the screen can tell those apart.
fn usage(path: &std::path::Path) -> Result<()> {
    // Read, never open: a report must not claim the directory, or a PEARL
    // running in another window would be refused for a look at its log.
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            println!("no usage recorded: {} does not exist yet", path.display());
            return Ok(());
        }
        Err(why) => return Err(why).context("reading the session log"),
    };
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut sessions = 0u64;
    let mut unread = 0u64;
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry.get("role").and_then(|r| r.as_str()) != Some("usage") {
            continue;
        }
        sessions += 1;
        let pressed = entry
            .get("content")
            .and_then(|c| c.as_str())
            .and_then(|c| serde_json::from_str::<std::collections::BTreeMap<String, u64>>(c).ok());
        match pressed {
            Some(pressed) => {
                for (key, times) in pressed {
                    *counts.entry(key).or_insert(0) += times;
                }
            }
            // A line this reader cannot parse is said out loud: a key reported
            // as never pressed may simply be a line nobody read.
            None => unread += 1,
        }
    }
    if counts.is_empty() {
        println!("no usage recorded in this session log");
        return Ok(());
    }
    let all: u64 = counts.values().sum();
    println!("keys, over {sessions} session(s)");
    for (key, times) in &counts {
        let share = *times as f64 * 100.0 / all as f64;
        println!("  {key:<8} {times:>6}  {share:>5.1}%");
    }
    if unread > 0 {
        println!("{unread} usage line(s) could not be read");
    }
    Ok(())
}
fn surface(path: PathBuf, resumed: bool) -> Result<()> {
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
        .with_place(root.clone(), pearl::RUNNER_POLICY);
    if let Some(c) = &cred {
        app = app.with_provider(c.provider.clone());
    }
    // The list says which rows can be paid for. Asking is the provider's
    // business, so the surface is handed the question rather than the answer.
    app = app.with_key_probe(|endpoint| {
        Houses::new(endpoint.to_string(), String::new)
            .credential()
            .is_some()
    });
    // Coming back to a directory carries on with it; only `--fresh` begins
    // again. A directory with nothing in it is a beginning either way.
    let (store, resumed) = match pearl::session(&path, resumed) {
        Ok(store) => (store, resumed),
        Err(pearl_session::StoreError::Nothing { .. }) if resumed => (
            pearl::session(&path, false).context("opening a fresh session")?,
            false,
        ),
        Err(e) => return Err(e).context("opening the session"),
    };
    // The ground, said once: what the model may touch is a promise the header
    // prints and the OS enforces, and a model that has to probe for it pays
    // for the answer with turns.
    match pearl::ground(&store, &root, pearl::RUNNER_POLICY) {
        Ok(_) => {}
        Err(e) => app.notice = Some(format!("the ground could not be written: {e}")),
    }
    // The list is a projection of the same log: carrying on puts the last
    // run's queue back, with everything unfinished held so nothing is spent by
    // coming back.
    if resumed {
        match pearl::restore_queue(&store, &queue) {
            Ok(Some(count)) if count > 0 => {
                app.notice = Some(format!(
                    "restored {count} task(s) from the last run; none of them run until you say so",
                ));
            }
            Ok(_) => {}
            Err(e) => app.notice = Some(format!("the last queue could not be read: {e}")),
        }
    }
    pearl::watch_queue(queue.clone(), store.clone());
    // The aside reads the log; it never appends, so a clone of the store is a
    // second reader rather than a second writer.
    let mut aside = Agent::new(
        queue.clone(),
        houses(&settings),
        store.clone(),
        settings.model(),
    )
    .with_settings(settings.clone());
    // One ledger of takeable writes: the runner records, the keyboard undoes.
    let ops = pearl_tools::Ops::new();
    let halter = pearl::spawn_runner(
        queue.clone(),
        houses(&settings),
        store.clone(),
        settings.clone(),
        ops.clone(),
        gauge.clone(),
    )
    .0;
    let undo_ops = ops.clone();
    let undo = move || {
        undo_ops
            .revert_last()
            .map(|o| o.unwrap_or_default())
            .map_err(|e| e.to_string())
    };
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
    let file_ops = ops.clone();
    let files = move |task: &str| -> Result<String, String> {
        Ok(file_ops
            .report(task)
            .unwrap_or_else(|| String::from("nothing yet: this task has not touched a file")))
    };
    let mut usage_store = store.clone();
    let done = move |app: &pearl_tui::App| {
        let _ = usage_store.append(pearl_session::Role::Usage, app.usage_json());
    };
    let ran = pearl_tui::run_with_usage(&mut app, answer, undo, save, files, done)
        .context("running the queue surface");
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
    let var = format!(
        "{}_API_KEY",
        pearl_provider::auth::provider_name(&host).to_uppercase()
    );
    let from_env = std::env::var(&var).ok();
    let path = pearl_provider::auth::default_path()?;
    pearl_provider::auth::resolve(&host, from_env, &path)
}

async fn run(task: String, model: String, session: PathBuf, resumed: bool) -> Result<()> {
    let house = pearl_provider::auth::provider_name(&pearl_provider::auth::host_of(&base_url()));
    let settings = pearl::knobs(&model, &house);
    let provider = houses(&settings);
    let store = match pearl::session(&session, resumed) {
        Ok(store) => store,
        Err(pearl_session::StoreError::Nothing { .. }) if resumed => {
            pearl::session(&session, false).context("opening a fresh session")?
        }
        Err(e) => return Err(e).context("opening the session"),
    };
    let root = std::env::current_dir()?;
    pearl::ground(&store, &root, Policy::WorkspaceWrite).ok();
    let queue = Queue::new();
    // The same door the surface uses, so `pearl run ""` says what is missing
    // rather than running a request with no question in it.
    queue.place(Request::task(task), Position::Last)?;
    let mut agent = Agent::new(queue, provider, store, model)
        .with_settings(settings.clone())
        .with_tools(root, Policy::WorkspaceWrite);
    agent.run().await.context("running the queue")?;
    if let Some(last) = agent.session().newest()? {
        println!("{}", last.content);
    }
    Ok(())
}
