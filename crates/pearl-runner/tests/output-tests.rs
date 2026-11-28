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
