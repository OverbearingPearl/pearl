//! Stories for the workspace baseline: a shell op's before-image, restored.
//! Each fails without the revert it names.

use pearl_tools::Baseline;
use rstest::rstest;
use std::fs;
use tempfile::TempDir;

#[rstest]
fn given_a_file_when_changed_after_capture_then_restore_returns_the_original() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "original").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::write(dir.path().join("a.txt"), "changed by shell").unwrap();
    let touched = baseline.restore().unwrap();
    assert_eq!(touched, 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "original"
    );
}

#[rstest]
fn given_a_clean_tree_when_shell_creates_a_file_then_restore_removes_it() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "keep").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::write(dir.path().join("created.txt"), "shell made this").unwrap();
    baseline.restore().unwrap();
    assert!(!dir.path().join("created.txt").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "keep"
    );
}

#[rstest]
fn given_a_file_when_shell_deletes_it_then_restore_brings_it_back() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "precious").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::remove_file(dir.path().join("a.txt")).unwrap();
    baseline.restore().unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "precious"
    );
}

#[rstest]
fn given_nested_files_when_restore_then_subdirectories_are_recreated() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src/deep")).unwrap();
    fs::write(dir.path().join("src/deep/a.txt"), "nested").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::remove_dir_all(dir.path().join("src")).unwrap();
    baseline.restore().unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("src/deep/a.txt")).unwrap(),
        "nested"
    );
}

#[rstest]
fn given_git_and_target_when_capture_then_they_are_skipped() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/config"), "git stuff").unwrap();
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("target/build.o"), "artifact").unwrap();
    fs::write(dir.path().join("work.txt"), "real work").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    assert_eq!(baseline.len(), 1);
}

#[rstest]
fn given_no_change_when_restore_then_nothing_is_touched() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "stable").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    let touched = baseline.restore().unwrap();
    assert_eq!(touched, 0);
}

#[rstest]
fn given_a_destination_inside_the_source_when_a_copy_is_asked_for_then_it_is_refused() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let err = pearl_tools::copy_workspace(dir.path(), dir.path().join("children").join("one"))
        .expect_err("a copy inside its own source was accepted");
    assert!(err.to_string().contains("inside"), "{err}");
}

#[rstest]
fn given_pearls_own_log_when_capture_and_restore_then_the_log_keeps_what_was_written_since() {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("pearl-session.jsonl");
    fs::write(&log, "{\"seq\":0}\n").unwrap();
    fs::write(dir.path().join("work.txt"), "before").unwrap();
    let own = pearl_tools::Own::new();
    own.add(&log);
    let baseline = Baseline::capture_except(dir.path(), &own).unwrap();
    assert_eq!(baseline.len(), 1, "the log is not the human's work");

    // The command changes a work file; the run keeps appending to the log.
    fs::write(dir.path().join("work.txt"), "after").unwrap();
    fs::write(&log, "{\"seq\":0}\n{\"seq\":1}\n").unwrap();
    baseline.restore().unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("work.txt")).unwrap(),
        "before"
    );
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "{\"seq\":0}\n{\"seq\":1}\n",
        "the restore rewound the log"
    );
}

#[rstest]
fn given_a_log_created_after_capture_when_restore_then_it_is_not_removed() {
    let dir = TempDir::new().unwrap();
    let own = pearl_tools::Own::new();
    own.add(dir.path().join("pearl-session.jsonl"));
    let baseline = Baseline::capture_except(dir.path(), &own).unwrap();
    fs::write(dir.path().join("pearl-session.jsonl"), "{\"seq\":0}\n").unwrap();
    baseline.restore().unwrap();
    assert!(
        dir.path().join("pearl-session.jsonl").exists(),
        "the restore deleted a log that did not exist at capture"
    );
}

#[rstest]
fn given_another_ops_output_file_when_capture_then_it_is_not_captured() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".pearl-out-7"), "a command still running").unwrap();
    fs::write(dir.path().join("work.txt"), "work").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    assert_eq!(baseline.len(), 1);
    baseline.restore().unwrap();
    assert!(dir.path().join(".pearl-out-7").exists());
}

#[rstest]
fn given_pearls_own_log_when_a_child_copies_the_workspace_then_the_copy_has_no_conversation() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("pearl-session.jsonl"), "the parent's turns").unwrap();
    fs::write(dir.path().join("parent-child.jsonl"), "a sibling's turns").unwrap();
    fs::write(dir.path().join("work.txt"), "the human's work").unwrap();
    let own = pearl_tools::Own::new();
    own.add(dir.path().join("pearl-session.jsonl"));
    own.add(dir.path().join("parent-child.jsonl"));
    let dest = TempDir::new().unwrap();
    let copied = pearl_tools::copy_workspace_except(dir.path(), dest.path(), &own).unwrap();
    assert_eq!(copied, 1);
    assert!(dest.path().join("work.txt").exists());
    assert!(!dest.path().join("pearl-session.jsonl").exists());
    assert!(!dest.path().join("parent-child.jsonl").exists());
}

#[rstest]
fn given_a_tree_nothing_touched_when_a_second_command_is_captured_then_it_is_not_read_again() {
    // A run of commands walks nearly the same tree every time, and the bytes
    // are already on disk under their own content's name. Reading them again
    // is the bill a shell op pays for looking; statting is the bill it owes.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("big.bin"), vec![b'x'; 24 * 1024 * 1024]).unwrap();
    // A file the run did not just write, which is nearly every file in a
    // workspace: a mark stamped inside this second is never trusted, because
    // two writes in one second look alike.
    let file = fs::File::options()
        .write(true)
        .open(dir.path().join("big.bin"))
        .unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(10))
        .unwrap();
    drop(file);
    let ledger = pearl_tools::Ops::new();
    let own = pearl_tools::Own::new();

    let started = std::time::Instant::now();
    let first = ledger.capture(dir.path(), &own).unwrap();
    let cold = started.elapsed();
    let started = std::time::Instant::now();
    let second = ledger.capture(dir.path(), &own).unwrap();
    let warm = started.elapsed();

    assert_eq!(first.len(), second.len(), "the second capture lost a file");
    assert!(
        warm * 3 < cold,
        "an unchanged tree was read again: {cold:?} then {warm:?}"
    );
}

#[rstest]
fn given_a_file_written_twice_in_one_second_when_the_second_op_is_undone_then_the_first_write_returns()
 {
    // The dangerous half of not reading a file again: a filesystem that stamps
    // whole seconds cannot tell two writes apart inside one second, and an
    // undo built on the stale mark would restore the wrong bytes in silence.
    let dir = TempDir::new().unwrap();
    let ledger = pearl_tools::Ops::new();
    let own = pearl_tools::Own::new();
    fs::write(dir.path().join("a.txt"), "original").unwrap();
    let first = ledger.capture(dir.path(), &own).unwrap();
    ledger.record(0, "task", "write one", first);

    fs::write(dir.path().join("a.txt"), "one").unwrap();
    let second = ledger.capture(dir.path(), &own).unwrap();
    ledger.record(1, "task", "write two", second);
    fs::write(dir.path().join("a.txt"), "two").unwrap();

    assert_eq!(ledger.revert(1).unwrap(), 1);
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "one");
}
