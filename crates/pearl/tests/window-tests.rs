//! Stories for the window the human named.
//!
//! `window` on the status line is a promise about money: the work stays inside
//! the context the human said they were paying for. These stories drive a run
//! to that edge and read what the person reads -- the row, and what went on the
//! wire -- because a fold that the estimate believes in and the endpoint does
//! not is a request that costs twice and then fails.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use pearl_agent::{Agent, CompactionPolicy, Policy};
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::{Budget, Role, SessionStore};
use rstest::rstest;
use serde_json::json;
use tempfile::TempDir;

/// The rows as text, the way the pane would read them.
macro_rules! rows_text {
    ($rows:expr) => {
        $rows
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    };
}

/// A budget that lets a short seeded history exceed the tail.
fn tight() -> CompactionPolicy {
    CompactionPolicy {
        budget: Budget {
            keep_recent: 3,
            tail_tokens: 200,
            min_fold_tokens: 100,
        },
    }
}

/// Seed a log with `turns` user/assistant pairs of readable size.
fn seed(dir: &TempDir, turns: usize) -> SessionStore {
    let mut store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    for i in 0..turns {
        store
            .append(Role::User, format!("turn {i}: {}", "x".repeat(400)))
            .unwrap();
        store
            .append(Role::Assistant, format!("reply {i}: {}", "y".repeat(400)))
            .unwrap();
    }
    store
}

/// Seed a night of shell work: turns whose prose is a dozen words and whose
/// tool calls carry the whole command. This is the shape a long task actually
/// has, and the shape a character count of the prose alone cannot see.
fn seed_calls(dir: &TempDir, turns: usize, command: usize) -> SessionStore {
    let mut store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    for i in 0..turns {
        let calls = json!([{
            "id": format!("c{i}"),
            "type": "function",
            "function": {
                "name": "bash",
                "arguments": json!({ "command": "x".repeat(command) }).to_string(),
            },
        }]);
        store
            .append_asked(calls.to_string(), "[calling 1 tool(s)]".to_string())
            .unwrap();
        store.append_answered(format!("c{i}"), "ok").unwrap();
    }
    store
}

/// One round of shell work, then the answer, at a prompt price the story sets.
/// The endpoint's count is the story's to state: what a character estimate
/// guesses and what the endpoint bills are the whole question here.
struct Priced {
    prompt: u32,
    asked: Mutex<Vec<CompletionRequest>>,
    rounds: AtomicUsize,
}

impl Priced {
    fn new(prompt: u32) -> Self {
        Self {
            prompt,
            asked: Mutex::new(Vec::new()),
            rounds: AtomicUsize::new(0),
        }
    }

    /// The requests that were work rather than a fold, in order.
    fn work(&self) -> Vec<CompletionRequest> {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|r| !folding(r))
            .cloned()
            .collect()
    }
}

/// Whether a request is the summarizer's rather than the task's.
fn folding(request: &CompletionRequest) -> bool {
    request
        .messages
        .iter()
        .any(|m| m.content.contains("You are compacting"))
}

impl Provider for Priced {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let fold = folding(&request);
        self.asked.lock().unwrap().push(request);
        let mut done = if fold {
            Completion::text("SUMMARY: the night so far", "mock")
        } else if self.rounds.fetch_add(1, Ordering::SeqCst) == 0 {
            Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![pearl_provider::ToolCall {
                    id: "1".into(),
                    kind: "function".into(),
                    function: pearl_provider::Function {
                        name: "bash".into(),
                        arguments: r#"{"command":"echo one more line >> work.txt"}"#.into(),
                    },
                }],
                usage: Usage::default(),
            }
        } else {
            Completion::text("the line is appended", "mock")
        };
        done.usage = Usage {
            prompt_tokens: self.prompt,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

// The endpoint counts the framing, the tool schemas and the calls it was
// billed for; a character count of the prose knows none of them. On the night
// this was read from, the estimate said 38k and the endpoint billed 145k --
// past a 128k window, with the fold never called. The number the endpoint
// already told us is the one to decide by.
#[rstest]
#[tokio::test]
async fn given_a_prompt_the_endpoint_bills_far_above_its_text_when_the_next_round_starts_then_the_history_is_folded(
) {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Priced::new(100_000), store, "mock")
        .with_compaction(tight())
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    queue.append("append a line to work.txt");

    agent.run().await.unwrap();

    let work = agent.provider().work();
    assert!(work.len() >= 2, "the task did not reach a second round");
    assert!(
        work[1].messages.len() < work[0].messages.len(),
        "the second request was not smaller: {} then {}",
        work[0].messages.len(),
        work[1].messages.len()
    );
    let app = pearl_tui::App::new(queue);
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("summarized"),
        "the row never says the history was folded:\n{report}"
    );
}

// A turn that asks for six tools carries six arguments, and the endpoint bills
// for every character of them. Counting the prose alone left a fifth of a long
// task's prompt out of the decision, so the first question of a task went out
// over the window with nothing folded -- and no bill yet to correct it.
#[rstest]
#[tokio::test]
async fn given_a_night_whose_tool_calls_outweigh_its_prose_when_a_task_asks_its_first_question_then_the_history_is_folded(
) {
    let dir = TempDir::new().unwrap();
    let store = seed_calls(&dir, 20, 3_000);
    let settings = Settings::new("mock");
    settings.apply("window 20000").unwrap();
    let queue = Queue::new();
    let mut agent = Agent::new(queue.clone(), Priced::new(15_600), store, "mock")
        .with_compaction(tight())
        .with_settings(settings);
    queue.append("what did the night do");

    agent.run().await.unwrap();

    let work = agent.provider().work();
    assert!(!work.is_empty(), "the task never asked");
    assert!(
        work[0]
            .messages
            .iter()
            .any(|m| m.content.starts_with("SUMMARY:")),
        "the first question went out with the whole night behind it: {} messages",
        work[0].messages.len()
    );
    let app = pearl_tui::App::new(queue);
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("summarized"),
        "the row never says the history was folded:\n{report}"
    );
}
