//! Stories for configuration that lands while the agent is working.
//!
//! The human notices mid-task that the model is wrong or the thinking is too
//! shallow. Turning a knob must reach the next request, not the next process.

use pearl_agent::Agent;
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use rstest::rstest;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// A provider that records what it was asked for.
struct Recorder {
    seen: Arc<Mutex<Vec<CompletionRequest>>>,
}

type Seen = Arc<Mutex<Vec<CompletionRequest>>>;

impl Recorder {
    fn new() -> (Self, Seen) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (Self { seen: seen.clone() }, seen)
    }
}

impl Provider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        // A fold asks for a summary; anything else is a turn of work.
        let folding = request.messages.iter().any(|m| m.content.contains("compacting"));
        self.seen.lock().unwrap().push(request);
        Ok(Completion::text(if folding { "THE SUMMARY" } else { "ok" }, "mock"))
    }
}

fn agent(dir: &TempDir, settings: &Settings) -> (Agent<Recorder>, Seen) {
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let (provider, seen) = Recorder::new();
    (Agent::new(Queue::new(), provider, store, "mock").with_settings(settings.clone()), seen)
}

#[rstest]
#[tokio::test]
async fn given_a_shared_settings_when_the_model_is_changed_then_the_next_request_uses_it() {
    let dir = TempDir::new().unwrap();
    let settings = Settings::new("first-model");
    let (mut a, seen) = agent(&dir, &settings);
    a.queue.append("one");
    a.run().await.unwrap();

    // What the human does at the surface, between two tasks.
    settings.apply("model second-model").unwrap();
    a.queue.append("two");
    a.run().await.unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].model, "first-model");
    assert_eq!(seen[1].model, "second-model");
}

#[rstest]
#[tokio::test]
async fn given_a_shared_settings_when_the_effort_is_raised_then_the_next_request_carries_it() {
    let dir = TempDir::new().unwrap();
    let settings = Settings::new("m");
    settings.apply("dialect openai").unwrap();
    let (mut a, seen) = agent(&dir, &settings);
    a.queue.append("one");
    a.run().await.unwrap();
    settings.apply("effort high").unwrap();
    a.queue.append("two");
    a.run().await.unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].reasoning_effort, None);
    assert_eq!(seen[1].reasoning_effort.as_deref(), Some("high"));
}

#[rstest]
#[tokio::test]
async fn given_a_long_history_when_the_window_is_lowered_then_the_next_task_folds() {
    let dir = TempDir::new().unwrap();
    let settings = Settings::new("m");
    let mut store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    for n in 0..30 {
        store.append(pearl_session::Role::User, format!("turn {n} {}", "x".repeat(200))).unwrap();
        store.append(pearl_session::Role::Assistant, "y".repeat(200)).unwrap();
    }
    let (provider, seen) = Recorder::new();
    let mut a = Agent::new(Queue::new(), provider, store, "m").with_settings(settings.clone());
    a.queue.append("work");
    a.run().await.unwrap();
    let before = seen.lock().unwrap().len();

    // The human decides the window is smaller than they thought.
    settings.apply("window 2000").unwrap();
    a.queue.append("more work");
    a.run().await.unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen.len() > before + 1, "lowering the window folded nothing");
    let last = seen.last().unwrap();
    assert!(
        last.messages.iter().any(|m| m.content.contains("THE SUMMARY")),
        "the request after the fold carried no summary"
    );
}
