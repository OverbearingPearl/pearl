//! Stories for reading a file: what the model gets, and what the row says.
//!
//! A session's turns were nearly all `grep`, `sed` and `head` through the
//! shell, because `read` took a path and nothing else: no range, no budget,
//! no way to say where to continue. The shell had all three, so the model
//! used the shell -- and every shell op pays for a whole-workspace baseline,
//! which a read cannot need. These stories pin the cheaper door being worth
//! walking through: a range, numbered lines, and a cut that says how to go on.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use tempfile::TempDir;

#[rstest]
#[tokio::test]
async fn given_a_long_file_when_a_range_is_read_then_the_lines_are_numbered_and_the_rest_is_named() {
    let dir = TempDir::new().unwrap();
    let text: String = (1..=5000).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.path().join("long.txt"), &text).unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools
        .call("read", json!({"path": "long.txt", "offset": 1200, "limit": 3}))
        .await;

    assert!(!out.is_error, "the read failed: {}", out.content);
    assert!(
        out.content.contains("  1200\tline 1200\n"),
        "the window is not the lines asked for, numbered: {}",
        out.content
    );
    assert!(out.content.contains("  1202\tline 1202\n"));
    assert!(!out.content.contains("line 1203"));
    assert!(
        out.content.contains("3798 more lines of 5000"),
        "the cut does not say how much is left: {}",
        out.content
    );
    assert!(
        out.content.contains("offset 1203"),
        "the cut does not say how to continue: {}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_a_short_file_when_it_is_read_then_the_whole_of_it_is_shown_without_a_cut() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("read", json!({"path": "a.txt"})).await;

    assert_eq!(out.content, "     1\tone\n     2\ttwo\n");
}

#[rstest]
#[tokio::test]
async fn given_a_file_of_one_very_long_line_when_it_is_read_then_the_line_is_cut_too() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("bundle.js"), "x".repeat(100_000)).unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("read", json!({"path": "bundle.js"})).await;

    assert!(
        out.content.len() < 4_000,
        "one long line went through whole: {} bytes",
        out.content.len()
    );
    assert!(out.content.contains('\u{2026}'), "the cut is not marked");
}

#[rstest]
#[tokio::test]
async fn given_an_offset_past_the_end_when_it_is_read_then_the_reader_is_told_the_length() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools
        .call("read", json!({"path": "a.txt", "offset": 90}))
        .await;

    assert!(out.is_error);
    assert!(
        out.content.contains("2 lines"),
        "the length is not said: {}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_an_empty_file_when_it_is_read_then_it_says_empty_rather_than_nothing() {
    // Nothing at all is what a failed read looks like too, and the model
    // cannot tell an empty file from a tool that did not work.
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("empty.txt"), "").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("read", json!({"path": "empty.txt"})).await;

    assert!(!out.is_error);
    assert!(
        out.content.contains("empty"),
        "an empty file reads as nothing at all: {:?}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_a_read_when_the_tools_are_offered_then_the_range_is_offered_with_it() {
    // The model chooses between doors by what they say. A read that advertises
    // only a path is a door the shell outbids, because the shell can page.
    let dir = TempDir::new().unwrap();
    let tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let specs = tools.specs();
    let read = specs.iter().find(|s| s.function.name == "read").expect("no read");
    let schema = read.function.parameters.to_string();
    assert!(schema.contains("offset"), "no offset offered: {schema}");
    assert!(schema.contains("limit"), "no limit offered: {schema}");
}
#[rstest]
#[tokio::test]
async fn given_a_read_of_a_range_when_the_task_runs_then_the_row_names_the_range() {
    use pearl_agent::Agent;
    use pearl_core::Queue;
    use pearl_provider::{
        Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall,
    };
    use pearl_session::SessionStore;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Reader {
        calls: AtomicUsize,
        args: String,
    }
    impl Provider for Reader {
        async fn complete(&self, _r: CompletionRequest) -> Result<Completion, ProviderError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(Completion {
                    content: String::new(),
                    reasoning: None,
                    model: "mock".into(),
                    tool_calls: vec![ToolCall {
                        id: "1".into(),
                        kind: "function".into(),
                        function: Function {
                            name: "read".into(),
                            arguments: self.args.clone(),
                        },
                    }],
                    usage: Default::default(),
                });
            }
            Ok(Completion::text("read it", "mock"))
        }
    }

    let dir = TempDir::new().unwrap();
    let text: String = (1..=5000).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.path().join("long.txt"), &text).unwrap();
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(
        queue.clone(),
        Reader {
            calls: AtomicUsize::new(0),
            args: json!({"path": "long.txt", "offset": 1200, "limit": 200}).to_string(),
        },
        store,
        "mock",
    )
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (id, _) = queue.append("look at the middle");

    agent.run().await.unwrap();

    let note = queue
        .get(id)
        .and_then(|node| node.turns.first().map(|turn| turn.note.clone()))
        .unwrap_or_default();
    assert_eq!(note, "read long.txt:1200-1399");
}
