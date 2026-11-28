//! Stories for a tool that refuses, and for the human being told why.
//!
//! A refusal that only the model hears is a row that quietly wrote nothing: the
//! model retries, the round is spent, and the task still says it is working.
//! The person's only account of the limit is the notice the refusal leaves
//! behind, so every way a write can be refused has to leave one. What the
//! report then shows of that notice is `crates/pearl/tests/refusal-tests.rs`,
//! where the screen itself can be read.

use pearl_core::{NodeState, Queue};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_runner::{Runner, ToolRegistry};
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
async fn given_a_write_named_by_the_absolute_path_of_a_file_inside_when_the_task_runs_then_the_file_is_written()
 {
    let dir = TempDir::new().unwrap();
    let inside = dir.path().join("created.txt");
    let queue = Queue::new();
    let store = Store::open(dir.path().join("pearl-log.jsonl")).unwrap();
    let mut runner = Runner::new(
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

    runner.run().await.unwrap();

    // A path from the root of the filesystem is how the model learns the tree
    // -- `pwd` prints it, a compiler error prints it. When it names a file the
    // human already owns, the write is the write that was asked for; refusing
    // it calls the human's own tree a boundary it never crossed.
    assert!(
        inside.exists(),
        "the write named a file inside the workspace and did not land"
    );
    let node = queue.get(id).unwrap();
    assert_eq!(node.state, NodeState::Done);
    assert!(
        node.notices.is_empty(),
        "a write that landed left a refusal behind it: {:?}",
        node.notices
    );
}

#[rstest]
#[tokio::test]
async fn given_a_write_that_climbs_out_of_the_workspace_when_it_is_refused_then_the_report_says_so()
{
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let store = Store::open(dir.path().join("pearl-log.jsonl")).unwrap();
    let mut runner = Runner::new(
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

    runner.run().await.unwrap();

    let node = queue.get(id).unwrap();
    assert!(
        node.notices
            .iter()
            .any(|n| n.contains("outside the workspace")),
        "the refusal left no notice of the workspace's edge: {:?}",
        node.notices
    );
}

/// A model that tries an absolute name, then repeats what the refusal told it.
struct Reporter {
    calls: AtomicUsize,
    args: String,
    heard: std::sync::Arc<std::sync::Mutex<String>>,
}

impl Provider for Reporter {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
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
        let mut heard = self.heard.lock().unwrap();
        for message in &request.messages {
            heard.push_str(&message.content);
            heard.push('\n');
        }
        Ok(Completion::text("done", "mock"))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_write_named_by_the_absolute_path_of_a_file_outside_when_it_is_refused_then_the_row_says_so()
 {
    // The one case the absolute name does not buy: a file the human's tree
    // does not hold. The model is told the edge is the edge, and not that its
    // spelling was wrong, because the spelling was fine.
    let dir = TempDir::new().unwrap();
    let elsewhere = TempDir::new().unwrap();
    let outside = elsewhere.path().join("escaped.txt");
    let queue = Queue::new();
    let store = Store::open(dir.path().join("pearl-log.jsonl")).unwrap();
    let mut runner = Runner::new(
        queue.clone(),
        Writer {
            calls: AtomicUsize::new(0),
            args: serde_json::json!({ "path": outside.display().to_string(), "content": "hello" })
                .to_string(),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let (id, _) = queue.append("write above the workspace");

    runner.run().await.unwrap();

    let node = queue.get(id).unwrap();
    assert!(
        node.notices
            .iter()
            .any(|n| n.contains("outside the workspace")),
        "the refusal did not name the edge: {:?}",
        node.notices
    );
    assert!(
        !outside.exists(),
        "the write landed outside the workspace after all"
    );
}

#[rstest]
fn given_the_doors_offered_to_a_model_when_the_writing_one_is_read_then_the_name_it_wants_is_stated()
 {
    // Every door that takes a name says which root its name is from, and the
    // writing one is pinned here because it is the door that loses work when
    // the model guesses: a wrong read costs a round, a wrong write costs the
    // file.
    let dir = TempDir::new().unwrap();
    let tools = ToolRegistry::new(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    let specs = tools.specs();
    let write = specs
        .iter()
        .find(|s| s.function.name == "write")
        .expect("no write door");
    assert!(
        write
            .function
            .description
            .contains("relative to the workspace root"),
        "the writing door does not say which root its name is from: {}",
        write.function.description
    );
}
