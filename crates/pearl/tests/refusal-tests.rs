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
async fn given_a_write_named_by_the_absolute_path_of_a_file_inside_when_the_report_is_read_then_the_row_names_it_from_the_root()
 {
    // A path from the root of the filesystem is how the model learns the
    // tree: `pwd` prints it, a compiler error prints it. When that path is a
    // file the human already owns, the write is the write that was asked for,
    // so what the row has to show is that it happened -- named the way every
    // other line in the log and in the baseline names a file, from the root.
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

    let app = pearl_tui::App::new(queue.clone());
    let report = rows_text!(pearl_tui::report_lines(&app, 100));
    assert!(
        report.contains("write created.txt"),
        "the report does not name the file the write landed in:\n{report}"
    );
    assert!(
        !report.contains("was refused"),
        "a write that landed left a refusal on the row:\n{report}"
    );
    assert!(
        inside.exists(),
        "the write named a file inside the workspace and did not land"
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
