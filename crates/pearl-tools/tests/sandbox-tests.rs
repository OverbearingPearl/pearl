//! Given-When-Then stories for the sandbox, executed for real on each OS.
//!
//! These are not cross-compile checks: each story spawns a confined process and
//! asserts what it could and could not touch. CI runs them on macOS, Linux and
//! Windows, so the confinement is proved where it actually runs.

use pearl_tools::sandbox::{self, Policy};
use tempfile::TempDir;

/// A shell command that writes `text` into `path`, as (program, args) per OS.
fn write_cmd(path: &std::path::Path, text: &str) -> (String, Vec<String>) {
    let p = path.display();
    if cfg!(windows) {
        (
            String::from("cmd"),
            vec![String::from("/C"), format!("echo {text}> \"{p}\"")],
        )
    } else {
        (
            String::from("sh"),
            vec![String::from("-c"), format!("printf %s '{text}' > '{p}'")],
        )
    }
}

fn run(program: &str, args: &[String], policy: Policy, root: &str) -> Option<i32> {
    let mut child = sandbox::spawn(program, args, policy, root).expect("spawn");
    child.wait_code().expect("wait")
}

#[test]
fn given_workspace_write_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let target = root.path().join("inside.txt");
    let (prog, args) = write_cmd(&target, "hello");
    let root_str = root.path().to_str().unwrap();
    let _ = run(&prog, &args, Policy::WorkspaceWrite, root_str);
    assert!(target.exists(), "a write inside the workspace must succeed");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
}

#[test]
fn given_workspace_write_when_writing_outside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("escape.txt");
    let (prog, args) = write_cmd(&target, "nope");
    let root_str = root.path().to_str().unwrap();
    let _ = run(&prog, &args, Policy::WorkspaceWrite, root_str);
    assert!(
        !target.exists(),
        "a write outside the workspace must be confined away"
    );
}

#[test]
fn given_read_only_when_writing_inside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let target = root.path().join("readonly.txt");
    let (prog, args) = write_cmd(&target, "nope");
    let root_str = root.path().to_str().unwrap();
    let _ = run(&prog, &args, Policy::ReadOnly, root_str);
    assert!(
        !target.exists(),
        "read-only must deny even a write inside the workspace"
    );
}

#[test]
fn given_full_policy_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let target = root.path().join("full.txt");
    let (prog, args) = write_cmd(&target, "free");
    let root_str = root.path().to_str().unwrap();
    let _ = run(&prog, &args, Policy::Full, root_str);
    assert!(target.exists(), "full policy runs as the user");
}
