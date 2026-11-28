//! Stories for the window the human named.
//!
//! `window` on the status line is a promise about money: the work stays inside
//! the context the human said they were paying for. These stories drive a run
//! to that edge and read what the person reads -- the row, and what went on the
//! wire -- because a fold that the estimate believes in and the endpoint does
//! not is a request that costs twice and then fails.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_core::{Queue, Settings};
use pearl_log::{Budget, Role, Store};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{FoldPolicy, Policy, Runner};
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
fn tight() -> FoldPolicy {
    FoldPolicy {
        budget: Budget {
            keep_recent: 3,
            tail_tokens: 200,
            min_fold_tokens: 100,
        },
    }
}

/// Seed a log with `rounds` user/assistant pairs of readable size.
fn seed(dir: &TempDir, bills: usize) -> Store {
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..bills {
        store
            .append(Role::User, format!("round {i}: {}", "x".repeat(400)))
            .unwrap();
        store
            .append(Role::Assistant, format!("reply {i}: {}", "y".repeat(400)))
            .unwrap();
    }
    store
}

/// Seed a night of shell work: messages whose prose is a dozen words and whose
/// tool calls carry the whole command. This is the shape a long task actually
/// has, and the shape a character count of the prose alone cannot see.
fn seed_calls(dir: &TempDir, bills: usize, command: usize) -> Store {
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..bills {
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
        .any(|m| m.content.contains("You are folding"))
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
async fn given_a_prompt_the_endpoint_bills_far_above_its_text_when_the_next_round_starts_then_the_history_is_folded()
 {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), Priced::new(100_000), store, "mock")
        .with_fold(tight())
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    queue.append("append a line to work.txt");

    runner.run().await.unwrap();

    let work = runner.provider().work();
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
        report.contains("folded"),
        "the row never says the history was folded:\n{report}"
    );
}

// A round that asks for six tools carries six arguments, and the endpoint bills
// for every character of them. Counting the prose alone left a fifth of a long
// task's prompt out of the decision, so the first question of a task went out
// over the window with nothing folded -- and no bill yet to correct it.
#[rstest]
#[tokio::test]
async fn given_a_night_whose_tool_calls_outweigh_its_prose_when_a_task_asks_its_first_question_then_the_history_is_folded()
 {
    let dir = TempDir::new().unwrap();
    let store = seed_calls(&dir, 20, 3_000);
    let settings = Settings::new("mock");
    settings.apply("window 20000").unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), Priced::new(15_600), store, "mock")
        .with_fold(tight())
        .with_settings(settings);
    queue.append("what did the night do");

    runner.run().await.unwrap();

    let work = runner.provider().work();
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
        report.contains("folded"),
        "the row never says the history was folded:\n{report}"
    );
}
/// An endpoint that refuses the first question for length and answers the
/// second: the window was fuller than anything local could tell.
struct Overfull {
    asked: AtomicUsize,
}

impl Provider for Overfull {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        if folding(&request) {
            return Ok(Completion::text("SUMMARY: the night so far", "mock"));
        }
        if self.asked.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(ProviderError::Refused {
                status: 400,
                message: "This model's maximum context length is 128000 tokens. \
                            However, your messages resulted in 144988 tokens."
                    .into(),
            });
        }
        Ok(Completion::text("the answer, after the fold", "mock"))
    }
}

// A refusal for length is the endpoint telling us the window is full, which is
// the one thing the fold exists for. Failing the row instead makes the human
// pay for the whole prompt and get nothing, then press space and pay again.
// One fold and one retry: an endpoint that refuses the smaller request too is
// refusing something else.
#[rstest]
#[tokio::test]
async fn given_a_question_refused_for_length_when_the_task_runs_then_it_folds_and_asks_again() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let queue = Queue::new();
    let mut runner = Runner::new(
        queue.clone(),
        Overfull {
            asked: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_fold(tight())
    .with_settings(Settings::new("mock"));
    let (id, _) = queue.append("the question that was too long");

    runner.run().await.unwrap();

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("the answer, after the fold"),
        "the row did not come back with an answer:\n{report}"
    );
    assert_eq!(
        queue.get(id).map(|n| format!("{:?}", n.state)),
        Some("Done".to_string()),
        "the row was left failed rather than answered"
    );
}

// The fold point is a price, not a safety margin. A prompt that runs up to
// three quarters of the window before it is folded spends the whole climb
// billed again every round; the mean of that sawtooth is what the night costs.
// Half the climb is half the bill, and the room left over is still enormous.
#[rstest]
#[tokio::test]
async fn given_a_night_that_bills_half_the_window_when_the_next_task_asks_then_the_history_is_folded()
 {
    let dir = TempDir::new().unwrap();
    // A hundred and twenty shell rounds: the prose is a dozen words and the
    // command is two thousand characters, which is the shape of real work.
    let store = seed_calls(&dir, 120, 2_000);
    let settings = Settings::new("mock");
    let recorder = Priced::new(1_000);
    let mut runner = Runner::new(Queue::new(), recorder, store, "mock").with_settings(settings);
    runner.queue.append("the next task");

    runner.run().await.unwrap();

    let work = runner.provider().work();
    assert!(!work.is_empty(), "the task never asked");
    assert!(
        work[0]
            .messages
            .iter()
            .any(|m| m.content.contains("SUMMARY:") || m.content.contains("[fold")),
        "a night billing half the window was carried whole: {} messages",
        work[0].messages.len()
    );
}

/// A model that bills what it was shown, four characters to the token, and
/// does one round of shell work before it answers. The mean of its bills is
/// the price of the night, and the story is about that mean.
struct ByTheRound {
    rounds: usize,
    asked: Mutex<Vec<u32>>,
    done: AtomicUsize,
}

impl Provider for ByTheRound {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let shown: usize = request.messages.iter().map(|m| m.content.len()).sum();
        let billed = (shown / 4) as u32;
        if folding(&request) {
            let mut done = Completion::text("SUMMARY: the night so far", "mock");
            done.usage = Usage {
                prompt_tokens: billed,
                completion_tokens: 20,
                cached_tokens: 0,
            };
            return Ok(done);
        }
        self.asked.lock().unwrap().push(billed);
        let n = self.done.fetch_add(1, Ordering::SeqCst);
        let mut done = if n < self.rounds {
            Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![pearl_provider::ToolCall {
                    id: format!("c{n}"),
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
            prompt_tokens: billed,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

#[rstest]
#[tokio::test]
async fn given_a_long_task_on_the_default_window_when_its_bills_are_averaged_then_the_mean_is_under_a_fifth_of_it()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("work.txt"), "x".repeat(8_000)).unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let settings = Settings::new("mock");
    let window = settings.window();
    let queue = Queue::new();
    let mut runner = Runner::new(
        queue.clone(),
        ByTheRound {
            rounds: 60,
            asked: Mutex::new(Vec::new()),
            done: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), Policy::WorkspaceWrite)
    .with_settings(settings);
    queue.append("the long task");

    runner.run().await.unwrap();

    let bills = runner.provider().asked.lock().unwrap().clone();
    assert!(bills.len() > 50, "the task stopped early: {bills:?}");
    let mean = bills.iter().sum::<u32>() / bills.len() as u32;
    assert!(
        mean < window / 5,
        "the night averaged {mean} tokens of a {window} window, over a fifth: {bills:?}"
    );
}

// The shape a real task has: one thing the human asked and forty rounds of
// tools behind it. No second user message ever arrives to be a seam, so a fold
// that waits for one waits for the end of the task. Watched on real keys: a
// window of 8000 folding at 3000, and a prompt that climbed to 12,526 without
// a single fold.
#[rstest]
#[tokio::test]
async fn given_one_question_and_many_rounds_of_tools_when_the_prompt_passes_the_fold_point_then_the_history_folds()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("work.txt"), "x".repeat(7_500)).unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    // The ground sentence is what every real launch writes first, and it is
    // load-bearing here: it puts a System entry ahead of the one user message.
    pearl::ground(&store, dir.path(), Policy::WorkspaceWrite).unwrap();
    let settings = Settings::new("mock");
    settings.apply("window 8000").unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(
        queue.clone(),
        ByTheRound {
            rounds: 8,
            asked: Mutex::new(Vec::new()),
            done: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), Policy::WorkspaceWrite)
    .with_settings(settings);
    queue.append("read the file until you are sure");

    runner.run().await.unwrap();

    // The first round is small whatever the fold does, so a story about the
    // smallest bill proves nothing. What the fold leaves is a page in the log
    // and a line the human reads.
    let store = runner.log();
    let entries = store.read_view(None, &Default::default()).unwrap();
    assert!(
        entries.iter().any(|e| e.role == Role::Fold),
        "a task of one question and eight rounds never folded: {:?}",
        runner.provider().asked.lock().unwrap()
    );
    let app = pearl_tui::App::new(queue.clone());
    let shown = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        shown.contains("folded"),
        "the report does not say the history was folded:\n{shown}"
    );
}
