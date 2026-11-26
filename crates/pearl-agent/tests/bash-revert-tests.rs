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
    let out = tools
        .call("bash", json!({"command": "echo made > made.txt"}))
        .await;
    assert!(!out.is_error);
    assert!(out.content.contains("[op 0]"));
    assert!(dir.path().join("made.txt").exists());
    let touched = tools.revert_bash(0).unwrap();
    assert!(touched >= 1);
    assert!(!dir.path().join("made.txt").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("keep.txt")).unwrap(),
        "original"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_shell_op_that_edits_a_file_when_reverted_then_the_original_returns() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "before").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    tools
        .call("bash", json!({"command": "echo after > a.txt"}))
        .await;
    assert_eq!(text(dir.path().join("a.txt")), "after\n");
    tools.revert_bash(0).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "before"
    );
}

#[rstest]
#[tokio::test]
async fn given_two_shell_ops_when_each_reverted_then_each_baseline_is_independent() {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::Full);
    tools
        .call("bash", json!({"command": "echo one > first.txt"}))
        .await;
    tools
        .call("bash", json!({"command": "echo two > second.txt"}))
        .await;
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
    let out = tools.call("bash", json!({ "command": FAILING })).await;
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
    let out = tools
        .call("bash", json!({"command": "echo hello-from-shell"}))
        .await;
    assert!(!out.is_error);
    assert!(out.content.contains("hello-from-shell"));
    assert!(!dir.path().join(".pearl-out-0").exists());
}

#[rstest]
#[tokio::test]
async fn given_a_command_the_sandbox_refuses_when_it_runs_then_the_output_says_the_sandbox_did_it()
{
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::ReadOnly);
    let out = tools
        .call("bash", json!({"command": "printf x > inside.txt"}))
        .await;
    assert!(
        out.is_error,
        "a write outside the workspace was allowed: {}",
        out.content
    );
    let notice = out
        .notice
        .expect("a refused write said nothing to the human");
    assert!(
        notice.starts_with("sandbox:"),
        "the notice did not name the sandbox: {notice}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_command_that_prints_the_words_and_succeeds_then_no_refusal_is_named() {
    // Reading a file about the sandbox prints the same words a refusal does.
    // The feature's first version announced a refused write for this command,
    // on a task that had written nothing: the words are evidence only when
    // the command came back failed.
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);
    let out = tools
        .call(
            "bash",
            json!({"command": "printf 'Operation not permitted\\n'"}),
        )
        .await;
    assert!(!out.is_error);
    assert_eq!(
        out.notice, None,
        "a command that succeeded was called a refusal"
    );
}

#[rstest]
#[tokio::test]
async fn given_an_edit_tool_call_when_the_human_undoes_it_then_the_file_is_as_it_was() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "keep this").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);
    let out = tools
        .call(
            "write",
            json!({"path": "a.txt", "content": "the model's words"}),
        )
        .await;
    assert!(!out.is_error, "the write failed: {}", out.content);
    assert_eq!(
        tools.ops().len(),
        1,
        "the write was not recorded: {:?}",
        tools.ops()
    );

    let said = tools.ledger().revert_last().unwrap().unwrap_or_default();

    assert!(
        said.contains("write a.txt"),
        "the undo does not name the write: {said}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep this");
}

#[rstest]
#[tokio::test]
async fn given_a_read_only_policy_when_the_write_tool_is_asked_for_a_file_then_the_human_is_told_the_ground_refused()
 {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::ReadOnly);

    let out = tools
        .call(
            "write",
            serde_json::json!({"path": "notes.txt", "content": "hi"}),
        )
        .await;

    assert!(out.is_error, "a read-only policy let a write through");
    let notice = out
        .notice
        .expect("the report had nothing to say about the refusal");
    assert!(
        notice.starts_with("sandbox: a write was refused"),
        "the notice does not name the sandbox: {notice}"
    );
    assert!(
        notice.contains("read-only"),
        "the notice does not say what the policy allows: {notice}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_path_climbing_out_of_the_workspace_when_it_is_refused_then_the_human_is_told_where_the_limit_is()
 {
    let dir = TempDir::new().unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let out = tools
        .call(
            "write",
            serde_json::json!({"path": "../escaped.txt", "content": "hi"}),
        )
        .await;

    assert!(out.is_error, "a path out of the root was written");
    let notice = out
        .notice
        .expect("nothing on the report said the path was refused");
    assert!(
        notice.contains("outside the workspace"),
        "the notice does not say what the limit was: {notice}"
    );
    assert!(
        notice.contains("writes land under"),
        "the notice does not say where writes go: {notice}"
    );
}
