//! Stories for fanning out from inside a round.
//!
//! The pool, the copies and the concurrent runner existed and nothing could
//! reach them: no tool offered fan-out, so a subagent was a library capability.
//! These stories are about the reach -- the model asks for children and gets
//! their answers back in the same round.

use pearl_core::{NodeState, Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_runner::Runner;
use pearl_tools::sandbox::Policy;
use rstest::rstest;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

/// First round: ask for two children. Later: answer, naming what came back.
struct Fan {
    calls: AtomicUsize,
}

impl Provider for Fan {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        // A child's own round: its fresh log holds only its title, and no
        // tool result yet. Children use the same endpoint as the parent.
        let is_child = request
            .messages
            .iter()
            .any(|m| m.role == pearl_provider::Role::User && m.content.starts_with("look at"))
            && !request
                .messages
                .iter()
                .any(|m| m.role == pearl_provider::Role::Tool);
        if is_child {
            return Ok(Completion::text("CHILD-DONE", "mock"));
        }
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let asked_fan = request
            .messages
            .iter()
            .any(|m| m.role == pearl_provider::Role::Tool && m.content.contains("CHILD-DONE"));
        if n == 0 {
            return Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "f1".into(),
                    kind: "function".into(),
                    function: Function {
                        name: "subagent".into(),
                        arguments: r#"{"titles":["look at a","look at b"]}"#.into(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion::text(
            if asked_fan {
                "SAW-CHILDREN"
            } else {
                "SAW-NOTHING"
            },
            "mock",
        ))
    }
}

#[rstest]
fn given_the_tool_list_when_it_is_read_then_fanning_out_is_offered() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let a = Runner::new(
        Queue::new(),
        Fan {
            calls: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let names: Vec<String> = a.tool_names();
    assert!(
        names.iter().any(|n| n == "subagent"),
        "fan-out was not offered: {names:?}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_task_that_asks_for_children_when_it_runs_then_the_children_settle() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Runner::new(
        Queue::new(),
        Fan {
            calls: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (parent, _) = a.queue.append("split the work");
    a.run().await.unwrap();

    let node = a.queue.get(parent).unwrap();
    assert_eq!(node.children.len(), 2, "no children were created");
    for child in &node.children {
        let c = a.queue.get(*child).unwrap();
        assert_eq!(
            c.state,
            NodeState::Done,
            "child {child} never ran: {:?}",
            c.state
        );
    }
}

#[rstest]
#[tokio::test]
async fn given_children_that_answered_when_the_parent_continues_then_it_sees_their_answers() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Runner::new(
        Queue::new(),
        Fan {
            calls: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (parent, _) = a.queue.append("split the work");
    a.run().await.unwrap();
    let node = a.queue.get(parent).unwrap();
    assert_eq!(
        node.result.as_deref(),
        Some("SAW-CHILDREN"),
        "the answers never came back"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_child_when_it_runs_then_it_works_in_its_own_copy() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("seed.txt"), "seed").unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Runner::new(
        Queue::new(),
        Fan {
            calls: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_settings(Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    a.queue.append("split the work");
    a.run().await.unwrap();
    let beside = dir.path().parent().unwrap().join(".pearl-children");
    let copies: Vec<String> = std::fs::read_dir(&beside)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        copies.len() >= 2,
        "each child needs its own copy beside the workspace: {copies:?}"
    );
    assert!(
        !dir.path().join(".pearl-children").exists(),
        "a copy inside the workspace copies itself and pollutes the parent's baseline"
    );
}

/// Records what each request carried, and answers plainly.
struct Recorder {
    seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Provider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let text = request
            .messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        self.seen.lock().unwrap().push(text);
        Ok(Completion::text("noted", "mock"))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_nested_child_that_answered_when_the_parent_runs_then_it_reads_what_the_child_found()
 {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut a = Runner::new(Queue::new(), Recorder { seen: seen.clone() }, store, "mock")
        .with_settings(Settings::new("mock"));
    let (parent, _) = a.queue.append("the whole");
    let (child, _) = a.queue.add_child(parent, "inspect the parser").unwrap();
    a.run().await.unwrap();

    assert_eq!(a.queue.get(child).unwrap().state, NodeState::Done);
    assert_eq!(a.queue.get(parent).unwrap().state, NodeState::Done);
    let last = seen.lock().unwrap().last().cloned().unwrap_or_default();
    assert!(
        last.contains("inspect the parser") && last.contains("noted"),
        "the parent never read its child's finding: {last}"
    );
}

// A door is read by a model deciding what to do next. "Split this task into
// child tasks that run at the same time" describes a task that is already
// several, and a model holding one task reads it as not applying -- which is
// why a night of wide searching spends its rounds in the parent, filling the
// conversation the parent has to carry for the rest of the task. The offer has
// to name what a single task saves by using it.
#[rstest]
fn given_the_doors_offered_to_a_model_when_the_fanning_one_is_read_then_it_names_what_the_reading_would_cost_here()
 {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("s.jsonl")).unwrap();
    let runner = Runner::new(
        Queue::new(),
        Fan {
            calls: AtomicUsize::new(0),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), Policy::WorkspaceWrite);

    let specs = runner.all_specs();
    let door = specs
        .iter()
        .find(|s| s.function.name == "subagent")
        .expect("no door offers a subagent");
    let said = door.function.description.to_lowercase();
    assert!(
        said.contains("out of this conversation") || said.contains("not this conversation"),
        "the door does not say where the reading stays: {said}"
    );
    assert!(
        said.contains("search") || said.contains("read"),
        "the door does not name the work it is for: {said}"
    );
}
