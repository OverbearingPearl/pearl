//! Stories for keeping the thinking without sending it back.
//!
//! The effort knob was added and its whole output discarded. Thinking is the
//! human's to read; it is not a turn the model had, and an endpoint that
//! returns it rejects it as input on the next request.

use pearl_agent::Agent;
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Role};
use pearl_session::SessionStore;
use rstest::rstest;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct Thinker {
    calls: AtomicUsize,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Provider for Thinker {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.seen.lock().unwrap().push(
            request.messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join("|"),
        );
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut c = Completion::text(if n == 0 { "the answer" } else { "second answer" }, "mock");
        c.reasoning = Some(String::from("WEIGHING-THE-OPTIONS"));
        Ok(c)
    }
}

fn agent(dir: &TempDir) -> Agent<Thinker> {
    agent_with_seen(dir, Arc::new(Mutex::new(Vec::new())))
}

fn agent_with_seen(dir: &TempDir, seen: Arc<Mutex<Vec<String>>>) -> Agent<Thinker> {
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    Agent::new(
        Queue::new(),
        Thinker { calls: AtomicUsize::new(0), seen },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
}

#[rstest]
#[tokio::test]
async fn given_a_model_that_thinks_when_the_task_settles_then_the_thinking_is_on_the_node() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir);
    let (id, _) = a.queue.append("a question");
    a.run().await.unwrap();
    let node = a.queue.get(id).unwrap();
    assert_eq!(node.reasoning.as_deref(), Some("WEIGHING-THE-OPTIONS"));
    assert_eq!(node.result.as_deref(), Some("the answer"));
}

#[rstest]
#[tokio::test]
async fn given_a_recorded_thought_when_the_next_request_is_built_then_it_is_not_sent_back() {
    let dir = TempDir::new().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut a = agent_with_seen(&dir, seen.clone());
    a.queue.append("first");
    a.run().await.unwrap();
    a.queue.append("second");
    a.run().await.unwrap();
    let seen = seen.lock().unwrap();
    assert!(
        !seen.last().unwrap().contains("WEIGHING-THE-OPTIONS"),
        "the thinking went back to the endpoint: {:?}",
        seen.last()
    );
}

#[rstest]
#[tokio::test]
async fn given_a_model_that_thinks_when_the_log_is_read_then_the_thought_is_recorded() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir);
    a.queue.append("a question");
    a.run().await.unwrap();
    let entries = a.session().read().unwrap();
    assert!(
        entries.iter().any(|e| e.role == pearl_session::Role::Reasoning
            && e.content.contains("WEIGHING")),
        "the thinking was not kept: {:?}",
        entries.iter().map(|e| (e.role, e.content.clone())).collect::<Vec<_>>()
    );
    let _ = Role::User;
}
