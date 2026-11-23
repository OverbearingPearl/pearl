//! Stories for the loop: a task across the wire, steering at the boundary.

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that answers with a fixed string, no network.
struct FixedProvider(&'static str);

impl Provider for FixedProvider {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion { content: self.0.to_string(), model: "mock".into() })
    }
}

fn agent(dir: &TempDir, answer: &'static str) -> Agent<FixedProvider> {
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    Agent::new(Queue::new(), FixedProvider(answer), store, "mock")
}

#[rstest]
#[tokio::test]
async fn given_a_queued_task_when_run_then_it_completes_and_persists() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "the answer");
    let (id, _) = a.queue.append("do the thing");
    a.run().await.unwrap();
    assert_eq!(a.queue.get(id).unwrap().state, NodeState::Done);
    assert_eq!(a.queue.get(id).unwrap().result.as_deref(), Some("the answer"));
    // The exchange is on disk: a user turn and an assistant turn at least.
    let entries = a.store.read().unwrap();
    assert!(entries.len() >= 2);
}

#[rstest]
#[tokio::test]
async fn given_an_empty_queue_when_step_then_nothing_runs() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "x");
    assert!(a.step().await.unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn given_steering_before_a_turn_when_run_then_it_reaches_the_session() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "done");
    let (id, _) = a.queue.append("task");
    a.queue.steer(id, "also consider X").unwrap();
    a.run().await.unwrap();
    let entries = a.store.read().unwrap();
    assert!(entries.iter().any(|e| e.content == "also consider X"));
    // Steering is drained, not left pending.
    assert!(a.queue.get(id).unwrap().steering.is_empty());
}

#[rstest]
#[tokio::test]
async fn given_two_tasks_when_run_then_both_complete_in_order() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "ok");
    let (first, _) = a.queue.append("first");
    let (second, _) = a.queue.append("second");
    let events = a.run().await.unwrap();
    assert_eq!(a.queue.get(first).unwrap().state, NodeState::Done);
    assert_eq!(a.queue.get(second).unwrap().state, NodeState::Done);
    assert!(!events.is_empty());
}
