//! Stories for a tool that refuses, and for the human being told why.
//!
//! A refusal that only the model hears is a row that quietly wrote nothing: the
//! model retries, the round is spent, and the task still says it is working.
//! The person's only account of the limit is the notice the refusal leaves
//! behind, so every way a write can be refused has to leave one. What the
//! report then shows of that notice is `crates/pearl/tests/refusal-tests.rs`,
//! where the screen itself can be read.

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
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

#[rstest]
#[tokio::test]
async fn given_a_write_named_by_an_absolute_path_when_it_is_refused_then_the_report_says_why() {
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
    // The model is told; the human has to be told as well, or the row looks
    // like it wrote the file it never wrote.
    let node = queue.get(id).unwrap();
    assert!(
        node.notices.iter().any(|n| n.contains("absolute path")),
        "the refusal left no notice: {:?}",
        node.notices
    );
    assert_eq!(node.state, NodeState::Done);
    assert!(
        !inside.exists(),
        "the write landed after all, so the story is about nothing"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_write_that_climbs_out_of_the_workspace_when_it_is_refused_then_the_report_says_so() {
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
    let (id, _) = queue.append("write above the workspace");

    agent.run().await.unwrap();

    let node = queue.get(id).unwrap();
    assert!(
        node.notices.iter().any(|n| n.contains("outside the workspace")),
        "the refusal left no notice of the workspace's edge: {:?}",
        node.notices
    );
}
