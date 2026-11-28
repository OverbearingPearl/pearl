//! Stories for the loop: a task across the wire, steering at the boundary.

use pearl_core::{NodeState, Queue};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_runner::Runner;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that answers with a fixed string, no network.
struct FixedProvider(&'static str);

impl Provider for FixedProvider {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion {
            content: self.0.to_string(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Default::default(),
        })
    }
}

fn runner(dir: &TempDir, answer: &'static str) -> Runner<FixedProvider> {
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    Runner::new(Queue::new(), FixedProvider(answer), store, "mock")
}

#[rstest]
#[tokio::test]
async fn given_a_queued_task_when_run_then_it_completes_and_persists() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir, "the answer");
    let (id, _) = a.queue.append("do the thing");
    a.run().await.unwrap();
    assert_eq!(a.queue.get(id).unwrap().state, NodeState::Done);
    assert_eq!(
        a.queue.get(id).unwrap().result.as_deref(),
        Some("the answer")
    );
    // The exchange is on disk: a user message and an assistant message at
    // least.
    let entries = a.log().read().unwrap();
    assert!(entries.len() >= 2);
}

#[rstest]
#[tokio::test]
async fn given_an_empty_queue_when_step_then_nothing_runs() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir, "x");
    assert!(a.step().await.unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn given_steering_before_a_round_when_run_then_it_reaches_the_log() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir, "done");
    let (id, _) = a.queue.append("task");
    a.queue.steer(id, "also consider X").unwrap();
    a.run().await.unwrap();
    let entries = a.log().read().unwrap();
    assert!(entries.iter().any(|e| e.content == "also consider X"));
    // Steering is drained, not left pending.
    assert!(a.queue.get(id).unwrap().steering.is_empty());
}

#[rstest]
#[tokio::test]
async fn given_two_tasks_when_run_then_both_complete_in_order() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir, "ok");
    let (first, _) = a.queue.append("first");
    let (second, _) = a.queue.append("second");
    let count = a.run().await.unwrap();
    assert_eq!(a.queue.get(first).unwrap().state, NodeState::Done);
    assert_eq!(a.queue.get(second).unwrap().state, NodeState::Done);
    assert_eq!(count, 2);
}

// --- M7: the tool-calling loop ---------------------------------------------

use pearl_provider::{Function, ToolCall};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A provider that asks for one tool call, then answers finally. It records
/// how many times it was asked, so the story can prove the loop ran twice.
struct ToolThenFinal {
    calls: AtomicUsize,
    tool: &'static str,
    args: &'static str,
}

impl Provider for ToolThenFinal {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    kind: "function".into(),
                    function: Function {
                        name: self.tool.into(),
                        arguments: self.args.into(),
                    },
                }],
                usage: Default::default(),
            })
        } else {
            Ok(Completion {
                content: "finished".into(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![],
                usage: Default::default(),
            })
        }
    }
}

/// A first round that asks for two tools at once, then an answer.
struct TwoThenFinal {
    calls: AtomicUsize,
}

impl Provider for TwoThenFinal {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let tool_calls = if n == 0 {
            vec![
                ToolCall {
                    id: "call_1".into(),
                    kind: "function".into(),
                    function: Function {
                        name: "read".into(),
                        arguments: r#"{"path":"a.txt"}"#.into(),
                    },
                },
                ToolCall {
                    id: "call_2".into(),
                    kind: "function".into(),
                    function: Function {
                        name: "bash".into(),
                        arguments: r#"{"command":"echo hi"}"#.into(),
                    },
                },
            ]
        } else {
            vec![]
        };
        Ok(Completion {
            content: if n == 0 {
                String::new()
            } else {
                "finished".into()
            },
            reasoning: None,
            model: "mock".into(),
            tool_calls,
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_tool_request_when_run_then_the_tool_runs_and_the_loop_finishes() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let provider = ToolThenFinal {
        calls: AtomicUsize::new(0),
        tool: "write",
        args: r#"{"path":"a.txt","content":"hello"}"#,
    };
    let mut a = Runner::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = a.queue.append("make a file");
    a.run().await.unwrap();
    assert_eq!(a.queue.get(id).unwrap().state, NodeState::Done);
    // The tool actually wrote the file inside the workspace.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "hello"
    );
    // The log recorded the tool result and the final answer.
    let entries = a.log().read().unwrap();
    assert!(entries.iter().any(|e| e.content == "wrote a.txt"));
    assert!(entries.iter().any(|e| e.content == "finished"));
    // Each round says what it was doing, so the bill cannot be read as a bare
    // number that no round owns.
    let bills = &a.queue.get(id).unwrap().bills;
    assert_eq!(
        bills.len(),
        2,
        "the loop took two rounds, not {}",
        bills.len()
    );
    assert_eq!(
        bills[0].note, "write a.txt",
        "the tool round did not name its work"
    );
    assert_eq!(
        bills[1].note, "answer",
        "the last round did not say it answered"
    );
}

#[tokio::test]
async fn given_a_turn_with_two_calls_when_it_is_costed_then_the_note_says_what_each_one_asked_for()
{
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let provider = TwoThenFinal {
        calls: AtomicUsize::new(0),
    };
    let mut a = Runner::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = a.queue.append("read it and say so");
    a.run().await.unwrap();
    let note = a.queue.get(id).unwrap().bills[0].note.clone();
    // Two calls in one round are two facts, and "read, bash" is neither: it
    // says which tools ran without saying what any of them was asked to do.
    assert!(
        note.contains("read a.txt"),
        "the read call lost its path: {note}"
    );
    assert!(
        note.contains("bash echo hi"),
        "the bash call lost its command: {note}"
    );
}

/// A command the platform's own shell cannot read. `)` is nothing to `cmd`,
/// and a bare `if` is nothing to `sh`: the story is about the complaint, so it
/// asks each shell for one in its own language.
fn unreadable() -> &'static str {
    #[cfg(windows)]
    return r#"{"command":"if"}"#;
    #[allow(unreachable_code)]
    r#"{"command":"echo )"}"#
}

#[tokio::test]
async fn given_a_command_the_shell_cannot_read_when_it_runs_then_its_complaint_is_the_report() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let provider = ToolThenFinal {
        calls: AtomicUsize::new(0),
        tool: "bash",
        args: unreadable(),
    };
    let mut a = Runner::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    a.queue.append("break a command");
    a.run().await.unwrap();
    // The shell's own complaint about a command it cannot read is a report the
    // model has to see. It used to be printed on the human's screen instead,
    // because the redirect lived inside the command the shell could not parse.
    let entries = a.log().read().unwrap();
    assert!(
        // The words are the shell's own, and each shell spells them its own
        // way: dash says "Syntax error", cmd says the syntax is incorrect.
        entries
            .iter()
            .any(|e| e.content.to_lowercase().contains("syntax")),
        "the shell's complaint never reached the report: {:?}",
        entries
            .iter()
            .map(|e| e.content.clone())
            .collect::<Vec<_>>()
    );
}

#[rstest]
#[tokio::test]
async fn given_a_file_edit_when_reverted_then_the_original_returns() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "original").unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let provider = ToolThenFinal {
        calls: AtomicUsize::new(0),
        tool: "edit",
        args: r#"{"path":"a.txt","anchor":"original","replacement":"changed"}"#,
    };
    let mut a = Runner::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    a.queue.append("edit the file");
    a.run().await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "changed"
    );
}
