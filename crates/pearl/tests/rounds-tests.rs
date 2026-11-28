//! Stories for a row that has spent its rounds without answering.
//!
//! A loop needs a bound: a model that keeps asking for tools would spend the
//! human's money and never settle. What the bound does when it is reached is
//! the story here -- a row that stops where it stands and says how to go on,
//! rather than one that dies with a reason only the log knows.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_core::{NodeState, Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_runner::Runner;
use pearl_tools::sandbox::Policy;
use rstest::rstest;
use tempfile::TempDir;

/// A model that never answers: every request asks for the same shell command.
struct Endless {
    calls: Arc<AtomicUsize>,
}

impl Provider for Endless {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Completion {
            content: String::new(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![ToolCall {
                id: "1".into(),
                kind: "function".into(),
                function: Function {
                    name: "bash".into(),
                    arguments: r#"{"command":"echo still-working"}"#.into(),
                },
            }],
            usage: pearl_provider::Usage {
                prompt_tokens: 1_000,
                completion_tokens: 10,
                cached_tokens: 0,
            },
        })
    }
}

fn runner(dir: &TempDir, queue: &Queue, calls: &Arc<AtomicUsize>) -> Runner<Endless> {
    Runner::new(
        queue.clone(),
        Endless {
            calls: calls.clone(),
        },
        Store::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite)
}

/// What the report window shows for the focused row, as one string.
fn report(app: &pearl_tui::App) -> String {
    pearl_tui::report_lines(app, 80)
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect::<Vec<String>>()
        .join("\n")
}

#[rstest]
#[tokio::test]
async fn given_a_row_that_kept_asking_for_tools_when_its_rounds_run_out_then_it_stops_and_says_how_to_go_on()
 {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runner = runner(&dir, &queue, &calls);
    let (id, _) = queue.append("keep going until you are sure it is right");
    runner.run().await.unwrap();

    let node = queue.get(id).expect("the row is gone");
    assert_eq!(
        node.state,
        NodeState::Paused,
        "a row out of rounds died instead of stopping: {:?}",
        node.error
    );
    assert!(node.error.is_none(), "the row was failed: {:?}", node.error);
    let said = node.activity.clone().unwrap_or_default();
    assert!(
        said.contains("64"),
        "the row does not say how many rounds it spent: {said:?}"
    );
    assert!(
        said.to_lowercase().contains("space"),
        "the row does not say which key goes on: {said:?}"
    );

    queue.focus_on(id).unwrap();
    let app = pearl_tui::App::new(queue.clone());
    let shown = report(&app);
    assert!(
        shown.contains("64") && shown.to_lowercase().contains("space"),
        "the window does not show the reason: {shown:?}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_row_stopped_for_want_of_rounds_when_the_human_goes_on_then_it_gets_another_stretch()
 {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runner = runner(&dir, &queue, &calls);
    let (id, _) = queue.append("keep going until you are sure it is right");
    runner.run().await.unwrap();
    let spent = calls.load(Ordering::SeqCst);
    assert_eq!(queue.get(id).map(|n| n.state), Some(NodeState::Paused));

    queue.run(id).unwrap();
    assert_eq!(
        queue.next_queued(),
        Some(id),
        "going on left the row unrunnable"
    );
    runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        spent * 2,
        "going on did not buy the row another stretch of the same length"
    );
    assert_eq!(
        queue.get(id).map(|n| n.state),
        Some(NodeState::Paused),
        "the second stretch did not stop where the first one did"
    );
}

// Going on buys another stretch of rounds, not a second copy of the bill. The
// log is what a reopened window reads the row's cost from, so a round written
// twice is a row that costs twice what it cost -- and that number is what the
// human reads while deciding whether to go on again.
#[rstest]
#[tokio::test]
async fn given_a_row_that_went_on_after_its_rounds_ran_out_when_the_window_is_reopened_then_each_round_is_billed_once()
 {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut runner = runner(&dir, &queue, &calls);
    let (id, _) = queue.append("keep going until you are sure it is right");
    runner.run().await.unwrap();
    queue.run(id).unwrap();
    runner.run().await.unwrap();
    let spent = calls.load(Ordering::SeqCst);
    assert_eq!(spent, 128, "the two stretches are not the same length");
    runner
        .log()
        .clone()
        .append_queue(runner.queue.snapshot())
        .unwrap();
    drop(runner);

    let store = pearl::log(&dir.path().join("s.jsonl"), true).unwrap();
    let back = Queue::new();
    pearl::restore_queue(&store, &back).unwrap();
    let row = back.get(id).expect("the row did not come back");
    assert_eq!(
        row.bills.len(),
        spent,
        "the log holds a round more than once"
    );
    assert_eq!(
        row.cost.up,
        1_000 * spent as u32,
        "the reopened row costs more than the work cost"
    );
}

// Sixty-four rounds was chosen when the round count was the only thing standing
// between a task and a full window. Folding bounds the window now, so the count
// is about the human's patience and the bill rather than the prompt -- and a
// number the human cannot name is a wall. Three tasks in one measured night
// stopped at exactly sixty-four and had to be handed another stretch by hand.
#[rstest]
#[tokio::test]
async fn given_a_stretch_the_human_named_when_the_rounds_run_out_then_the_row_names_that_number() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let settings = Settings::new("mock");
    settings.apply("rounds 4").unwrap();
    let mut runner = Runner::new(
        queue.clone(),
        Endless {
            calls: calls.clone(),
        },
        Store::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    )
    .with_settings(settings)
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (id, _) = queue.append("a task that will not settle");

    runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "the stretch was not the one named"
    );
    let row = queue.get(id).unwrap();
    assert_eq!(row.state, NodeState::Paused);
    let said = row.activity.clone().unwrap_or_default();
    assert!(
        said.contains("4 rounds") && said.contains("another 4"),
        "the row does not name the stretch the human set: {said}"
    );
    queue.focus_on(id).unwrap();
    let shown = report(&pearl_tui::App::new(queue.clone()));
    assert!(
        shown.contains("4 rounds") && shown.contains("another 4"),
        "the window does not show the stretch the human set: {shown}"
    );
}
