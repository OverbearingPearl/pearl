//! What a child's reading costs the parent.
//!
//! A wide search is cheap in a conversation of its own and expensive in the one
//! that has to carry it for the rest of the task: every later round pays for
//! every earlier output again. Fan-out exists so the reading happens somewhere
//! else and only the conclusion comes back. These stories are about the bill
//! that arrangement avoids, measured on the request the parent actually sends.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use pearl_core::{Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_runner::{Policy, Runner};
use rstest::rstest;
use tempfile::TempDir;

/// The shell's own way of printing a file. `cat` on Windows is an MSYS
/// program, and the sandbox cannot start one (ADR 19), so a story that asks
/// for it there measures the cage instead of the feature it names.
fn read_work() -> String {
    let verb = if cfg!(windows) { "type" } else { "cat" };
    format!(r#"{{"command":"{verb} work.txt"}}"#)
}

/// The parent asks for one child, then answers. The child reads the same file
/// twenty times before it answers. Both share the endpoint, so the story tells
/// them apart by the title the child was given.
struct Searcher {
    parent: Mutex<Vec<CompletionRequest>>,
    child_rounds: AtomicUsize,
    child_prompt: AtomicUsize,
}

impl Searcher {
    fn new() -> Self {
        Self {
            parent: Mutex::new(Vec::new()),
            child_rounds: AtomicUsize::new(0),
            child_prompt: AtomicUsize::new(0),
        }
    }
}

impl Provider for Searcher {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let shown: usize = request.messages.iter().map(|m| m.content.len()).sum();
        // The child's own log holds its title and nothing else. The
        // parent's digest names the child too, so a substring match would
        // make the parent a child once the row exists.
        let is_child = request.messages.iter().any(|m| {
            m.role == pearl_provider::Role::User && m.content.trim() == "search the whole tree"
        });
        let mut done = if is_child {
            self.child_prompt.fetch_add(shown, Ordering::SeqCst);
            if self.child_rounds.fetch_add(1, Ordering::SeqCst) < 20 {
                Completion {
                    content: String::new(),
                    reasoning: None,
                    model: "mock".into(),
                    tool_calls: vec![pearl_provider::ToolCall {
                        id: "1".into(),
                        kind: "function".into(),
                        function: pearl_provider::Function {
                            name: "bash".into(),
                            arguments: read_work().into(),
                        },
                    }],
                    usage: Usage::default(),
                }
            } else {
                Completion::text("the answer is in src/lib.rs:12", "mock")
            }
        } else {
            self.parent.lock().unwrap().push(request);
            let asked = self.parent.lock().unwrap().iter().any(|r| {
                r.messages
                    .iter()
                    .any(|m| m.content.contains("src/lib.rs:12"))
            });
            if !asked {
                Completion {
                    content: String::new(),
                    reasoning: None,
                    model: "mock".into(),
                    tool_calls: vec![pearl_provider::ToolCall {
                        id: "f1".into(),
                        kind: "function".into(),
                        function: pearl_provider::Function {
                            name: "subagent".into(),
                            arguments: r#"{"titles":["search the whole tree"]}"#.into(),
                        },
                    }],
                    usage: Usage::default(),
                }
            } else {
                Completion::text("the task is done", "mock")
            }
        };
        done.usage = Usage {
            prompt_tokens: (shown / 4) as u32,
            completion_tokens: 20,
            cached_tokens: 0,
        };
        Ok(done)
    }
}

#[rstest]
#[tokio::test]
async fn given_a_wide_search_sent_to_a_child_when_the_parent_asks_again_then_its_prompt_carries_the_conclusion_and_not_the_reading()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("work.txt"), "x".repeat(4_000)).unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let queue = Queue::new();
    let mut runner = Runner::new(queue.clone(), Searcher::new(), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    queue.append("find where the answer lives");

    runner.run().await.unwrap();

    let asked = runner.provider().parent.lock().unwrap();
    let last = asked.last().expect("the parent never asked again");
    let text: String = last
        .messages
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("src/lib.rs:12"),
        "the parent never read what the child concluded:\n{text}"
    );
    assert!(
        !text.contains(&"x".repeat(200)),
        "the parent's prompt carries the child's reading"
    );
    // The child read eighty thousand characters to find one line. A parent
    // that had done that reading itself would carry it into every later round.
    let child = runner.provider().child_prompt.load(Ordering::SeqCst);
    let parent: usize = last.messages.iter().map(|m| m.content.len()).sum();
    assert!(
        child > 40_000,
        "the child did not do the reading: {child} characters"
    );
    assert!(
        parent < child / 10,
        "the parent carries {parent} characters against the child's {child}"
    );
}
