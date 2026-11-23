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
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "original");
}

#[rstest]
fn given_a_clean_tree_when_shell_creates_a_file_then_restore_removes_it() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "keep").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::write(dir.path().join("created.txt"), "shell made this").unwrap();
    baseline.restore().unwrap();
    assert!(!dir.path().join("created.txt").exists());
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "keep");
}

#[rstest]
fn given_a_file_when_shell_deletes_it_then_restore_brings_it_back() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "precious").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::remove_file(dir.path().join("a.txt")).unwrap();
    baseline.restore().unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "precious");
}

#[rstest]
fn given_nested_files_when_restore_then_subdirectories_are_recreated() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src/deep")).unwrap();
    fs::write(dir.path().join("src/deep/a.txt"), "nested").unwrap();
    let baseline = Baseline::capture(dir.path()).unwrap();
    fs::remove_dir_all(dir.path().join("src")).unwrap();
    baseline.restore().unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("src/deep/a.txt")).unwrap(), "nested");
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
