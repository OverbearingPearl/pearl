//! Stories for what a round's bill says about the round.
//!
//! Every request a model answers re-reads the whole conversation, so the figure
//! the provider bills as "prompt tokens" is the size of the conversation that
//! round read, not the words that round added. A row of bills that climbs --
//! `↑864`, `↑3.4k`, `↑6.5k`, `↑10.7k` -- is the conversation growing, and a
//! human reading it as "this round used ten thousand tokens" is reading a bill
//! that does not say that. The bill says both: what it added, and what it read.

use pearl_core::{Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{Policy, Runner};
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
    let store = Store::open(dir.path().join("pearl-log.jsonl")).unwrap();
    let mut runner = Runner::new(
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

    runner.run().await.unwrap();

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
    // What a round added is not money, so it may not be what the row totals.
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
    let store = Store::open(dir.path().join("pearl-log.jsonl")).unwrap();
    let mut runner = Runner::new(
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

    runner.run().await.unwrap();

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

/// A model whose prompt the cache mostly served: eight thousand of the ten
/// thousand tokens it was billed for came back from the endpoint's cache.
struct Served;

impl Provider for Served {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let mut done = Completion::text("the answer is written", "mock");
        done.usage = Usage {
            prompt_tokens: 10_000,
            completion_tokens: 50,
            cached_tokens: 8_000,
        };
        Ok(done)
    }
}

// What the cache served is the difference between a bill that looks ruinous
// and one that is not, and it is a number the provider reports. A reading that
// lives only in memory is a reading the next window does not have: the human
// who comes back to a directory has to be able to see what the work cost.
#[rstest]
#[tokio::test]
async fn given_a_round_the_cache_mostly_served_when_the_window_is_reopened_then_the_row_still_says_how_much()
 {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    let queue = Queue::new();
    {
        let store = Store::open(&path).unwrap();
        let mut runner =
            Runner::new(queue.clone(), Served, store, "mock").with_settings(Settings::new("mock"));
        runner.queue.append("write the answer");
        runner.run().await.unwrap();
        runner
            .log()
            .clone()
            .append_queue(runner.queue.snapshot())
            .unwrap();
    }

    let store = pearl::log(&path, true).unwrap();
    let back = Queue::new();
    pearl::restore_queue(&store, &back).unwrap();
    let app = pearl_tui::App::new(back);
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("cache 80%"),
        "the reopened row does not say what the cache served:\n{report}"
    );
}

/// A model that bills a fold differently from the work, so the two are told
/// apart on the row: nine thousand tokens to write the summary, one thousand
/// to answer the task.
struct Folder;

impl Provider for Folder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let folding = request
            .messages
            .iter()
            .any(|m| m.content.contains("You are folding"));
        let (content, prompt, down) = if folding {
            ("SUMMARY: the night so far", 9_000, 300)
        } else {
            ("the task is done", 1_000, 10)
        };
        let mut done = Completion::text(content, "mock");
        done.usage = Usage {
            prompt_tokens: prompt,
            completion_tokens: down,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

// A fold is a request like any other, and it is the largest one a long task
// makes: it reads the whole history to write a page. A row that says the
// history was folded without saying what that cost is a row the human
// cannot read the price of -- and the notice alone looks free.
#[rstest]
#[tokio::test]
async fn given_a_history_folded_to_fit_the_window_when_the_report_is_read_then_the_fold_has_a_bill()
{
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..20 {
        store
            .append(
                pearl_log::Role::User,
                format!("round {i}: {}", "x".repeat(400)),
            )
            .unwrap();
        store
            .append(
                pearl_log::Role::Assistant,
                format!("reply {i}: {}", "y".repeat(400)),
            )
            .unwrap();
    }
    let settings = Settings::new("mock");
    settings.apply("window 1000").unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), Folder, store, "mock")
        .with_settings(settings)
        .with_fold(pearl_runner::FoldPolicy {
            budget: pearl_log::Budget {
                keep_recent: 3,
                tail_tokens: 200,
                min_fold_tokens: 100,
            },
        });
    let (id, _) = queue.append("the next task");
    runner.run().await.unwrap();

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("fold"),
        "the report does not name the fold as a round:\n{report}"
    );
    assert!(
        report.contains("\u{2191}9.0k"),
        "the fold's own bill is not on the row:\n{report}"
    );
    assert_eq!(
        queue.get(id).map(|n| n.cost.up),
        Some(10_000),
        "the row's total leaves out what the fold cost"
    );
}

/// A model that bills what it was actually shown, at four characters to the
/// token, the way an endpoint does. The price of a fold is then a fact about
/// the transcript rather than a number the story chose.
struct ByWeight;

impl Provider for ByWeight {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let shown: usize = request.messages.iter().map(|m| m.content.len()).sum();
        let folding = request
            .messages
            .iter()
            .any(|m| m.content.contains("You are folding"));
        let mut done = Completion::text(
            if folding {
                "SUMMARY: the night so far"
            } else {
                "the task is done"
            },
            "mock",
        );
        done.usage = Usage {
            prompt_tokens: (shown / 4) as u32,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

// A fold reads the history to write a page, and the history of a night of
// shell work is mostly output. Paying the model to read eighty thousand
// characters of `x` in order to decide it does not need them is the cost the
// fold exists to remove, and the human sees it as the price of the one round
// named `fold`.
#[rstest]
#[tokio::test]
async fn given_a_history_of_long_shell_output_when_the_fold_is_billed_then_it_paid_for_the_ends_and_not_the_middle()
 {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
    for i in 0..12 {
        store
            .append_asked(
                json!([{"id": format!("c{i}"), "type": "function",
                        "function": {"name": "bash", "arguments": "{}"}}])
                .to_string(),
                format!("calling round {i}"),
            )
            .unwrap();
        store
            .append_answered(
                format!("c{i}"),
                format!(
                    "[op {i}] cargo test\n{}\nerror: test failed, to rerun pass `--lib`",
                    "x".repeat(8_000)
                ),
            )
            .unwrap();
    }
    let settings = Settings::new("mock");
    settings.apply("window 1000").unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), ByWeight, store, "mock")
        .with_settings(settings)
        .with_fold(pearl_runner::FoldPolicy {
            budget: pearl_log::Budget {
                keep_recent: 3,
                tail_tokens: 200,
                min_fold_tokens: 100,
            },
        });
    let (id, _) = queue.append("the next task");
    runner.run().await.unwrap();

    let row = queue.get(id).unwrap();
    let fold = row
        .bills
        .iter()
        .find(|t| t.note == "fold")
        .expect("the fold left no bill on the row");
    // The history is ninety-six thousand characters of output. A fold that
    // read it whole bills above twenty-four thousand tokens; the two ends of
    // twelve outputs and the rules are a quarter of that.
    assert!(
        fold.cost.up < 8_000,
        "the fold paid {} tokens to shorten a history it should not have read",
        fold.cost.up
    );
    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("fold"),
        "the report does not name the fold as a round:\n{report}"
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
        pearl_core::Bill::doing(
            pearl_core::Cost::round(1_000, 20, 2_000),
            String::from("bash cargo test --all --quiet --tests -- --exact goes_on_and_on"),
        ),
    );

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 40));
    let row = report
        .lines()
        .find(|line| line.starts_with("round 1"))
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
        pearl_core::Bill::doing(
            pearl_core::Cost::round(1_000, 20, 300),
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
