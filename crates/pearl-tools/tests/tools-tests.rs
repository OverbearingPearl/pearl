//! Stories for the agent's hands: edits you can take back, paths that stay put.

use pearl_tools::{ToolError, Workspace};
use rstest::rstest;
use tempfile::TempDir;

#[rstest]
fn given_a_file_when_read_then_its_bytes_returned() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let ws = Workspace::new(dir.path());
    assert_eq!(ws.read("a.txt").unwrap(), "hello");
}

#[rstest]
fn given_a_unique_anchor_when_edited_then_only_that_text_changes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "foo bar baz").unwrap();
    let mut ws = Workspace::new(dir.path());
    ws.edit("a.txt", "bar", "QUX").unwrap();
    assert_eq!(ws.read("a.txt").unwrap(), "foo QUX baz");
}

#[rstest]
fn given_an_edit_when_reverted_then_the_original_returns() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "original").unwrap();
    let mut ws = Workspace::new(dir.path());
    ws.edit("a.txt", "original", "changed").unwrap();
    assert_eq!(ws.read("a.txt").unwrap(), "changed");
    ws.revert("a.txt").unwrap();
    assert_eq!(ws.read("a.txt").unwrap(), "original");
}

#[rstest]
fn given_a_new_file_when_written_and_reverted_then_it_is_removed() {
    let dir = TempDir::new().unwrap();
    let mut ws = Workspace::new(dir.path());
    ws.write("new.txt", "fresh").unwrap();
    assert!(dir.path().join("new.txt").exists());
    ws.revert("new.txt").unwrap();
    assert!(!dir.path().join("new.txt").exists());
}

#[rstest]
fn given_two_matches_when_edited_then_ambiguous_error() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "x x").unwrap();
    let mut ws = Workspace::new(dir.path());
    assert!(matches!(
        ws.edit("a.txt", "x", "y"),
        Err(ToolError::AnchorAmbiguous)
    ));
}

#[rstest]
fn given_no_match_when_edited_then_anchor_not_found() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "abc").unwrap();
    let mut ws = Workspace::new(dir.path());
    assert!(matches!(
        ws.edit("a.txt", "zzz", "y"),
        Err(ToolError::AnchorNotFound)
    ));
}

#[rstest]
fn given_a_climbing_path_when_read_then_rejected() {
    let dir = TempDir::new().unwrap();
    let ws = Workspace::new(dir.path());
    assert!(matches!(
        ws.read("../escape.txt"),
        Err(ToolError::Rejected(_))
    ));
}

#[rstest]
fn given_several_edits_when_revert_all_then_every_file_restored() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "one").unwrap();
    std::fs::write(dir.path().join("b.txt"), "two").unwrap();
    let mut ws = Workspace::new(dir.path());
    ws.edit("a.txt", "one", "1").unwrap();
    ws.edit("b.txt", "two", "2").unwrap();
    ws.revert_all().unwrap();
    assert_eq!(ws.read("a.txt").unwrap(), "one");
    assert_eq!(ws.read("b.txt").unwrap(), "two");
}

#[rstest]
fn given_a_long_file_when_a_range_is_read_then_only_that_range_is_held() {
    // The window is what is kept: a tool result is appended to the log and
    // replayed into every request after it, so a whole large file read to show
    // twenty lines is paid for on every round that follows.
    let dir = TempDir::new().unwrap();
    let text: String = (1..=5000).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.path().join("long.txt"), &text).unwrap();
    let ws = Workspace::new(dir.path());

    let window = ws.read_window("long.txt", 1200, 3).unwrap();

    assert_eq!(window.lines, vec!["line 1200", "line 1201", "line 1202"]);
    assert_eq!(window.from, 1200);
    // The count is carried to the end, because "there is more" without a
    // number leaves the reader unable to decide whether to keep reading.
    assert_eq!(window.total, 5000);
    assert!(!window.bytes);
}

#[rstest]
fn given_a_file_of_one_very_long_line_when_a_window_is_read_then_the_byte_budget_stops_it() {
    // A window of lines is no budget against a minified bundle: one line can
    // be a hundred thousand characters.
    let dir = TempDir::new().unwrap();
    let text: String = (0..40).map(|_| format!("{}\n", "x".repeat(8_000))).collect();
    std::fs::write(dir.path().join("bundle.js"), &text).unwrap();
    let ws = Workspace::new(dir.path());

    let window = ws.read_window("bundle.js", 1, 2000).unwrap();

    assert!(window.bytes, "the byte budget did not stop the window");
    assert!(
        window.lines.iter().map(|l| l.len()).sum::<usize>() < 80 * 1024,
        "the window went past its budget"
    );
    assert_eq!(window.total, 40);
}

#[rstest]
fn given_a_file_that_is_not_text_when_a_window_is_read_then_it_says_so() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("blob.bin"), [0xff, 0xfe, 0x00, 0x81]).unwrap();
    let ws = Workspace::new(dir.path());

    let err = ws.read_window("blob.bin", 1, 10).unwrap_err().to_string();

    assert!(err.contains("not text"), "the refusal is not explained: {err}");
}
