//! Stories for configuration that lands while the agent is working.
//!
//! The human notices mid-task that the model is wrong or the thinking is too
//! shallow. Turning a knob must reach the next request, not the next process.

use pearl_agent::Agent;
use pearl_core::{Dialect, Effort, Queue, Settings};
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

/// A provider that refuses the first request with the endpoint's own words.
struct Refuses {
    seen: Seen,
    words: String,
}

impl Provider for Refuses {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let first = self.seen.lock().unwrap().is_empty();
        self.seen.lock().unwrap().push(request);
        if first {
            return Err(ProviderError::Refused { status: 400, message: self.words.clone() });
        }
        Ok(Completion::text("ok", "mock"))
    }
}

fn refusing(dir: &TempDir, settings: &Settings, words: &str) -> (Agent<Refuses>, Seen) {
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let provider = Refuses { seen: seen.clone(), words: words.to_string() };
    (Agent::new(Queue::new(), provider, store, "mock").with_settings(settings.clone()), seen)
}

#[rstest]
#[tokio::test]
async fn given_an_endpoint_that_refuses_the_thinking_field_when_it_refuses_then_the_retry_carries_none()
 {
    let dir = TempDir::new().unwrap();
    let settings = Settings::new("m");
    settings.apply("dialect openai").unwrap();
    settings.apply("effort high").unwrap();
    let (mut a, seen) = refusing(
        &dir,
        &settings,
        "Unsupported parameter: 'reasoning_effort' is not supported with this model.",
    );
    a.queue.append("one");
    a.run().await.unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "the refusal was not retried");
    assert_eq!(seen[0].reasoning_effort.as_deref(), Some("high"));
    assert_eq!(seen[1].reasoning_effort, None, "the retry still carried the refused field");
    assert_eq!(settings.effort(), None, "the surface still shows a rung the endpoint refuses");
    // The rung is what the endpoint named, so the rung is what is remembered
    // -- against this house. Throwing the dialect away would take the other
    // four rungs down with the one that was wrong.
    assert_eq!(settings.refused(), vec![Effort::High], "the refused rung was not remembered");
    assert_eq!(settings.dialect(), Dialect::OpenAi, "one refused rung threw the dialect away");
    let node = a.queue.view().iter().find(|n| n.title == "one").unwrap().clone();
    assert_eq!(node.result.as_deref(), Some("ok"), "the corrected answer never reached the task");
}

#[rstest]
#[tokio::test]
async fn given_a_refusal_about_something_else_when_it_arrives_then_the_task_fails_with_the_servers_words()
 {
    let dir = TempDir::new().unwrap();
    let settings = Settings::new("m");
    settings.apply("dialect openai").unwrap();
    settings.apply("effort high").unwrap();
    let (mut a, seen) =
        refusing(&dir, &settings, "Invalid 'model': no such model exists on this account.");
    a.queue.append("one");
    a.run().await.unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "a refusal about the model was retried as if it were about thinking");
    assert_eq!(settings.effort().map(|e| e.as_str()), Some("high"), "an unrelated refusal moved a knob");
    let node = a.queue.view().iter().find(|n| n.title == "one").unwrap().clone();
    let said = node.error.unwrap_or_default();
    assert!(said.contains("no such model"), "the server's words did not reach the human: {said}");
    assert!(said.contains("400"), "the status did not reach the human: {said}");
}
