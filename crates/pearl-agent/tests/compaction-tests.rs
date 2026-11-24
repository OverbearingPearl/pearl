//! Stories for folding history when the context fills up.
//!
//! The loop is what decides, so these stories drive a session to the window and
//! read what the model was shown: a fold is real when the request after it is
//! smaller and still carries the work.

use std::sync::Mutex;

use pearl_agent::{Agent, CompactionPolicy};
use pearl_core::Queue;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::{Budget, Role, SessionStore};
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
        Self { requested: Mutex::new(Vec::new()), answer: "the task is done" }
    }

    fn folds(&self) -> usize {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.messages.iter().any(|m| m.content.contains("You are compacting")))
            .count()
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
        let folding = request.messages.iter().any(|m| m.content.contains("You are compacting"));
        let content = if folding { "SUMMARY: we read files and decided things" } else { self.answer };
        self.requested.lock().unwrap().push(request);
        Ok(Completion {
            content: content.to_string(),
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage { prompt_tokens: 4_000, completion_tokens: 20 },
        })
    }
}

/// A budget that lets a short seeded history exceed the tail.
fn tight() -> CompactionPolicy {
    CompactionPolicy { budget: Budget { keep_recent: 3, tail_tokens: 200, min_fold_tokens: 100 } }
}

/// A window small enough that the seeded history is over it. The window is a
/// live setting now, so the story turns it the way the human would.
fn small_window() -> pearl_core::Settings {
    let settings = pearl_core::Settings::new("mock");
    settings.apply("window 1000").unwrap();
    settings
}

/// Seed a log with `turns` user/assistant pairs of readable size.
fn seed(dir: &TempDir, turns: usize) -> SessionStore {
    let mut store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    for i in 0..turns {
        store.append(Role::User, format!("turn {i}: {}", "x".repeat(400))).unwrap();
        store.append(Role::Assistant, format!("reply {i}: {}", "y".repeat(400))).unwrap();
    }
    store
}

#[rstest]
#[tokio::test]
async fn given_a_session_over_the_window_when_a_task_starts_then_the_model_reads_a_summary() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let recorder = Recorder::new();
    let mut agent = Agent::new(Queue::new(), recorder, store, "mock").with_compaction(tight()).with_settings(small_window());
    agent.queue.append("the next task");

    agent.run().await.unwrap();

    // The fold happened before the task's own request.
    assert_eq!(agent.provider().folds(), 1);
    let shown = agent.provider().last_messages();
    assert!(shown[0].starts_with("SUMMARY: we read files"), "first message was {shown:?}");
    // The summary is in the log, and so is everything it stands in for: the
    // view is compacted, the record is not.
    let entries = agent.session().read().unwrap();
    assert!(entries.iter().any(|e| e.role == Role::Compaction && e.covers.is_some()));
    assert!(entries.len() > 40);
}

#[rstest]
#[tokio::test]
async fn given_a_folded_session_when_it_is_reopened_then_the_summary_is_reused() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let path = store.path().to_path_buf();

    let mut first = Agent::new(Queue::new(), Recorder::new(), store, "mock").with_compaction(tight()).with_settings(small_window());
    first.queue.append("first task");
    first.run().await.unwrap();
    assert_eq!(first.provider().folds(), 1);
    drop(first);

    // A second process opens the same file. The view is still over the window,
    // but the summary it needs is already in the log, so no model is asked for
    // it a second time.
    let second_store = SessionStore::open(&path).unwrap();
    let mut second =
        Agent::new(Queue::new(), Recorder::new(), second_store, "mock").with_compaction(tight()).with_settings(small_window());
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
    let mut agent = Agent::new(Queue::new(), Recorder::new(), store, "mock");
    agent.queue.append("a task");
    agent.run().await.unwrap();

    // The mock billed 4000 prompt tokens for a request the character count
    // read as roughly two thousand: the meter learns the scale, so the next
    // estimate is close.
    assert_eq!(agent.meter().samples(), 1);
    assert!(agent.meter().weight() > 1.0);
}

#[rstest]
#[tokio::test]
async fn given_a_session_that_fits_when_a_task_starts_then_nothing_is_folded() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 2);
    let mut agent = Agent::new(Queue::new(), Recorder::new(), store, "mock").with_compaction(tight()).with_settings(small_window());
    agent.queue.append("a small task");

    agent.run().await.unwrap();

    assert_eq!(agent.provider().folds(), 0);
    assert!(!agent.session().read().unwrap().iter().any(|e| e.role == Role::Compaction));
}
