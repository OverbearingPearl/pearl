//! Stories for the runner the surface starts.
//!
//! The surface and the runner are two holders of one queue. What these stories
//! pin is that the runner is given the queue the surface keeps: the bug they
//! rule out is a runner over a second, empty queue that never sees a task.

use std::time::Duration;

use pearl_core::{NodeState, Queue, TaskId};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

struct Talker;

impl Provider for Talker {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion::text("answered by the runner", "mock"))
    }
}

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

fn store(dir: &TempDir) -> SessionStore {
    SessionStore::open(dir.path().join("session.jsonl")).unwrap()
}

#[rstest]
#[tokio::test]
async fn given_a_runner_started_over_the_queue_when_a_task_is_placed_then_that_queue_shows_it_done() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let (halt, handle) = pearl::spawn_runner(queue.clone(), Talker, store(&dir), "mock");

    let (id, _) = queue.append("ship it");
    until(&queue, id, NodeState::Done).await;
    assert_eq!(queue.get(id).unwrap().result.as_deref(), Some("answered by the runner"));

    halt.send(true).unwrap();
    handle.await.unwrap().unwrap();
}

#[rstest]
#[tokio::test]
async fn given_a_runner_when_halted_then_a_task_placed_afterwards_stays_queued() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let (halt, handle) = pearl::spawn_runner(queue.clone(), Talker, store(&dir), "mock");

    halt.send(true).unwrap();
    handle.await.unwrap().unwrap();

    let (id, _) = queue.append("too late");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(queue.get(id).unwrap().state, NodeState::Queued);
}
