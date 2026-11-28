//! Stories for folding history when the context fills up.
//!
//! The loop is what decides, so these stories drive a run to the window and
//! read what the model was shown: a fold is real when the request after it is
//! smaller and still carries the work.

use std::sync::Mutex;

use pearl_core::Queue;
use pearl_log::{Budget, Role, Store};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{FoldPolicy, Runner};
use rstest::rstest;
use tempfile::TempDir;

/// A provider that writes a summary when it is asked to fold, answers a task
/// when it is asked to work, and remembers every request it saw.
struct Recorder {
    requested: Mutex<Vec<CompletionRequest>>,
    answer: &'static str,
}

impl Recorder {
    fn new() -> Self {
        Self {
            requested: Mutex::new(Vec::new()),
            answer: "the task is done",
        }
    }

    fn folds(&self) -> usize {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                r.messages
                    .iter()
                    .any(|m| m.content.contains("You are folding"))
            })
            .count()
    }

    /// What the fold itself was shown, which is not what the task was shown.
    fn fold_messages(&self) -> Vec<String> {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .find(|r| {
                r.messages
                    .iter()
                    .any(|m| m.content.contains("You are folding"))
            })
            .map(|r| r.messages.iter().map(|m| m.content.clone()).collect())
            .unwrap_or_default()
    }

    /// Which model each request was addressed to, and whether it was a fold.
    fn models_asked(&self) -> Vec<(String, bool)> {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r.model.clone(),
                    r.messages
                        .iter()
                        .any(|m| m.content.contains("You are folding")),
                )
            })
            .collect()
    }

    fn last_messages(&self) -> Vec<String> {
        self.requested
            .lock()
            .unwrap()
            .last()
            .map(|r| r.messages.iter().map(|m| m.content.clone()).collect())
            .unwrap_or_default()
    }
}

impl Provider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let folding = request
            .messages
            .iter()
            .any(|m| m.content.contains("You are folding"));
        let content = if folding {
            "SUMMARY: we read files and decided things"
        } else {
            self.answer
        };
        self.requested.lock().unwrap().push(request);
        Ok(Completion {
            content: content.to_string(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: 4_000,
                completion_tokens: 20,
                cached_tokens: 0,
            },
        })
    }
}

/// A budget that lets a short seeded history exceed the tail.
fn tight() -> FoldPolicy {
    FoldPolicy {
        budget: Budget {
            keep_recent: 3,
            tail_tokens: 200,
            min_fold_tokens: 100,
        },
    }
}

/// A window small enough that the seeded history is over it. The window is a
/// live setting now, so the story sets it the way the human would.
fn small_window() -> pearl_core::Settings {
    let settings = pearl_core::Settings::new("mock");
    settings.apply("window 1000").unwrap();
    settings
}

/// Seed a log with `rounds` user/assistant pairs of readable size.
fn seed(dir: &TempDir, bills: usize) -> Store {
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..bills {
        store
            .append(Role::User, format!("round {i}: {}", "x".repeat(400)))
            .unwrap();
        store
            .append(Role::Assistant, format!("reply {i}: {}", "y".repeat(400)))
            .unwrap();
    }
    store
}

#[rstest]
#[tokio::test]
async fn given_a_log_over_the_window_when_a_task_starts_then_the_model_reads_a_summary() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let recorder = Recorder::new();
    let mut runner = Runner::new(Queue::new(), recorder, store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    runner.queue.append("the next task");

    runner.run().await.unwrap();

    // The fold happened before the task's own request.
    assert_eq!(runner.provider().folds(), 1);
    let shown = runner.provider().last_messages();
    assert!(
        shown[0].starts_with("SUMMARY: we read files"),
        "first message was {shown:?}"
    );
    // The summary is in the log, and so is everything it stands in for: the
    // view is folded, the record is not.
    let entries = runner.log().read().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e.role == Role::Fold && e.covers.is_some())
    );
    assert!(entries.len() > 40);
}

#[rstest]
#[tokio::test]
async fn given_a_folded_log_when_it_is_reopened_then_the_summary_is_reused() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let path = store.path().to_path_buf();

    let mut first = Runner::new(Queue::new(), Recorder::new(), store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    first.queue.append("first task");
    first.run().await.unwrap();
    assert_eq!(first.provider().folds(), 1);
    drop(first);

    // A second process opens the same file. The view is still over the window,
    // but the summary it needs is already in the log, so no model is asked for
    // it a second time.
    let second_store = Store::open(&path).unwrap();
    let mut second = Runner::new(Queue::new(), Recorder::new(), second_store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    second.queue.append("second task");
    second.run().await.unwrap();

    assert_eq!(second.provider().folds(), 0);
    assert!(second.provider().last_messages()[0].starts_with("SUMMARY: we read files"));
}

#[rstest]
#[tokio::test]
async fn given_billed_usage_when_the_model_reports_it_then_the_meter_learns_the_scale() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 10);
    let mut runner = Runner::new(Queue::new(), Recorder::new(), store, "mock");
    runner.queue.append("a task");
    runner.run().await.unwrap();

    // The mock billed 4000 prompt tokens for a request the character count
    // read as roughly two thousand: the meter learns the scale, so the next
    // estimate is close.
    assert_eq!(runner.meter().samples(), 1);
    assert!(runner.meter().weight() > 1.0);
}

#[rstest]
#[tokio::test]
async fn given_a_log_that_fits_when_a_task_starts_then_nothing_is_folded() {
    let dir = TempDir::new().unwrap();
    // One pair is eight hundred characters, two hundred tokens against a
    // window of a thousand: a history that fits, whatever the fold fraction.
    let store = seed(&dir, 1);
    let mut runner = Runner::new(Queue::new(), Recorder::new(), store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    runner.queue.append("a small task");

    runner.run().await.unwrap();

    assert_eq!(runner.provider().folds(), 0);
    assert!(
        !runner
            .log()
            .read()
            .unwrap()
            .iter()
            .any(|e| e.role == Role::Fold)
    );
}

#[rstest]
#[tokio::test]
async fn given_a_history_over_the_window_when_it_folds_then_the_task_carries_the_news() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let mut runner = Runner::new(Queue::new(), Recorder::new(), store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    let (id, _) = runner.queue.append("the next task");

    runner.run().await.unwrap();

    let notices = runner.queue.get(id).unwrap().notices;
    assert!(
        notices.iter().any(|n| n.contains("folded")),
        "history folded and the task carried none of the news: {notices:?}"
    );
}

/// Seed a log whose bulk is shell output: a call, then a long answer to it.
/// The opening line is the op handle and the last line is what went wrong,
/// which is the pair a fold has to keep.
fn seed_output(dir: &TempDir, bills: usize, bytes: usize) -> Store {
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..bills {
        store
            .append_asked(
                format!(
                    "[{{\"id\":\"c{i}\",\"type\":\"function\",\"function\":\
                     {{\"name\":\"bash\",\"arguments\":\"{{}}\"}}}}]"
                ),
                format!("calling round {i}"),
            )
            .unwrap();
        let body = format!(
            "[op {i}] cargo test\n{}\nerror: test failed, to rerun pass `--lib`",
            "x".repeat(bytes)
        );
        store.append_answered(format!("c{i}"), body).unwrap();
    }
    store
}

#[rstest]
#[tokio::test]
async fn given_a_history_of_long_shell_output_when_it_folds_then_the_summary_reads_the_ends_and_not_the_middle()
 {
    let dir = TempDir::new().unwrap();
    let store = seed_output(&dir, 12, 8_000);
    let recorder = Recorder::new();
    let mut runner = Runner::new(Queue::new(), recorder, store, "mock")
        .with_fold(tight())
        .with_settings(small_window());
    runner.queue.append("the next task");

    runner.run().await.unwrap();

    assert_eq!(runner.provider().folds(), 1, "the history never folded");
    let shown = runner.provider().fold_messages();
    let transcript = shown
        .iter()
        .find(|m| m.starts_with("Fold these entries"))
        .expect("the fold read no transcript");
    // The handle and the failure survive: a summary that drops either makes
    // the agent run the command again to learn what it already knew.
    assert!(
        transcript.contains("[op 0] cargo test"),
        "{transcript:.200}"
    );
    assert!(
        transcript.contains("error: test failed, to rerun pass `--lib`"),
        "the fold lost the line that said what went wrong"
    );
    // The middle does not. Twelve outputs of eight thousand characters are
    // ninety-six thousand characters of `x`, and a fold that reads them all
    // pays for the bulk it exists to throw away.
    assert!(
        transcript.len() < 30_000,
        "the fold read {} characters of a history it was meant to shorten",
        transcript.len()
    );
    assert!(
        transcript.contains("folded]"),
        "nothing said the middle was left out: {transcript:.200}"
    );
}

/// A provider that asks for one shell round after another, so a single task
/// can fill the window by itself -- the shape a log has when a launch
/// runs one task.
struct Rounder {
    requested: Mutex<Vec<CompletionRequest>>,
    left: Mutex<usize>,
}

impl Provider for Rounder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let folding = request
            .messages
            .iter()
            .any(|m| m.content.contains("You are folding"));
        let mut left = self.left.lock().unwrap();
        let tool_calls = if folding || *left == 0 {
            vec![]
        } else {
            *left -= 1;
            vec![pearl_provider::ToolCall {
                id: format!("call-{left}"),
                kind: "function".into(),
                function: pearl_provider::Function {
                    name: "bash".into(),
                    // A literal, not a shell expansion: `seq` and `printf`
                    // are not `cmd` builtins, and a story about the window
                    // must not depend on which programs the platform ships.
                    arguments: serde_json::json!({"command": format!("echo {}", "x".repeat(900))})
                        .to_string(),
                },
            }]
        };
        let content = if folding {
            "SUMMARY: the task read a lot".to_string()
        } else {
            "done".to_string()
        };
        self.requested.lock().unwrap().push(request);
        Ok(Completion {
            content,
            reasoning: None,
            model: "mock".into(),
            tool_calls,
            usage: Usage {
                prompt_tokens: 4_000,
                completion_tokens: 20,
                cached_tokens: 0,
            },
        })
    }
}

#[rstest]
#[tokio::test]
#[cfg_attr(
    windows,
    ignore = "the task that fills the window does not settle on Windows: M111"
)]
async fn given_one_task_that_fills_the_window_when_it_runs_then_it_folds_its_own_messages() {
    // The fold used to be attempted once, before a task's first request. A
    // launch that runs one task therefore never folded, however long the task
    // grew: the header showed a context past the window and no summary.
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let provider = Rounder {
        requested: Mutex::new(Vec::new()),
        left: Mutex::new(8),
    };
    let mut runner = Runner::new(Queue::new(), provider, store, "mock")
        .with_fold(tight())
        .with_settings(small_window())
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = runner.queue.append("one long task");

    runner.run().await.unwrap();

    let entries = runner.log().read().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e.role == Role::Fold && e.covers.is_some()),
        "a task past the window folded nothing"
    );
    let notices = runner.queue.get(id).unwrap().notices.clone();
    let fold = notices
        .iter()
        .find(|n| n.contains("folded"))
        .expect("no fold notice");
    assert!(
        fold.contains("→"),
        "the fold notice does not say how much it reclaimed: {fold:?}"
    );
    assert!(
        fold.contains("→ ") && fold.contains(", to fit"),
        "the notice counts entries but not the size it saved: {fold:?}"
    );
}

// Writing a page of prose from a transcript is not the work the expensive
// model is for. The human who names a cheaper one on the same endpoint should
// have every fold go to it, and the work still go where they chose.
#[rstest]
#[tokio::test]
async fn given_a_cheaper_model_named_for_the_fold_when_the_history_folds_then_the_summary_is_asked_of_it()
 {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let settings = small_window();
    settings.apply("fold cheap-writer").unwrap();
    let recorder = Recorder::new();
    let mut runner = Runner::new(Queue::new(), recorder, store, "mock")
        .with_fold(tight())
        .with_settings(settings);
    runner.queue.append("the next task");

    runner.run().await.unwrap();

    let asked = runner.provider().models_asked();
    assert_eq!(runner.provider().folds(), 1, "the history never folded");
    assert!(
        asked
            .iter()
            .any(|(m, folding)| *folding && m == "cheap-writer"),
        "the fold went to the working model: {asked:?}"
    );
    assert!(
        asked.iter().any(|(m, folding)| !*folding && m == "mock"),
        "the work followed the fold to the cheap model: {asked:?}"
    );
}
