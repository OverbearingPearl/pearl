//! Stories for what an undo may touch.
//!
//! A revert takes back the human's work, and pearl's own state is not the
//! human's work: the conversation in `pearl-session.jsonl` is the record of
//! everything that happened, including the command being undone. These stories
//! sit at the top because the defect lives in the seam between the session log
//! and the workspace baseline -- each crate's own stories were green while an
//! undo ate the conversation.

use pearl_agent::Agent;
use pearl_core::{Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::{Role, SessionStore};
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
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
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

#[rstest]
#[tokio::test]
async fn given_a_log_in_the_workspace_when_the_last_shell_op_is_undone_then_the_conversation_is_whole()
 {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "echo made > made.txt");
    a.queue.append("make a file");
    a.run().await.unwrap();
    assert!(dir.path().join("made.txt").exists(), "the op did nothing");

    // The human keeps talking after the command ran. Those turns exist only
    // in the log, and the log is a file in the workspace the baseline captured.
    let mut watch = a.session().clone();
    watch
        .append(Role::User, "and what did you find out?")
        .unwrap();

    let undone = a.revert_last().unwrap().expect("nothing to undo");
    assert!(undone.contains("made.txt"), "the undo named nothing");
    assert!(!dir.path().join("made.txt").exists(), "the work stayed");

    let log = std::fs::read_to_string(dir.path().join("pearl-session.jsonl")).unwrap();
    assert!(
        log.contains("and what did you find out?"),
        "the undo rewound the conversation to before the command"
    );
    assert!(
        log.contains("done"),
        "the answer the model gave is gone from the log"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_list_typed_after_a_shell_op_when_that_op_is_undone_then_the_list_still_comes_back()
{
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "echo made > made.txt");
    a.queue.append("make a file");
    a.run().await.unwrap();

    // The list the human shapes while the agent works is persisted in the same
    // file (ADR 29). An undo of the command must not take the list with it.
    let mut watch = a.session().clone();
    let later = Queue::new();
    later.append("the next thing I want");
    watch.append_queue(later.snapshot()).unwrap();

    a.revert_last().unwrap().expect("nothing to undo");
    let back = a
        .session()
        .last_queue()
        .unwrap()
        .expect("the list is gone from the log");
    assert!(
        back.contains("the next thing I want"),
        "the undo took the human's list with it"
    );
}
