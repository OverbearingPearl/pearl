//! Given-When-Then stories for the sandbox, executed for real on each OS.
//!
//! These are not cross-compile checks: each story spawns a confined process and
//! asserts what it could and could not touch. CI runs them on macOS, Linux and
//! Windows, so the confinement is proved where it actually runs.
//!
//! Every command names its target relative to the workspace, because the
//! sandbox sets the working directory to the workspace and a relative name
//! needs no quoting. A quoted absolute path would have to survive two parsers
//! (the command line and the shell), and a test that fights its own quoting
//! proves nothing about confinement.

use pearl_tools::sandbox::{self, Policy};
use tempfile::TempDir;

/// A shell command that writes `text` into `target`, as (program, args) per OS.
fn write_cmd(target: &str, text: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            String::from("cmd"),
            vec![
                String::from("/C"),
                format!("echo {text}> {target}"),
            ],
        )
    } else {
        (
            String::from("sh"),
            vec![String::from("-c"), format!("printf %s '{text}' > '{target}'")],
        )
    }
}

/// Spawn the command under `policy` with `root` as the workspace, and wait.
fn run(target: &str, text: &str, policy: Policy, root: &str) {
    let (program, args) = write_cmd(target, text);
    let mut child = sandbox::spawn(&program, &args, policy, root).expect("spawn");
    child.wait_code().expect("wait");
}

#[test]
fn given_workspace_write_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("inside.txt", "hello", Policy::WorkspaceWrite, root_str);
    let written = std::fs::read_to_string(root.path().join("inside.txt"))
        .expect("a write inside the workspace must succeed");
    assert_eq!(written.trim_end(), "hello");
}

#[test]
fn given_workspace_write_when_writing_outside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    // One level up is the temporary directory that holds the workspace: a real
    // place to write, and outside the sandbox's grant.
    let escape = format!("pearl-escape-{}.txt", std::process::id());
    run(&format!("../{escape}"), "nope", Policy::WorkspaceWrite, root_str);
    let outside = root.path().parent().unwrap().join(&escape);
    let landed = outside.exists();
    if landed {
        let _ = std::fs::remove_file(&outside);
    }
    assert!(!landed, "a write outside the workspace must be confined away");
}

#[test]
fn given_read_only_when_writing_inside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("readonly.txt", "nope", Policy::ReadOnly, root_str);
    assert!(
        !root.path().join("readonly.txt").exists(),
        "read-only must deny even a write inside the workspace"
    );
}

#[test]
fn given_full_policy_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("full.txt", "free", Policy::Full, root_str);
    assert!(
        root.path().join("full.txt").exists(),
        "full policy runs as the user"
    );
}
