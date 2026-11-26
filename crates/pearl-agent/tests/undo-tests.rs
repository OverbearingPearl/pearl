//! Stories for taking a write back.
//!
//! The headline promise is that every write can be taken back. A promise the
//! human cannot reach from the product is not kept: the op id was printed in a
//! tool result and nothing could act on it.

use pearl_agent::Agent;
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::SessionStore;
use pearl_tools::sandbox::Policy;
use rstest::rstest;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

/// A provider that runs one command and then answers.
struct Writer {
    calls: AtomicUsize,
    command: &'static str,
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
                        name: "bash".into(),
                        arguments: format!(r#"{{"command":"{}"}}"#, self.command),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion::text("done", "mock"))
    }
}

fn agent(dir: &TempDir, command: &'static str) -> Agent<Writer> {
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    Agent::new(
        Queue::new(),
        Writer {
            calls: AtomicUsize::new(0),
            command,
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite)
}

/// Read text with the line ending the human sees. cmd.exe writes CRLF and a
/// POSIX shell writes LF; the story is about the text that comes back, not
/// about which shell wrote it. A revert still restores exact bytes.
fn text(path: std::path::PathBuf) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

#[rstest]
#[tokio::test]
async fn given_a_file_the_agent_created_when_the_last_op_is_undone_then_the_file_is_gone() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "echo made > made.txt");
    a.queue.append("make a file");
    a.run().await.unwrap();
    assert!(
        dir.path().join("made.txt").exists(),
        "the fixture wrote nothing"
    );

    let undone = a
        .revert_last()
        .unwrap()
        .expect("nothing was offered to undo");
    assert!(
        !dir.path().join("made.txt").exists(),
        "the write was not taken back"
    );
    assert!(
        undone.contains("made.txt"),
        "the undo did not say what it undid: {undone}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_file_the_agent_changed_when_the_last_op_is_undone_then_the_old_text_returns() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("kept.txt"), "before\n").unwrap();
    let mut a = agent(&dir, "echo after > kept.txt");
    a.queue.append("change a file");
    a.run().await.unwrap();
    assert_eq!(text(dir.path().join("kept.txt")), "after\n");

    a.revert_last().unwrap().expect("nothing to undo");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("kept.txt")).unwrap(),
        "before\n"
    );
}

#[rstest]
#[tokio::test]
async fn given_no_write_when_an_undo_is_asked_for_then_it_says_there_is_nothing() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "true");
    assert!(a.revert_last().unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn given_two_ops_when_undo_is_asked_twice_then_each_goes_back_in_turn() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "echo one > one.txt");
    a.queue.append("first");
    a.run().await.unwrap();
    // A second op through the same registry, as a second task would.
    a.queue.append("second");
    a.run().await.unwrap();
    assert!(a.revert_last().unwrap().is_some());
}

#[rstest]
fn given_the_tool_list_when_it_is_read_then_taking_a_write_back_is_offered() {
    let dir = TempDir::new().unwrap();
    let registry = pearl_agent::ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);
    let names: Vec<String> = registry
        .specs()
        .iter()
        .map(|s| s.function.name.clone())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("revert")),
        "the model cannot take its own write back: {names:?}"
    );
}
