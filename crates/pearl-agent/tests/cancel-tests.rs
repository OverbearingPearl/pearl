//! Stories for a cancel the runner keeps.
//!
//! A cancel the screen shows and the runner ignores is worse than no cancel at
//! all: the human pays for work they believe has stopped.

use std::sync::Mutex;

use pearl_agent::Agent;
use pearl_core::Queue;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::{Role, SessionStore};
use rstest::rstest;
use tempfile::TempDir;

/// A provider that cancels the task from inside the answer and then keeps
/// asking for tools: a runner that does not read the queue would meet it
/// again, and the count is how the story tells the two apart.
struct Stopper {
    queue: Queue,
    calls: Mutex<usize>,
}

impl Provider for Stopper {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if *calls == 1 {
            let focused = self.queue.focus().expect("a running task is focused");
            self.queue.cancel(focused).unwrap();
        }
        Ok(Completion {
            content: String::new(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                kind: "function".into(),
                function: Function {
                    name: "bash".into(),
                    arguments: "{\"command\":\"true\"}".into(),
                },
            }],
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_running_task_when_the_human_cancels_it_then_the_model_is_not_asked_again() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    let queue = Queue::new();
    let (id, _) = queue.append("a long piece of work");
    let agent = Agent::new(
        queue.clone(),
        Stopper {
            queue: queue.clone(),
            calls: Mutex::new(0),
        },
        store,
        "mock",
    );
    let mut agent = agent;

    agent.run().await.unwrap();

    assert_eq!(
        *agent.provider().calls.lock().unwrap(),
        1,
        "the runner asked the model again after the task was cancelled"
    );
    assert_eq!(
        queue.get(id).unwrap().state,
        pearl_core::NodeState::Cancelled
    );
    let entries = agent.session().read().unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.role == Role::User && entry.content.contains("cancelled this task")),
        "the session does not say the task was cancelled: {entries:?}"
    );
}
