//! Stories for what a long answer looks like when it is cut short: the human
//! reads the head of it and a mark saying there was more. Each fails without
//! the cut it names.

use pearl_runner::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Print a file the story wrote, in the shell this platform runs.
#[cfg(not(windows))]
const SHOW: &str = "cat long.txt";
#[cfg(windows)]
const SHOW: &str = "type \"long.txt\"";

#[rstest]
#[tokio::test]
async fn given_a_long_answer_in_chinese_when_it_is_read_back_then_the_head_arrives_whole() {
    let dir = TempDir::new().unwrap();
    // Three bytes a character, so the cut at 8000 bytes lands inside one of
    // them. A cut counted in bytes and taken in bytes is a panic here, and the
    // window dies with the answer the human was waiting for.
    fs::write(dir.path().join("long.txt"), "中".repeat(3000)).unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    let out = tools.call("bash", json!({"command": SHOW})).await;
    assert!(!out.is_error, "the command failed: {}", out.content);
    assert!(
        out.content.contains("truncated"),
        "a cut answer says it was cut: {}",
        out.content
    );
    assert!(
        out.content.contains(&"中".repeat(2000)),
        "the head of the answer is what the human reads"
    );
    assert!(
        !out.content.contains('\u{fffd}'),
        "no half a character reached the screen"
    );
}

/// On Windows an MSYS program is ended by the loader before it prints a word.
/// The report the model reads has to say so, or it retries the same command
/// against the same empty capture.
#[rstest]
#[cfg(windows)]
#[tokio::test]
async fn given_a_program_the_sandbox_cannot_start_when_its_report_is_read_then_the_words_say_so() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("work.txt"), "the words\n").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);
    let out = tools.call("bash", json!({"command": "cat work.txt"})).await;
    assert!(out.is_error, "the command did not fail: {}", out.content);
    assert!(
        out.content.contains("never started"),
        "the report is a number and nothing else: {}",
        out.content
    );
    assert!(
        out.content.contains("type"),
        "the report does not name what to run instead: {}",
        out.content
    );
}

/// The shell tool tells the model which commands its shell can run before the
/// model spends a round finding out.
#[rstest]
#[cfg(windows)]
fn given_the_windows_shell_when_its_tools_are_listed_then_the_commands_it_can_run_are_named() {
    let dir = TempDir::new().unwrap();
    let tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);
    let specs = tools.specs();
    let bash = specs.iter().find(|s| s.name == "bash").expect("a bash tool");
    assert!(bash.description.contains("findstr"), "{}", bash.description);
    assert!(bash.description.contains("cat"), "{}", bash.description);
}
