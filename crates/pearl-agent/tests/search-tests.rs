//! Stories for finding a thing without running a shell.
//!
//! One log held 166 `grep` invocations through the shell -- 54 asking for line
//! numbers, 16 walking the tree -- and every one paid for a whole-workspace
//! baseline before it ran. A command that only looks cannot change anything,
//! so that baseline is an undo nobody will ever ask for. These stories pin the
//! cheaper door: the same hits, numbered, capped, and honest about the cut.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use tempfile::TempDir;

fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "use std::fmt;\n\nfn alpha() {}\n    fn indented() {}\nfn beta() {}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/other.rs"),
        "// fn commented() {}\nfn gamma() {}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "# Title\n\nfn in prose\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".git/config"), "fn hidden() {}\n").unwrap();
    std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
    std::fs::write(dir.path().join("target/debug/out"), "fn built() {}\n").unwrap();
    dir
}

#[rstest]
#[tokio::test]
async fn given_text_in_several_files_when_it_is_searched_then_every_hit_is_named_by_file_and_line() {
    let dir = workspace();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "fn "})).await;

    assert!(!out.is_error, "the search failed: {}", out.content);
    assert!(
        out.content.contains("src/lib.rs:3:fn alpha() {}"),
        "a hit is not named by file and line:\n{}",
        out.content
    );
    assert!(out.content.contains("src/other.rs:2:fn gamma() {}"));
    assert!(out.content.contains("README.md:3:fn in prose"));
    // Version control and build output are not the human's work, and a hit
    // inside them is a hit the reader did not ask for.
    assert!(!out.content.contains(".git"), "searched .git:\n{}", out.content);
    assert!(
        !out.content.contains("target/"),
        "searched build output:\n{}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_a_line_anchor_when_it_is_searched_then_only_lines_that_start_with_it_match() {
    // Most of the greps in a real log were anchors -- `^fn`, `^## ` -- and an
    // anchor read as text would match nothing at all, which is worse than
    // refusing it: the model would believe the code was not there.
    let dir = workspace();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "^fn"})).await;

    assert!(out.content.contains("src/lib.rs:3:fn alpha() {}"));
    assert!(
        !out.content.contains("indented"),
        "an anchored search matched an indented line:\n{}",
        out.content
    );
    assert!(
        !out.content.contains("commented"),
        "an anchored search matched a comment:\n{}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_a_pattern_only_a_regex_engine_reads_when_it_is_searched_then_the_shell_door_is_named() {
    let dir = workspace();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "^\\s*//"})).await;

    assert!(out.is_error, "a regex was silently searched as text");
    assert!(
        out.content.contains("bash grep"),
        "the refusal does not say which door does it: {}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_text_with_a_dot_in_it_when_it_is_searched_then_it_is_the_text_itself() {
    // `queue.get` and `Cargo.toml` are what people search for; a dot refused as
    // a wildcard would send most searches back to the shell.
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.rs"), "let n = queue.get(id);\n").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "queue.get"})).await;

    assert!(!out.is_error, "{}", out.content);
    assert!(out.content.contains("a.rs:1:let n = queue.get(id);"));
}

#[rstest]
#[tokio::test]
async fn given_one_directory_named_when_it_is_searched_then_only_that_directory_is_walked() {
    let dir = workspace();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools
        .call("search", json!({"text": "fn ", "path": "src"}))
        .await;

    assert!(out.content.contains("src/lib.rs"));
    assert!(
        !out.content.contains("README.md"),
        "the search left the path it was given:\n{}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_nothing_matching_when_it_is_searched_then_it_says_how_much_it_looked_at() {
    // "No matches" and "the tool did not work" look the same to a model unless
    // the answer says what was searched.
    let dir = workspace();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "nothing_here"})).await;

    assert!(!out.is_error);
    assert!(
        out.content.contains("files searched"),
        "an empty answer does not say what was searched: {}",
        out.content
    );
}

#[rstest]
#[tokio::test]
async fn given_more_hits_than_a_window_holds_when_it_is_searched_then_the_rest_is_counted() {
    let dir = TempDir::new().unwrap();
    let text: String = (0..500).map(|n| format!("needle {n}\n")).collect();
    std::fs::write(dir.path().join("many.txt"), text).unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools.call("search", json!({"text": "needle"})).await;

    assert!(
        out.content.contains("300 more matches"),
        "the cut does not say how many were left: {}",
        out.content.lines().last().unwrap_or_default()
    );
    assert!(out.content.lines().count() <= 202);
}

#[rstest]
#[tokio::test]
async fn given_a_search_when_the_tools_are_offered_then_finding_is_offered_beside_reading() {
    let dir = TempDir::new().unwrap();
    let tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let specs = tools.specs();
    assert!(
        specs.iter().any(|s| s.function.name == "search"),
        "the model is not offered a way to find anything"
    );
}
#[rstest]
#[tokio::test]
async fn given_a_search_that_found_something_when_the_row_names_it_then_it_says_what_was_looked_for() {
    use pearl_agent::Agent;
    use pearl_core::Queue;
    use pearl_provider::{
        Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall,
    };
    use pearl_session::SessionStore;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Finder(AtomicUsize);
    impl Provider for Finder {
        async fn complete(&self, _r: CompletionRequest) -> Result<Completion, ProviderError> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(Completion {
                    content: String::new(),
                    reasoning: None,
                    model: "mock".into(),
                    tool_calls: vec![ToolCall {
                        id: "1".into(),
                        kind: "function".into(),
                        function: Function {
                            name: "search".into(),
                            arguments: json!({"text": "^fn", "path": "src"}).to_string(),
                        },
                    }],
                    usage: Default::default(),
                });
            }
            Ok(Completion::text("found three", "mock"))
        }
    }

    let dir = workspace();
    let queue = Queue::new();
    let store = SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let mut agent = Agent::new(queue.clone(), Finder(AtomicUsize::new(0)), store, "mock")
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    let (id, _) = queue.append("find the functions");

    agent.run().await.unwrap();

    let note = queue
        .get(id)
        .and_then(|node| node.turns.first().map(|turn| turn.note.clone()))
        .unwrap_or_default();
    assert_eq!(
        note, "search ^fn",
        "the row does not say what was looked for"
    );
}
