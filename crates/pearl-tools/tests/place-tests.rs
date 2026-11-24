//! Stories for the ground the work stands on.
//!
//! Which directory, under which sandbox, on which branch: three facts whose
//! absence is only noticed after the files have already changed.

use pearl_tools::place::{policy_name, Place};
use pearl_tools::sandbox::Policy;
use rstest::rstest;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "trunk"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    git(dir.path(), &["add", "a.txt"]);
    git(dir.path(), &["commit", "-qm", "first"]);
    dir
}

#[rstest]
fn given_a_plain_directory_when_the_place_is_read_then_no_branch_is_claimed() {
    let dir = TempDir::new().unwrap();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, true);
    assert_eq!(place.branch, None);
    assert!(
        place.describe().starts_with("write:"),
        "the sandbox is a directory and must be named as one: {}",
        place.describe()
    );
}

#[rstest]
fn given_a_repository_when_the_place_is_read_then_the_branch_is_named() {
    let dir = repo();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, true);
    assert_eq!(place.branch.as_deref(), Some("trunk"));
    assert_eq!(place.dirty, Some(0));
    assert!(place.describe().contains("git:trunk"), "{}", place.describe());
}

#[rstest]
fn given_a_changed_tree_when_the_place_is_read_then_the_count_is_shown() {
    let dir = repo();
    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
    std::fs::write(dir.path().join("new.txt"), "new").unwrap();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, true);
    assert_eq!(place.dirty, Some(2), "an untracked file is a change too");
    assert!(place.describe().contains("git:trunk*2"), "{}", place.describe());
}

#[rstest]
fn given_git_was_not_asked_when_the_place_is_read_then_the_count_is_unknown_not_clean() {
    let dir = repo();
    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, false);
    assert_eq!(place.dirty, None);
    assert!(place.describe().contains("*?"), "an unknown count was shown as clean");
}

#[rstest]
fn given_a_worktree_when_the_place_is_read_then_the_branch_is_still_found() {
    let dir = repo();
    let gitdir = dir.path().join("elsewhere");
    std::fs::rename(dir.path().join(".git"), &gitdir).unwrap();
    std::fs::write(dir.path().join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, false);
    assert_eq!(place.branch.as_deref(), Some("trunk"), "a .git file was not followed");
}

#[rstest]
fn given_a_detached_head_when_the_place_is_read_then_the_commit_is_named() {
    let dir = repo();
    let sha = String::from_utf8(
        Command::new("git").arg("-C").arg(dir.path()).args(["rev-parse", "HEAD"]).output().unwrap().stdout,
    ).unwrap();
    std::fs::write(dir.path().join(".git").join("HEAD"), sha.trim()).unwrap();
    let place = Place::read(dir.path(), Policy::WorkspaceWrite, false);
    assert_eq!(place.branch.as_deref(), Some(&sha.trim()[..7]));
}

#[rstest]
fn given_each_policy_when_it_is_named_then_the_promise_is_in_the_word() {
    assert_eq!(policy_name(Policy::ReadOnly), "read-only");
    assert_eq!(policy_name(Policy::WorkspaceWrite), "workspace-write");
    assert_eq!(policy_name(Policy::Full), "full-access");
}

#[rstest]
fn given_a_path_under_home_when_it_is_written_then_home_is_a_tilde() {
    let home = std::env::var("HOME").unwrap();
    let place = Place::read(Path::new(&home), Policy::ReadOnly, false);
    assert!(place.describe().starts_with('~'), "{}", place.describe());
}
