//! Stories for what the model is handed about the rest of the list.
//!
//! A topic keeps its turns to itself, so a task in another line of work would
//! otherwise see nothing of it. It sees the rows instead: what is done, one
//! sentence each, and what still waits.

use std::sync::Mutex;

use pearl_agent::Agent;
use pearl_core::Queue;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::{Role, SessionStore};
use rstest::rstest;
use tempfile::TempDir;

/// A provider that never uses a tool and remembers every request it saw.
struct Recorder {
    requested: Mutex<Vec<CompletionRequest>>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            requested: Mutex::new(Vec::new()),
        }
    }

    fn last_messages(&self) -> Vec<String> {
        self.requested
            .lock()
            .unwrap()
            .last()
            .map(|r| r.messages.iter().map(|m| m.content.clone()).collect())
            .unwrap_or_default()
    }
}

impl Provider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.requested.lock().unwrap().push(request);
        Ok(Completion {
            content: "the task is done".into(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 10,
                cached_tokens: 0,
            },
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_work_done_in_another_topic_when_a_task_runs_then_it_reads_the_rows_not_the_talk() {
    let dir = TempDir::new().unwrap();
    let mut store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    let queue = Queue::new();

    // One line of work, finished, with words of its own.
    let (topic, _) = queue.topic("parser").unwrap();
    let (lexer, _) = queue.add_child(topic, "write the lexer").unwrap();
    queue.start(lexer).unwrap();
    queue.complete(lexer, "tokens come out").unwrap();
    store.in_topic(Some(topic.to_string()));
    store
        .append(Role::User, "SECRET-OF-PARSER: a long dead end")
        .unwrap();
    store
        .append(Role::Assistant, "SECRET-OF-PARSER: how it was reached")
        .unwrap();

    // Another line of work, about to ask the model something.
    store.in_topic(None);
    let (other, _) = queue.topic("docs").unwrap();
    queue.add_child(other, "write the guide").unwrap();

    let recorder = Recorder::new();
    let mut agent = Agent::new(queue, recorder, store, "mock");
    agent.run().await.unwrap();

    let shown = agent.provider().last_messages().join("\n");
    assert!(
        shown.contains("▸ parser (1/1)"),
        "the finished line is not named: {shown}"
    );
    assert!(
        shown.contains("tokens come out"),
        "its conclusion is not in the digest: {shown}"
    );
    assert!(
        shown.contains("write the guide"),
        "the row in flight's level is missing: {shown}"
    );
    assert!(
        !shown.contains("SECRET-OF-PARSER"),
        "another topic's talk reached the wire: {shown}"
    );
}
