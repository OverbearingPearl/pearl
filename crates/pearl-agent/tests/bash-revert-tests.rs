//! Stories for reverting a shell op: the command runs confined, and its effect
//! on the workspace can be taken back. Each fails without the revert it names.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Read text with the line ending the human sees. cmd.exe writes CRLF and a
/// POSIX shell writes LF; the story is about the text that comes back, not
/// about which shell wrote it. A revert still restores exact bytes.
fn text(path: std::path::PathBuf) -> String {
    fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

/// A command that fails with a diagnostic on stderr and exit code 2, in the
/// shell the tool runs on this platform.
#[cfg(not(windows))]
const FAILING: &str = "echo 'error: cannot find value' >&2; exit 2";
#[cfg(windows)]
const FAILING: &str = "echo error: cannot find value 1>&2 & exit /b 2";

#[rstest]
#[tokio::test]
async fn given_a_shell_op_that_creates_a_file_when_reverted_then_the_file_is_gone() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("keep.txt"), "original").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    let out = tools.call("bash", json!({"command": "echo made > made.txt"})).await;
    assert!(!out.is_error);
    assert!(out.content.contains("[op 0]"));
    assert!(dir.path().join("made.txt").exists());
    let touched = tools.revert_bash(0).unwrap();
    assert!(touched >= 1);
    assert!(!dir.path().join("made.txt").exists());
    assert_eq!(fs::read_to_string(dir.path().join("keep.txt")).unwrap(), "original");
}

#[rstest]
#[tokio::test]
async fn given_a_shell_op_that_edits_a_file_when_reverted_then_the_original_returns() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "before").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    tools.call("bash", json!({"command": "echo after > a.txt"})).await;
    assert_eq!(text(dir.path().join("a.txt")), "after\n");
    tools.revert_bash(0).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "before");
}

#[rstest]
#[tokio::test]
async fn given_two_shell_ops_when_each_reverted_then_each_baseline_is_independent() {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    tools.call("bash", json!({"command": "echo one > first.txt"})).await;
    tools.call("bash", json!({"command": "echo two > second.txt"})).await;
    assert!(dir.path().join("first.txt").exists());
    assert!(dir.path().join("second.txt").exists());
    // Revert the second op only: its file goes, the first op's file stays.
    tools.revert_bash(1).unwrap();
    assert!(!dir.path().join("second.txt").exists());
    assert!(dir.path().join("first.txt").exists());
}

#[rstest]
#[tokio::test]
async fn given_an_unknown_op_when_reverted_then_nothing_happens() {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    let touched = tools.revert_bash(99).unwrap();
    assert_eq!(touched, 0);
}

#[rstest]
#[tokio::test]
async fn given_a_failing_command_when_it_runs_then_its_diagnostics_reach_the_model() {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    let out = tools
        .call("bash", json!({ "command": FAILING }))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("exit 2"));
    assert!(out.content.contains("error: cannot find value"));
    // The capture file is gone; only the baseline remains.
    assert!(!dir.path().join(".pearl-out-0").exists());
}

#[rstest]
#[tokio::test]
async fn given_a_successful_command_when_it_runs_then_its_output_reaches_the_model() {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    let out = tools.call("bash", json!({"command": "echo hello-from-shell"})).await;
    assert!(!out.is_error);
    assert!(out.content.contains("hello-from-shell"));
    assert!(!dir.path().join(".pearl-out-0").exists());
}
