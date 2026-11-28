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

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_core::{Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{Policy, Runner};
use rstest::rstest;
use tempfile::TempDir;

/// The shell's own way of printing a file. `cat` on Windows is an MSYS
/// program, and the sandbox cannot start one (ADR 19), so a story that asks
/// for it there measures the cage instead of the feature it names.
fn read_work() -> String {
    let verb = if cfg!(windows) { "type" } else { "cat" };
    format!(r#"{{"command":"{verb} work.txt"}}"#)
}

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
                        arguments: read_work().into(),
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
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let mut runner = Runner::new(Queue::new(), Printer::new(40), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    runner.queue.append("read work.txt until you are sure");

    runner.run().await.unwrap();

    let asked = runner.provider().asked.lock().unwrap();
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
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let mut runner = Runner::new(Queue::new(), Printer::new(6), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    runner.queue.append("read work.txt until you are sure");
    // A row the task in flight does not own, so the digest has something to
    // say: a lone task's digest is empty, and an empty one is not sent.
    runner.queue.append("later: the other row's work");

    runner.run().await.unwrap();

    let asked = runner.provider().asked.lock().unwrap();
    let mut deepest = 0;
    for request in asked.iter() {
        let carried: Vec<usize> = request
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.content.contains("The queue, as it stands"))
            .map(|(i, _)| i)
            .collect();
        assert!(
            carried.len() <= 1,
            "one request carried the digest {} times",
            carried.len()
        );
        // The history it inherited comes first, so the digest is never the
        // message a request opens with.
        assert!(
            carried.first() != Some(&0),
            "the digest is the first message of a request"
        );
        // Everything after the digest is this task's own work, appended round
        // by round: the deepest request is the one the digest was rebuilt in
        // most often.
        for at in carried {
            deepest = deepest.max(request.messages.len() - at - 1);
        }
    }
    assert!(
        deepest > 6,
        "the digest sits at the end of the work rather than before it"
    );
}
