//! Stories for the ledger behind undo: an edit is takeable back, an id is
//! never shared between two writers, and a file touched since is left alone.

use pearl_tools::Ops;
use rstest::rstest;
use tempfile::TempDir;

#[rstest]
fn given_a_written_file_when_its_op_is_taken_back_then_the_old_words_return() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "before").unwrap();
    let ops = Ops::new();
    let op = ops.next_op();
    std::fs::write(&path, "after").unwrap();
    ops.record_file(op, "edit a.txt", path.clone(), Some(b"before".to_vec()), b"after".to_vec());

    assert_eq!(ops.revert_last().unwrap().as_deref(), Some("undone op 0 (edit a.txt): 1 path(s) restored"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "before");
}

#[rstest]
fn given_a_file_the_tool_created_when_its_op_is_taken_back_then_the_file_is_gone() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("new.txt");
    let ops = Ops::new();
    let op = ops.next_op();
    std::fs::write(&path, "made").unwrap();
    ops.record_file(op, "write new.txt", path.clone(), None, b"made".to_vec());
    ops.revert(op).unwrap();
    assert!(!path.exists(), "the created file survived its own undo");
}

#[rstest]
fn given_a_file_changed_after_the_write_when_it_is_taken_back_then_it_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.txt");
    let ops = Ops::new();
    let op = ops.next_op();
    std::fs::write(&path, "op wrote this").unwrap();
    ops.record_file(op, "write a.txt", path.clone(), Some(b"older".to_vec()), b"op wrote this".to_vec());
    std::fs::write(&path, "the human wrote this").unwrap();

    let refused = ops.revert(op);
    assert!(refused.is_err(), "an undo overwrote a later write");
    let said = refused.err().map(|error| error.to_string()).unwrap_or_default();
    assert!(said.contains("changed after that write"), "the refusal does not say why: {said}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "the human wrote this");
}

#[rstest]
fn given_two_writers_on_one_ledger_when_each_takes_an_id_then_the_ids_differ() {
    let ops = Ops::new();
    let one = ops.clone();
    let first = ops.next_op();
    let second = one.next_op();
    assert_ne!(first, second, "two writers shared an op id");
}
