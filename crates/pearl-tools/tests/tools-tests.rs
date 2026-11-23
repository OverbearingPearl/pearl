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
    assert!(matches!(ws.edit("a.txt", "x", "y"), Err(ToolError::AnchorAmbiguous)));
}

#[rstest]
fn given_no_match_when_edited_then_anchor_not_found() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "abc").unwrap();
    let mut ws = Workspace::new(dir.path());
    assert!(matches!(ws.edit("a.txt", "zzz", "y"), Err(ToolError::AnchorNotFound)));
}

#[rstest]
fn given_a_climbing_path_when_read_then_rejected() {
    let dir = TempDir::new().unwrap();
    let ws = Workspace::new(dir.path());
    assert!(matches!(ws.read("../escape.txt"), Err(ToolError::Rejected(_))));
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
