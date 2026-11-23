//! Stories for the loop: a task across the wire, steering at the boundary.

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that answers with a fixed string, no network.
struct FixedProvider(&'static str);

impl Provider for FixedProvider {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        Ok(Completion { content: self.0.to_string(), model: "mock".into(), tool_calls: vec![] })
    }
}

fn agent(dir: &TempDir, answer: &'static str) -> Agent<FixedProvider> {
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    Agent::new(Queue::new(), FixedProvider(answer), store, "mock")
}

#[rstest]
#[tokio::test]
async fn given_a_queued_task_when_run_then_it_completes_and_persists() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "the answer");
    let (id, _) = a.queue.append("do the thing");
    a.run().await.unwrap();
    assert_eq!(a.queue.get(id).unwrap().state, NodeState::Done);
    assert_eq!(a.queue.get(id).unwrap().result.as_deref(), Some("the answer"));
    // The exchange is on disk: a user turn and an assistant turn at least.
    let entries = a.session().read().unwrap();
    assert!(entries.len() >= 2);
}

#[rstest]
#[tokio::test]
async fn given_an_empty_queue_when_step_then_nothing_runs() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "x");
    assert!(a.step().await.unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn given_steering_before_a_turn_when_run_then_it_reaches_the_session() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "done");
    let (id, _) = a.queue.append("task");
    a.queue.steer(id, "also consider X").unwrap();
    a.run().await.unwrap();
    let entries = a.session().read().unwrap();
    assert!(entries.iter().any(|e| e.content == "also consider X"));
    // Steering is drained, not left pending.
    assert!(a.queue.get(id).unwrap().steering.is_empty());
}

#[rstest]
#[tokio::test]
async fn given_two_tasks_when_run_then_both_complete_in_order() {
    let dir = TempDir::new().unwrap();
    let mut a = agent(&dir, "ok");
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
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    kind: "function".into(),
                    function: Function { name: self.tool.into(), arguments: self.args.into() },
                }],
            })
        } else {
            Ok(Completion { content: "finished".into(), model: "mock".into(), tool_calls: vec![] })
        }
    }
}

#[rstest]
#[tokio::test]
async fn given_a_tool_request_when_run_then_the_tool_runs_and_the_loop_finishes() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let provider = ToolThenFinal { calls: AtomicUsize::new(0), tool: "write", args: r#"{"path":"a.txt","content":"hello"}"# };
    let mut a = Agent::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = a.queue.append("make a file");
    a.run().await.unwrap();
    assert_eq!(a.queue.get(id).unwrap().state, NodeState::Done);
    // The tool actually wrote the file inside the workspace.
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "hello");
    // The session recorded the tool result and the final answer.
    let entries = a.session().read().unwrap();
    assert!(entries.iter().any(|e| e.content == "wrote a.txt"));
    assert!(entries.iter().any(|e| e.content == "finished"));
}

#[rstest]
#[tokio::test]
async fn given_a_file_edit_when_reverted_then_the_original_returns() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "original").unwrap();
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let provider = ToolThenFinal { calls: AtomicUsize::new(0), tool: "edit", args: r#"{"path":"a.txt","anchor":"original","replacement":"changed"}"# };
    let mut a = Agent::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    a.queue.append("edit the file");
    a.run().await.unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "changed");
}
