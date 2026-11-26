//! Stories for a queue that fills while it drains.
//!
//! `serve` is the scheduler's side of the surface: it takes whatever has been
//! placed, runs it, and waits for more instead of exiting. The queue is the
//! shared subject; the human keeps editing it while the work runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue, TaskId};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;
use tokio::sync::watch;

/// Answers every request and counts them. Optionally fails the first, and
/// optionally takes its time, so "still running" is a state a story can see.
struct Talker {
    calls: Arc<AtomicUsize>,
    fail_first: bool,
    delay: Duration,
}

impl Talker {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            delay: Duration::ZERO,
        }
    }

    fn failing_once() -> Self {
        Self {
            fail_first: true,
            ..Self::new()
        }
    }

    fn slow() -> Self {
        Self {
            delay: Duration::from_millis(150),
            ..Self::new()
        }
    }
}

impl Provider for Talker {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if self.fail_first && n == 0 {
            return Err(ProviderError::Transport(String::from(
                "the endpoint refused",
            )));
        }
        Ok(Completion {
            content: format!("answer {n}"),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage::default(),
        })
    }
}

fn store(dir: &TempDir) -> SessionStore {
    SessionStore::open(dir.path().join("session.jsonl")).unwrap()
}

/// Poll until `id` reaches `want`, or give up. The runner is another task, so
/// a story waits on the queue rather than on the scheduler.
async fn until(queue: &Queue, id: TaskId, want: NodeState) {
    for _ in 0..200 {
        if queue.get(id).map(|n| n.state) == Some(want) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let found = queue.get(id).map(|n| n.state);
    panic!("task never reached {want:?}: found {found:?}");
}

#[rstest]
#[tokio::test]
async fn given_a_task_placed_after_the_runner_starts_when_it_is_served_then_it_completes() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Talker::new(), store(&dir), "mock");
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    // The queue was empty when the runner started; placing now is the case the
    // surface is for.
    let (id, _) = queue.append("write the thing");
    until(&queue, id, NodeState::Done).await;

    assert_eq!(queue.get(id).unwrap().result.as_deref(), Some("answer 0"));
    drop(serving);
}

#[rstest]
#[tokio::test]
async fn given_work_in_flight_when_another_is_placed_then_both_finish_in_order() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Talker::slow(), store(&dir), "mock");
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    let (first, _) = queue.append("first");
    until(&queue, first, NodeState::Running).await;
    let (second, _) = queue.append("second");

    until(&queue, first, NodeState::Done).await;
    until(&queue, second, NodeState::Done).await;
    // The first answer was written to the first task: the queue did not mix
    // the two up while both were in flight.
    assert_eq!(
        queue.get(first).unwrap().result.as_deref(),
        Some("answer 0")
    );
    assert_eq!(
        queue.get(second).unwrap().result.as_deref(),
        Some("answer 1")
    );
    drop(serving);
}

#[rstest]
#[tokio::test]
async fn given_an_empty_queue_when_served_then_the_runner_waits_instead_of_exiting() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Talker::new(), store(&dir), "mock");
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !serving.is_finished(),
        "the runner exited with the queue empty"
    );

    let (id, _) = queue.append("placed after the wait");
    until(&queue, id, NodeState::Done).await;
    drop(serving);
}

#[rstest]
#[tokio::test]
async fn given_a_stop_signal_when_it_trips_then_the_runner_returns_and_leaves_work_queued() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Talker::slow(), store(&dir), "mock");
    let (stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    // Held, so the runner is provably idle when the stop arrives: what this
    // story pins is that stopping leaves work on the queue, not that a task
    // lost a race with the signal.
    let (id, _) = queue.append("never starts");
    queue.hold(id, "waiting for the human").unwrap();
    stop.send(true).unwrap();
    let served = tokio::time::timeout(Duration::from_secs(5), serving)
        .await
        .unwrap()
        .unwrap();
    assert!(served.is_ok());
    assert_eq!(queue.get(id).unwrap().state, NodeState::Queued);
}

#[rstest]
#[tokio::test]
async fn given_a_provider_that_fails_once_when_served_then_the_node_fails_and_the_next_runs() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let talker = Talker::failing_once();
    let calls = talker.calls.clone();
    let mut agent = Agent::new(queue.clone(), talker, store(&dir), "mock");
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    let (first, _) = queue.append("this one fails");
    until(&queue, first, NodeState::Failed).await;
    let (second, _) = queue.append("this one works");
    until(&queue, second, NodeState::Done).await;

    // A failed task is a badge, not the end of the session.
    assert!(queue.get(first).unwrap().error.is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(serving);
}
