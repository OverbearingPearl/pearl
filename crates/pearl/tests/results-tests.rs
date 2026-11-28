//! Stories for what a long task carries: the newest tool outputs whole, the
//! older ones a line, and the human told that is what happened.
//!
//! A night of shell work is mostly output. On the log this was read from, tool
//! results were four fifths of every request and the model's own answers were
//! under a fiftieth. The human pays for the same `grep` forty times because
//! each round re-reads the whole conversation -- and the fortieth reading of
//! the first `grep` is worth nothing that the first reading was not.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_core::{Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{Policy, Runner};
use rstest::rstest;
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

/// A night of reading the same file: `rounds` rounds of `cat`, then an answer.
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

    fn last(&self) -> CompletionRequest {
        self.asked.lock().unwrap().last().unwrap().clone()
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
            Completion::text("the file is the same every time", "mock")
        };
        done.usage = Usage {
            prompt_tokens: 1_000,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

// Thirty readings of one file. The newest few are the work in hand; the rest
// are a line saying how much was there, and the person is told so rather than
// left to wonder why the agent stopped quoting its first command.
#[rstest]
#[tokio::test]
async fn given_a_night_of_reading_one_file_when_the_report_is_read_then_the_older_outputs_are_a_line_and_the_row_says_so()
 {
    let dir = TempDir::new().unwrap();
    let body = "x".repeat(2_000);
    std::fs::write(dir.path().join("work.txt"), format!("{body}\n")).unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), Printer::new(30), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    queue.append("read work.txt until you are sure");

    runner.run().await.unwrap();

    let last = runner.provider().last();
    let outputs: Vec<&str> = last
        .messages
        .iter()
        .filter(|m| m.tool_call_id.is_some())
        .map(|m| m.content.as_str())
        .collect();
    assert!(
        outputs.len() >= 30,
        "the night did not happen: {}",
        outputs.len()
    );
    assert!(
        outputs[0].starts_with("[folded:"),
        "the first output was carried whole on the last request: {}",
        outputs[0]
    );
    assert!(
        outputs.last().unwrap().contains(&body),
        "the newest output is the work in hand and must be whole"
    );
    let app = pearl_tui::App::new(queue);
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("the log keeps them whole"),
        "the row never says what became of the older outputs:\n{report}"
    );
}
