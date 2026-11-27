//! The prefix the endpoint's cache matches, and what moves it.
//!
//! A house that caches automatically -- DeepSeek, OpenAI -- matches the request
//! from its first byte and stops at the first difference. Everything before
//! that difference is billed at a tenth; everything after it is billed whole.
//! So the price of a long task is decided less by how much is sent than by how
//! much of what is sent stayed where it was last round.
//!
//! These stories compare consecutive requests as bytes, because that is what
//! the cache compares. A story about message counts cannot fail when a field
//! moves.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use pearl_agent::{Agent, Policy};
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

/// A night of shell work: `rounds` rounds of `cat`, then an answer, recording
/// every request it was asked.
struct Printer {
    rounds: usize,
    asked: Mutex<Vec<CompletionRequest>>,
    done: AtomicUsize,
}

impl Printer {
    fn new(rounds: usize) -> Self {
        Self {
            rounds,
            asked: Mutex::new(Vec::new()),
            done: AtomicUsize::new(0),
        }
    }
}

impl Provider for Printer {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.asked.lock().unwrap().push(request);
        let mut done = if self.done.fetch_add(1, Ordering::SeqCst) < self.rounds {
            Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![pearl_provider::ToolCall {
                    id: "1".into(),
                    kind: "function".into(),
                    function: pearl_provider::Function {
                        name: "bash".into(),
                        arguments: r#"{"command":"cat work.txt"}"#.into(),
                    },
                }],
                usage: Usage::default(),
            }
        } else {
            Completion::text("the task is done", "mock")
        };
        done.usage = Usage {
            prompt_tokens: 1_000,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

/// The bytes of one message, as they go on the wire.
fn bytes(m: &pearl_provider::ChatMessage) -> String {
    serde_json::to_string(m).unwrap()
}

#[rstest]
#[tokio::test]
async fn given_a_task_of_many_rounds_when_each_request_is_compared_with_the_first_then_the_history_it_inherited_never_moves()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("work.txt"), "x".repeat(400)).unwrap();
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    let mut agent = Agent::new(Queue::new(), Printer::new(40), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    agent.queue.append("read work.txt until you are sure");

    agent.run().await.unwrap();

    let asked = agent.provider().asked.lock().unwrap();
    assert!(asked.len() > 30, "the task stopped early: {}", asked.len());
    let first: Vec<String> = asked[0].messages.iter().map(bytes).collect();
    for (round, request) in asked.iter().enumerate().skip(1) {
        let now: Vec<String> = request.messages.iter().map(bytes).collect();
        assert!(
            now.len() >= first.len(),
            "round {round} sent fewer messages than the first"
        );
        for (i, was) in first.iter().enumerate() {
            assert_eq!(
                &now[i], was,
                "round {round} moved message {i} of the history it inherited"
            );
        }
    }
}

// The queue digest is the one part of the prompt that is rebuilt rather than
// read from the log. Putting it anywhere but the end would move the whole
// history behind it every time a row changed state, which is every round.
#[rstest]
#[tokio::test]
async fn given_a_task_whose_queue_changes_as_it_runs_when_a_request_is_read_then_the_digest_is_not_a_prefix_of_the_work()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("work.txt"), "x".repeat(400)).unwrap();
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    let mut agent = Agent::new(Queue::new(), Printer::new(6), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    agent.queue.append("read work.txt until you are sure");

    agent.run().await.unwrap();

    let asked = agent.provider().asked.lock().unwrap();
    let last = asked.last().unwrap();
    let digest = last
        .messages
        .iter()
        .position(|m| m.content.contains("The queue, as it stands"))
        .expect("no request carried the queue digest");
    // Everything after the digest is this task's own work, appended round by
    // round. Nothing before it is the digest of a later round.
    let later = last.messages[digest + 1..]
        .iter()
        .filter(|m| m.content.contains("The queue, as it stands"))
        .count();
    assert_eq!(later, 0, "the digest was sent again after the first one");
    assert!(
        last.messages.len() - digest > 6,
        "the digest sits at the end of the work rather than before it"
    );
}
