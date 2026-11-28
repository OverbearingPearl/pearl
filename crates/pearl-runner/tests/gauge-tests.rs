//! Stories for the runner reporting what the window holds.

use pearl_core::{Gauge, Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::Runner;
use rstest::rstest;
use tempfile::TempDir;

struct Costly;

impl Provider for Costly {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let folding = request
            .messages
            .iter()
            .any(|m| m.content.contains("folding"));
        Ok(Completion {
            content: if folding {
                "SUMMARY".into()
            } else {
                "ok".into()
            },
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: 30_000,
                completion_tokens: 5,
                cached_tokens: 0,
            },
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_request_that_cost_tokens_when_it_returns_then_the_gauge_shows_the_cost() {
    let dir = TempDir::new().unwrap();
    let gauge = Gauge::new();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Runner::new(Queue::new(), Costly, store, "mock").with_gauge(gauge.clone());
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
    let mut store = Store::open(dir.path().join("s.jsonl")).unwrap();
    for n in 0..40 {
        store
            .append(
                pearl_log::Role::User,
                format!("round {n} {}", "x".repeat(1600)),
            )
            .unwrap();
        store
            .append(pearl_log::Role::Assistant, "y".repeat(1600))
            .unwrap();
    }
    let mut a = Runner::new(Queue::new(), Costly, store, "mock")
        .with_settings(settings)
        .with_gauge(gauge.clone());
    a.queue.append("work");
    a.run().await.unwrap();
    assert!(gauge.folds() >= 1, "a fold happened and nothing counted it");
}

/// A fold is a request the human paid for without asking for it. The bill has
/// to carry it: a ledger that counts only the rounds a task asked for
/// under-reports the log by every fold it ever made.
#[rstest]
#[tokio::test]
async fn given_a_fold_when_the_directory_bill_is_read_then_the_fold_is_paid_for_too() {
    let dir = TempDir::new().unwrap();
    let gauge = Gauge::new();
    let settings = Settings::new("mock");
    settings.apply("window 40000").unwrap();
    let mut store = Store::open(dir.path().join("s.jsonl")).unwrap();
    for n in 0..40 {
        store
            .append(
                pearl_log::Role::User,
                format!("round {n} {}", "x".repeat(1600)),
            )
            .unwrap();
        store
            .append(pearl_log::Role::Assistant, "y".repeat(1600))
            .unwrap();
    }
    let mut a = Runner::new(Queue::new(), Costly, store, "mock")
        .with_settings(settings)
        .with_gauge(gauge.clone());
    a.queue.append("work");
    a.run().await.unwrap();

    let folds = gauge.folds();
    assert!(folds >= 1, "no fold ran to pay for");
    // One task round plus every fold: the ledger counts requests, not tasks.
    assert_eq!(
        gauge.cost().rounds,
        folds + 1,
        "the bill missed the fold: {} round(s) for {folds} fold(s) and one task",
        gauge.cost().rounds
    );
}

/// A provider whose prompt is mostly served from cache.
struct Cached;

impl Provider for Cached {
    async fn complete(&self, _r: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion {
            content: "ok".into(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: 1000,
                completion_tokens: 5,
                cached_tokens: 900,
            },
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_prompt_served_from_cache_when_it_returns_then_the_gauge_shows_the_saving() {
    let dir = TempDir::new().unwrap();
    let gauge = Gauge::new();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Runner::new(Queue::new(), Cached, store, "mock").with_gauge(gauge.clone());
    a.queue.append("work");
    a.run().await.unwrap();
    assert_eq!(gauge.cache_rate(), Some(90));
    assert!(
        gauge.describe(128_000).contains("cache 90%"),
        "{}",
        gauge.describe(128_000)
    );
}

#[rstest]
#[tokio::test]
async fn given_a_task_that_ran_when_its_cost_is_read_then_the_bill_is_on_the_task_and_the_log() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let gauge = pearl_core::Gauge::new();
    let mut a = Runner::new(Queue::new(), Costly, store, "mock")
        .with_settings(Settings::new("mock"))
        .with_gauge(gauge.clone());
    let (id, _) = a.queue.append("work");
    a.run().await.unwrap();
    let cost = a.queue.get(id).unwrap().cost;
    assert!(
        cost.up > 0 && cost.rounds >= 1,
        "the task carried no bill: {cost:?}"
    );
    assert_eq!(gauge.cost().up, cost.up, "the log and the task disagree");
    assert!(gauge.cost().ms > 0 || cost.ms == 0, "no time was measured");
}
