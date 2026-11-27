//! Stories for a row that has spent its rounds without answering.
//!
//! A loop needs a bound: a model that keeps asking for tools would spend the
//! human's money and never settle. What the bound does when it is reached is
//! the story here -- a row that stops where it stands and says how to go on,
//! rather than one that dies with a reason only the log knows.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::SessionStore;
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
            usage: Default::default(),
        })
    }
}

fn agent(dir: &TempDir, queue: &Queue, calls: &Arc<AtomicUsize>) -> Agent<Endless> {
    Agent::new(
        queue.clone(),
        Endless {
            calls: calls.clone(),
        },
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
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
    let mut agent = agent(&dir, &queue, &calls);
    let (id, _) = queue.append("keep going until you are sure it is right");
    agent.run().await.unwrap();

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
async fn given_a_row_stopped_for_want_of_rounds_when_the_human_goes_on_then_it_gets_another_stretch() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut agent = agent(&dir, &queue, &calls);
    let (id, _) = queue.append("keep going until you are sure it is right");
    agent.run().await.unwrap();
    let spent = calls.load(Ordering::SeqCst);
    assert_eq!(queue.get(id).map(|n| n.state), Some(NodeState::Paused));

    queue.run(id).unwrap();
    assert_eq!(queue.next_queued(), Some(id), "going on left the row unrunnable");
    agent.run().await.unwrap();

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
