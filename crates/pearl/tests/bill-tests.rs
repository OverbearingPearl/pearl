//! Stories for what a round's bill says about the round.
//!
//! Every request a model answers re-reads the whole conversation, so the figure
//! the provider bills as "prompt tokens" is the size of the conversation that
//! round read, not the words that round added. A row of turns that climbs --
//! `↑864`, `↑3.4k`, `↑6.5k`, `↑10.7k` -- is the conversation growing, and a
//! human reading it as "this round used ten thousand tokens" is reading a bill
//! that does not say that. The turn says both: what it added, and what it read.

use pearl_agent::{Agent, Policy};
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::SessionStore;
use rstest::rstest;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

/// A model that spends two rounds on tools and then answers, billing a
/// conversation that grows by five hundred tokens a round.
struct Grower {
    asked: AtomicUsize,
}

impl Provider for Grower {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let round = self.asked.fetch_add(1, Ordering::SeqCst);
        let (prompt, completion) = match round {
            0 => (1_000, 10),
            1 => (1_500, 20),
            _ => (2_000, 30),
        };
        let call = match round {
            0 => Some("echo one >> work.txt"),
            1 => Some("echo two >> work.txt"),
            _ => None,
        };
        Ok(Completion {
            content: if call.is_some() {
                String::new()
            } else {
                String::from("both lines are in work.txt")
            },
            reasoning: None,
            model: String::from("mock"),
            tool_calls: call
                .map(|command| {
                    vec![pearl_provider::ToolCall {
                        id: String::from("c1"),
                        kind: String::from("function"),
                        function: pearl_provider::Function {
                            name: String::from("bash"),
                            arguments: json!({"command": command}).to_string(),
                        },
                    }]
                })
                .unwrap_or_default(),
            usage: Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                cached_tokens: 0,
            },
        })
    }
}

/// What the report window shows for the focused row, as one string.
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

#[rstest]
#[tokio::test]
async fn given_a_round_that_read_more_than_it_added_when_the_report_is_read_then_it_says_both() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(
        queue.clone(),
        Grower {
            asked: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (id, _) = queue.append("write two lines");

    agent.run().await.unwrap();

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));

    // The second round read a conversation of 1.5k tokens and added five
    // hundred of them. Saying `↑1.5k` alone tells the human it added all of
    // them, which is the mistake this story is about.
    assert!(
        report.contains("↑500 of 1.5k"),
        "the second round does not say what it added:\n{report}"
    );
    assert!(
        report.contains("↑500 of 2.0k"),
        "the last round does not say what it added:\n{report}"
    );
    // The row still bills what was paid for: three rounds of prompts, summed.
    // What a turn added is not money, so it may not be what the row totals.
    let paid = queue.get(id).map(|n| n.cost.up).unwrap_or_default();
    assert_eq!(paid, 4_500, "the row stopped billing the money");
}

#[rstest]
#[tokio::test]
async fn given_a_first_round_when_the_report_is_read_then_it_does_not_say_the_same_number_twice() {
    // The first round of a task adds everything it read, so there is nothing to
    // compare and `↑1.0k of 1.0k` is a line that says one thing twice.
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(
        queue.clone(),
        Grower {
            asked: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (_id, _) = queue.append("write two lines");

    agent.run().await.unwrap();

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        !report.contains("of 1.0k"),
        "the first round compares the prompt with itself:\n{report}"
    );
    assert!(
        report.contains("↑1.0k ↓10"),
        "the first round lost its bill:\n{report}"
    );
}

#[rstest]
fn given_a_turn_whose_row_is_narrow_when_the_report_is_read_then_the_note_keeps_its_end() {
    // A row has room for the shape of one command and not for all of it. The
    // end of a command is what it did, so the middle is what a narrow row may
    // drop: a line that keeps only the start keeps the program and loses the
    // work, which is how `bash cd /Users/…/pearl && timeout 900 cargo` came to
    // be a row that named nothing.
    let queue = Queue::new();
    let (id, _) = queue.append("run the long one");
    queue.start(id).unwrap();
    queue.charge(
        id,
        pearl_core::Turn::doing(
            pearl_core::Cost::turn(1_000, 20, 2_000),
            String::from("bash cargo test --all --quiet --tests -- --exact goes_on_and_on"),
        ),
    );

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 40));
    let row = report
        .lines()
        .find(|line| line.starts_with("turn 1"))
        .unwrap_or_default();
    assert!(
        row.contains("and_on"),
        "the narrow row dropped the end of the command, which is what it did: {row}"
    );
    assert!(
        row.contains('\u{2026}'),
        "the row cut the note without saying it had: {row}"
    );
}

#[rstest]
fn given_a_read_of_a_range_when_the_report_is_read_then_the_row_says_which_range() {
    // `read src/x.rs` and `read src/x.rs:1200-1400` are different acts, and the
    // row is the only place the human sees which part of a file was looked at.
    let queue = Queue::new();
    let (id, _) = queue.append("look at the middle");
    queue.start(id).unwrap();
    queue.charge(
        id,
        pearl_core::Turn::doing(
            pearl_core::Cost::turn(1_000, 20, 300),
            String::from("read crates/pearl-tui/src/app.rs:1200-1399"),
        ),
    );

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));

    assert!(
        report.contains("app.rs:1200-1399"),
        "the row does not say which part was read:\n{report}"
    );
}
