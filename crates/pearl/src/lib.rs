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
    Surface {
        /// Where the conversation the surface carries is written and read.
        session: PathBuf,
        /// Whether the launch kept the conversation the last run left off
        /// instead of beginning one of its own.
        resumed: bool,
    },
    /// Print what the log says about key use, then leave. It opens nothing and
    /// changes nothing: a report is read, not acted on.
    Usage {
        /// The log to read.
        session: PathBuf,
    },
    /// Run one task end to end, print what it said, and exit.
    Run {
        /// The task to carry out.
        task: String,
        /// The model to use for this request.
        model: String,
        /// Where the session is written.
        session: PathBuf,
        /// Whether the launch kept the conversation the last run left off.
        resumed: bool,
    },
}

#[derive(Parser)]
#[command(
    name = "pearl",
    version,
    about = "A queue-centric agent development environment."
)]
struct Cli {
    /// What to do. With no subcommand, `pearl` opens the surface.
    #[command(subcommand)]
    command: Option<Command>,
    /// Where the conversation is written and read.
    // A session is every run in a directory: each launch opens this file, the
    // log is replayed on every request, and compaction only folds at the
    // threshold. So the file has to be nameable from the default entry point
    // -- the surface *is* the program -- or a launch that meant to start clean
    // goes on appending to the log every earlier run wrote. Global for that
    // reason: `queue` only means "the default", and asking for it to pass a
    // flag said the default was a different program.
    #[arg(long, default_value = "pearl-session.jsonl", global = true)]
    session: PathBuf,
    /// Print what this directory's session log says about key use, then leave.
    #[arg(long, global = true)]
    usage: bool,
    /// Start a new conversation, leaving the last run's list and words behind.
    // The default is to carry on: a person who leaves and comes back to a
    // directory expects their list, and a queue that empties itself with the
    // window is a working memory the surface threw away. `--fresh` is for
    // starting over on purpose -- and the one launch that writes the mark the
    // view starts after.
    #[arg(long, global = true)]
    fresh: bool,
    /// Carry on with the last run, and say so if there is nothing to carry on
    /// with. The default already does this; the flag makes it explicit.
    #[arg(long, global = true, hide = true)]
    resume: bool,
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
    },
    /// Open the queue as a live flow surface (the default).
    Queue {},
    /// Say which keys this session's log shows a human pressing, and leave.
    Usage { session: PathBuf },
}

/// Read the arguments into the plan they describe. A usage error, `--help` and
/// `--version` all come back as a `clap::Error`, which the binary prints or
/// exits on rather than deciding here.
pub fn plan(args: impl IntoIterator<Item = String>) -> Result<Plan, clap::Error> {
    let cli = Cli::try_parse_from(args)?;
    let resumed = !cli.fresh;
    if cli.usage {
        return Ok(Plan::Usage {
            session: cli.session,
        });
    }
    Ok(match cli.command {
        Some(Command::Run { task, model }) => Plan::Run {
            task,
            model,
            session: cli.session,
            resumed,
        },
        Some(Command::Usage { session }) => Plan::Usage { session },
        Some(Command::Queue {}) => Plan::Surface {
            session: cli.session,
            resumed,
        },
        None => Plan::Surface {
            session: cli.session,
            resumed,
        },
    })
}

/// Open the session this launch records.
///
/// A launch begins a conversation: the mark it appends is where the view
/// starts, so the first request carries what this run said and not what every
/// run before it said. `resumed` is the human saying "keep the one the last run
/// left off", and it is the only launch that writes no mark.
pub fn session(
    path: &std::path::Path,
    resumed: bool,
) -> Result<SessionStore, pearl_session::StoreError> {
    // An empty file would resume into an empty conversation and say nothing:
    // the human asked for the last run and would get a fresh session wearing
    // its name. A missing launch is a wrong path, and a wrong path has to be
    // read as one -- before `open`, which would create the file it refused.
    // Read the size before `open`, which creates the file it is about to
    // refuse: a refusal that leaves a file behind is a refusal the human has
    // to clean up for nothing.
    if resumed
        && std::fs::metadata(path)
            .map(|m| m.len() == 0)
            .unwrap_or(true)
    {
        return Err(pearl_session::StoreError::Nothing {
            path: path.to_path_buf(),
        });
    }
    let mut store = SessionStore::open(path)?;
    if !resumed {
        store.append_launch()?;
    }
    Ok(store)
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
) -> (
    tokio::sync::watch::Sender<bool>,
    tokio::task::JoinHandle<Result<usize, AgentError>>,
) {
    let model = settings.model();
    let mut runner = Agent::new(queue, provider, store, model)
        .with_settings(settings)
        .with_tools(
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            RUNNER_POLICY,
        )
        .with_ops(ops)
        .with_gauge(gauge);
    let (halt, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(async move { runner.serve(rx).await });
    (halt, handle)
}

/// Where the kept knobs live. `PEARL_CONFIG` names another, which is how a
/// project keeps its own defaults beside the work.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("PEARL_CONFIG") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    PathBuf::from(home).join(".pearl").join("config")
}

/// The knobs a process starts with: the house's own models seeded, then the
/// kept file read over them. Both the surface and a one-shot run build them
/// here, because a run that ignored the file would answer a different question
/// from the one the human configured.
pub fn knobs(model: &str, house: &str) -> pearl_core::Settings {
    knobs_at(model, house, &config_path())
}

/// The same, from a named file. The path is a parameter so a story can state
/// one without setting a process-wide variable other stories are reading.
pub fn knobs_at(model: &str, house: &str, path: &std::path::Path) -> pearl_core::Settings {
    let settings = pearl_core::Settings::new(model);
    settings.set_launch_house(house);
    settings.set_models(pearl_provider::auth::models_for(house));
    if let Err(e) = settings.load(path) {
        eprintln!("config {}: {e}", path.display());
    }
    settings
}

/// The queue the last run left, put back on a fresh handle with everything
/// unfinished held: a restore is a reading of the list, never a spending of the
/// human's money (ADR 29). Returns how many tasks came back held.
/// Tell the model the ground it stands on, once per conversation.
///
/// The sandbox is named on the header and enforced by the OS, and the model
/// knew neither: a task spent thirty-six rounds probing where it could write,
/// paying for the discovery in tokens and in a working tree it had to clean up
/// afterwards. A system turn at the start says the root, what the policy
/// allows, and what to do about a file outside it.
///
/// Written once: a resumed conversation already carries it, and a second copy
/// would be a second opinion about the same ground.
pub fn ground(
    store: &pearl_session::SessionStore,
    root: &std::path::Path,
    policy: Policy,
) -> Result<bool, pearl_session::StoreError> {
    let words = ground_words(root, policy);
    let shown = pearl_session::view(&store.read_for_view()?);
    if shown
        .iter()
        .any(|e| e.role == pearl_session::Role::System && e.content.starts_with("You work inside"))
    {
        return Ok(false);
    }
    store.clone().append(pearl_session::Role::System, words)?;
    Ok(true)
}

fn ground_words(root: &std::path::Path, policy: Policy) -> String {
    let allows = match policy {
        Policy::Full => String::from("This launch may write anywhere."),
        Policy::ReadOnly => {
            String::from("This launch does not write files: treat everything as read-only.")
        }
        Policy::WorkspaceWrite => format!(
            "Writes land under {} and the scratch roots, and nowhere else; the \
             operating system refuses the rest.",
            root.display()
        ),
    };
    format!(
        "You work inside {}. {} To change a file outside that ground, say in your \
         answer what should change there instead of trying to save it.",
        root.display(),
        allows
    )
}

pub fn restore_queue(
    store: &SessionStore,
    queue: &Queue,
) -> Result<Option<usize>, pearl_session::StoreError> {
    use pearl_session::Role;

    // One read for both halves: the list as it stood, and the work each row
    // did. The snapshot carries the list; the answers, the thinking and the
    // bills are the entries written when they happened (ADR 29).
    let entries = store.read()?;
    let began = entries
        .iter()
        .rposition(|entry| entry.role == Role::Launch)
        .map(|at| at + 1)
        .unwrap_or(0);
    let run = &entries[began..];
    let Some(snapshot) = run.iter().rev().find(|entry| entry.role == Role::Queue) else {
        return Ok(None);
    };
    let held = queue.restore(&snapshot.content);

    for (task, report) in reports_in(run) {
        if let Ok(id) = task.parse() {
            queue.rehydrate(id, report);
        }
    }
    // A child's work is in its own log, beside this one. Its report is read
    // there rather than copied here: a child's conversation is its own.
    for node in queue.view() {
        if node.parent.is_none() || node.result.is_some() {
            continue;
        }
        let beside = store.dir().join(format!("parent-{}.jsonl", node.id));
        if let Ok(entries) = SessionStore::peek(&beside) {
            for (_, report) in reports_in(last_run(&entries)) {
                queue.rehydrate(node.id, report);
            }
        }
    }
    Ok(held)
}

/// The stretch of a log the last launch began.
fn last_run(entries: &[pearl_session::Entry]) -> &[pearl_session::Entry] {
    let began = entries
        .iter()
        .rposition(|entry| entry.role == pearl_session::Role::Launch)
        .map(|at| at + 1)
        .unwrap_or(0);
    &entries[began..]
}

/// What each task's work came to, read from the entries written for it.
fn reports_in(
    entries: &[pearl_session::Entry],
) -> std::collections::HashMap<String, pearl_core::Report> {
    use pearl_session::Role;

    let mut reports: std::collections::HashMap<String, pearl_core::Report> =
        std::collections::HashMap::new();
    for entry in entries {
        let Some(task) = entry.task.as_ref() else {
            continue;
        };
        let report = reports.entry(task.clone()).or_default();
        match entry.role {
            // The settling answer is the assistant turn that asked for
            // nothing; a turn that asked for tools is work on the way.
            Role::Assistant if entry.calls.is_none() => {
                report.result = Some(entry.content.clone());
            }
            Role::Reasoning => report.reasoning = Some(entry.content.clone()),
            Role::Bill => {
                if let Ok(turn) = serde_json::from_str::<pearl_core::Turn>(&entry.content) {
                    report.turns.push(turn);
                }
            }
            _ => {}
        }
    }
    reports
}

/// Keep the log's newest queue snapshot in step with the list the human is
/// editing. The queue is the surface's state, so it is written whenever it
/// changes -- not only when a task runs, or a list built at the keyboard and
/// closed without a run would be lost.
pub fn watch_queue(queue: Queue, mut store: SessionStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = String::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            // What counts as a change is the list, not the work in flight.
            let now = queue.standing();
            if now != last {
                let _ = store.append_queue(queue.snapshot());
                last = now;
            }
        }
    })
}
