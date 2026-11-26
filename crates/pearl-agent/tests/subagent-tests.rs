//! Stories for subagents: a node fans out into derived sessions that settle
//! back as queue events while the human keeps one focus. Each story fails
//! without the fan-out it names.

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that answers with a fixed string, so the loop is testable
/// offline and every subagent settles deterministically.
struct Fixed(&'static str);

impl Provider for Fixed {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion {
            content: self.0.to_string(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_running_parent_when_spawn_then_children_have_their_own_sessions() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("ok"), store, "mock");
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    assert_eq!(spawned.len(), 2);
    // Each child is a node under the parent, with a distinct session file.
    for s in &spawned {
        assert_eq!(a.queue.get(s.child).unwrap().parent, Some(parent));
        assert!(s.session.exists());
        assert_ne!(s.session, dir.path().join("parent.jsonl"));
    }
    assert_ne!(spawned[0].session, spawned[1].session);
}

#[rstest]
#[tokio::test]
async fn given_fanned_out_parent_when_children_settle_then_parent_finishes() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("done"), store, "mock");
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    // The parent records its result but cannot finish while children run.
    a.queue.complete(parent, "orchestrated").unwrap();
    assert_eq!(a.queue.get(parent).unwrap().state, NodeState::Running);
    // Run each subagent in its derived session; they settle.
    for s in spawned {
        let child_store = SessionStore::open(&s.session).unwrap();
        a.run_subagent(s.child, child_store).await.unwrap();
        assert_eq!(a.queue.get(s.child).unwrap().state, NodeState::Done);
    }
    // The last child settling lets the waiting parent finish.
    assert_eq!(a.queue.get(parent).unwrap().state, NodeState::Done);
}

#[rstest]
#[tokio::test]
async fn given_fan_out_when_children_run_then_human_focus_stays_on_parent() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("done"), store, "mock");
    let (parent, _) = a.queue.append("the big task");
    a.queue.start(parent).unwrap();
    a.queue.focus_on(parent).unwrap();
    let spawned = a.spawn(parent, &["part A", "part B"]).unwrap();
    for s in spawned {
        let child_store = SessionStore::open(&s.session).unwrap();
        a.run_subagent(s.child, child_store).await.unwrap();
    }
    // Execution fanned out; attention did not. One focus, on the parent.
    assert_eq!(a.queue.focus(), Some(parent));
}

#[rstest]
#[tokio::test]
async fn given_subagent_when_it_runs_then_its_session_is_separate_from_parent() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("parent.jsonl")).unwrap();
    let mut a = Agent::new(Queue::new(), Fixed("child answer"), store, "mock");
    let (parent, _) = a.queue.append("parent task");
    a.queue.start(parent).unwrap();
    let spawned = a.spawn(parent, &["child task"]).unwrap();
    let s = &spawned[0];
    let child_store = SessionStore::open(&s.session).unwrap();
    a.run_subagent(s.child, child_store).await.unwrap();
    // The child's answer is in the child's session, not the parent's.
    let child_entries = SessionStore::open(&s.session).unwrap().read().unwrap();
    assert!(child_entries.iter().any(|e| e.content == "child answer"));
    let parent_entries = a.session().read().unwrap();
    assert!(!parent_entries.iter().any(|e| e.content == "child answer"));
}
