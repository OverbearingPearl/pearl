//! Stories for parallel fan-out: children run at the same time, each in its own
//! workspace copy, so one child's writes cannot land in another's tree. Each
//! story fails without the fan-out it names.

use std::sync::Arc;

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use pearl_tools::sandbox::Policy;
use rstest::rstest;
use tempfile::TempDir;
use tokio::sync::Barrier;
use tokio::time::{Duration, timeout};

/// A provider that answers at once, for stories about the queue and the tree.
struct Fixed(&'static str);

impl Provider for Fixed {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion { content: self.0.to_string(), model: "mock".into(), tool_calls: vec![] })
    }
}

/// A provider that will not answer until every child has asked. Reaching the
/// barrier is the proof of parallelism: a sequential fan-out would wait forever
/// on the first child and the test would time out rather than pass.
struct Rendezvous(Arc<Barrier>);

impl Provider for Rendezvous {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.0.wait().await;
        Ok(Completion { content: "together".to_string(), model: "mock".into(), tool_calls: vec![] })
    }
}

#[rstest]
#[tokio::test]
async fn given_two_children_when_fanned_out_then_both_are_in_flight_at_once() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut a = Agent::new(Queue::new(), Rendezvous(barrier), store, "mock");
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    // Both children must be asking at the same time for the barrier to open, so
    // a sequential fan-out hangs here and fails by timeout instead of passing.
    let settled = timeout(Duration::from_secs(10), a.run_children(&spawned))
        .await
        .expect("a sequential fan-out would never open the barrier")
        .unwrap();
    assert_eq!(settled, 2);
    for s in &spawned {
        assert_eq!(a.queue.get(s.child).unwrap().state, NodeState::Done);
    }
}

#[rstest]
#[tokio::test]
async fn given_children_when_fanned_out_then_each_works_on_its_own_copy() {
    let home = TempDir::new().unwrap();
    std::fs::write(home.path().join("shared.txt"), "original").unwrap();
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("done"), store, "mock")
        .with_tools(home.path(), Policy::WorkspaceWrite);
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    // Each child got a copy of the workspace, and the copies are distinct.
    assert_ne!(spawned[0].workspace, spawned[1].workspace);
    for s in &spawned {
        assert!(s.workspace.join("shared.txt").exists());
        assert_ne!(s.workspace.as_path(), home.path());
    }
}

#[rstest]
#[tokio::test]
async fn given_two_children_writing_the_same_name_then_neither_sees_the_other() {
    let home = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("done"), store, "mock")
        .with_tools(home.path(), Policy::WorkspaceWrite);
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    // Both children write the same file name inside their own copy.
    for s in &spawned {
        std::fs::write(s.workspace.join("shared.txt"), "mine").unwrap();
    }
    // The copies diverge, and the original tree was never touched.
    assert!(!home.path().join("shared.txt").exists());
    let first = std::fs::read_to_string(spawned[0].workspace.join("shared.txt")).unwrap();
    let second = std::fs::read_to_string(spawned[1].workspace.join("shared.txt")).unwrap();
    assert_eq!(first, "mine");
    assert_eq!(second, "mine");
}

#[rstest]
#[tokio::test]
async fn given_children_in_flight_when_the_queue_is_read_then_all_show_running() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("done"), store, "mock");
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    a.queue.focus_on(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    // The queue marks every child running before the fan-in, so the surface can
    // show three nodes in flight while the human still focuses the parent.
    for s in &spawned {
        a.queue.start(s.child).unwrap();
    }
    for s in &spawned {
        assert_eq!(a.queue.get(s.child).unwrap().state, NodeState::Running);
    }
    assert_eq!(a.queue.focus(), Some(parent));
}
