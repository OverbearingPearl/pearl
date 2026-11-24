//! Stories for the runner reporting what the window holds.

use pearl_agent::Agent;
use pearl_core::{Gauge, Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

struct Costly;

impl Provider for Costly {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let folding = request.messages.iter().any(|m| m.content.contains("compacting"));
        Ok(Completion {
            content: if folding { "SUMMARY".into() } else { "ok".into() },
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage { prompt_tokens: 30_000, completion_tokens: 5 },
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_request_that_cost_tokens_when_it_returns_then_the_gauge_shows_the_cost() {
    let dir = TempDir::new().unwrap();
    let gauge = Gauge::new();
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Costly, store, "mock").with_gauge(gauge.clone());
    a.queue.append("work");
    a.run().await.unwrap();
    assert_eq!(gauge.prompt_tokens(), 30_000);
}

#[rstest]
#[tokio::test]
async fn given_a_history_over_the_window_when_it_folds_then_the_gauge_counts_the_fold() {
    let dir = TempDir::new().unwrap();
    let gauge = Gauge::new();
    let settings = Settings::new("mock");
    settings.apply("window 40000").unwrap();
    let mut store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    for n in 0..40 {
        store.append(pearl_session::Role::User, format!("turn {n} {}", "x".repeat(1600))).unwrap();
        store.append(pearl_session::Role::Assistant, "y".repeat(1600)).unwrap();
    }
    let mut a = Agent::new(Queue::new(), Costly, store, "mock")
        .with_settings(settings)
        .with_gauge(gauge.clone());
    a.queue.append("work");
    a.run().await.unwrap();
    assert!(gauge.folds() >= 1, "a fold happened and nothing counted it");
}
