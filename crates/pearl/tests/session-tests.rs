//! Stories for what a launch carries: a conversation, or the one it resumed.
//!
//! The session file is every run in a directory. What a request carries is the
//! conversation this launch began, because a queue that starts empty and a
//! context that starts full is the muddiness a person feels as "it remembers
//! the wrong thing".

use std::path::Path;

use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Given a fresh directory for a session file.
#[fixture]
fn dir() -> TempDir {
    TempDir::new().unwrap()
}

/// A session file that an earlier run wrote, with its own launch mark.
fn earlier(path: &Path) {
    let mut store = pearl_session::SessionStore::open(path).unwrap();
    store.append_launch().unwrap();
    store.append(pearl_session::Role::User, "last week's task").unwrap();
    store.append(pearl_session::Role::Assistant, "last week's answer").unwrap();
}

/// What the model would be sent: the view of the log, as the wire reads it.
fn carried(path: &Path, resumed: bool) -> Vec<String> {
    let store = pearl::session(path, resumed).unwrap();
    pearl_agent::replay(&pearl_session::view(&store.read().unwrap()))
        .into_iter()
        .map(|m| m.content)
        .collect()
}

#[rstest]
fn given_a_session_file_from_an_earlier_run_when_a_launch_opens_it_then_the_conversation_is_empty(
    dir: TempDir,
) {
    let path = dir.path().join("pearl-session.jsonl");
    earlier(&path);
    let sent = carried(&path, false);
    assert!(sent.is_empty(), "a launch carried an earlier run: {sent:?}");
}

#[rstest]
fn given_a_session_file_from_an_earlier_run_when_a_resume_opens_it_then_the_conversation_is_that_run(
    dir: TempDir,
) {
    let path = dir.path().join("pearl-session.jsonl");
    earlier(&path);
    let sent = carried(&path, true);
    assert_eq!(sent, ["last week's task", "last week's answer"]);
}

// The log itself keeps every run, so the file is still the record of the
// directory: a launch adds a mark rather than throwing history away.
#[rstest]
fn given_a_session_file_from_an_earlier_run_when_a_launch_opens_it_then_the_file_keeps_that_run(
    dir: TempDir,
) {
    let path = dir.path().join("pearl-session.jsonl");
    earlier(&path);
    let store = pearl::session(&path, false).unwrap();
    let kept: Vec<String> = store.read().unwrap().into_iter().map(|e| e.content).collect();
    assert!(
        kept.iter().any(|c| c == "last week's answer"),
        "the earlier run was dropped from the file: {kept:?}"
    );
}

#[rstest]
fn given_a_path_nothing_was_ever_written_to_when_it_is_resumed_then_it_says_so() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nothing.jsonl");
    let refused = pearl::session(&path, true);
    assert!(refused.is_err(), "an empty file was resumed into a fresh session without a word");
    let said = refused.err().map(|error| error.to_string()).unwrap_or_default();
    assert!(said.contains("nothing to resume"), "the refusal does not name the problem: {said}");
    assert!(!path.exists(), "the refusal created the file it refused to read");
}

#[rstest]
fn given_a_session_a_night_old_when_it_is_resumed_then_it_opens() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    pearl::session(&path, false).unwrap();
    assert!(pearl::session(&path, true).is_ok(), "a written session was refused");
}
