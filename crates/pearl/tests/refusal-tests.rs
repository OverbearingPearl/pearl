//! Stories for a refusal reaching the screen, not only the model.
//!
//! A tool that refuses in this process has no OS error to pass on, so the
//! notice is the whole of what the human gets: the row says `Done` either way,
//! and a write that never happened reads exactly like one that did.

use pearl_agent::Agent;
use pearl_core::Queue;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::SessionStore;
use rstest::rstest;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

/// A model that writes to the path it was given and then says it is done.
struct Writer {
    calls: AtomicUsize,
    args: String,
}

impl Provider for Writer {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    kind: "function".into(),
                    function: Function {
                        name: "write".into(),
                        arguments: self.args.clone(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion::text("written", "mock"))
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
async fn given_a_write_named_by_an_absolute_path_when_the_report_is_read_then_it_says_why() {
    let dir = TempDir::new().unwrap();
    let inside = dir.path().join("created.txt");
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(
        queue.clone(),
        Writer {
            calls: AtomicUsize::new(0),
            args: serde_json::json!({ "path": inside.display().to_string(), "content": "hello" })
                .to_string(),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = queue.append("write the file");

    agent.run().await.unwrap();

    // The file is inside the workspace, but the tools take paths from its root
    // and a path written from the root of the filesystem is not one of them.
    // The model is told; the human has to be told as well, or the row reads
    // like it wrote the file it never wrote.
    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("absolute path"),
        "the report does not say why the write was refused:\n{report}"
    );
    assert!(
        !inside.exists(),
        "the write landed after all, so the story is about nothing"
    );
    assert_eq!(queue.focus(), Some(id));
}

#[rstest]
#[tokio::test]
async fn given_a_write_that_climbs_out_of_the_workspace_when_the_report_is_read_then_it_says_so() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(
        queue.clone(),
        Writer {
            calls: AtomicUsize::new(0),
            args: serde_json::json!({ "path": "../escaped.txt", "content": "hello" }).to_string(),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    queue.append("write above the workspace");

    agent.run().await.unwrap();

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("outside the workspace"),
        "the report does not say the edge was the limit:\n{report}"
    );
}
