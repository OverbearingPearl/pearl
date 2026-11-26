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
    store
        .append(pearl_session::Role::User, "last week's task")
        .unwrap();
    store
        .append(pearl_session::Role::Assistant, "last week's answer")
        .unwrap();
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
    let kept: Vec<String> = store
        .read()
        .unwrap()
        .into_iter()
        .map(|e| e.content)
        .collect();
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
    assert!(
        refused.is_err(),
        "an empty file was resumed into a fresh session without a word"
    );
    let said = refused
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        said.contains("nothing to resume"),
        "the refusal does not name the problem: {said}"
    );
    assert!(
        !path.exists(),
        "the refusal created the file it refused to read"
    );
}

#[rstest]
fn given_a_session_a_night_old_when_it_is_resumed_then_it_opens() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    pearl::session(&path, false).unwrap();
    assert!(
        pearl::session(&path, true).is_ok(),
        "a written session was refused"
    );
}

#[rstest]
fn given_a_run_that_left_a_list_when_it_is_resumed_then_the_queue_comes_back_held() {
    use pearl_core::{NodeState, Queue};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    {
        let queue = Queue::new();
        queue.append("write the parser");
        queue.append("run the suite");
        let mut store = pearl::session(&path, false).unwrap();
        store.append_queue(queue.snapshot()).unwrap();
    }

    let store = pearl::session(&path, true).unwrap();
    let queue = Queue::new();
    let held = pearl::restore_queue(&store, &queue).unwrap();

    assert_eq!(
        held,
        Some(2),
        "the restored list is not the one that was left"
    );
    assert_eq!(queue.len(), 2);
    assert!(
        queue
            .view()
            .iter()
            .all(|task| task.held && task.state == NodeState::Queued)
    );
}

#[rstest]
fn given_a_list_edited_at_the_keyboard_when_nothing_ran_then_the_log_still_has_it() {
    use pearl_core::Queue;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    let mut store = pearl::session(&path, false).unwrap();
    let queue = Queue::new();
    queue.append("a task typed and never run");
    store.append_queue(queue.snapshot()).unwrap();
    assert!(
        store
            .last_queue()
            .unwrap()
            .unwrap()
            .contains("a task typed and never run")
    );
}

#[rstest]
#[cfg_attr(
    not(unix),
    ignore = "the claim file is not refused by a second open on Windows yet: M108"
)]
fn given_a_second_window_on_one_directory_when_it_opens_then_it_is_told_the_directory_is_taken() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("busy.jsonl");
    let _first = pearl::session(&path, false).unwrap();

    let second = pearl::session(&path, false);

    let said = second.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(said.contains("one window per directory"), "{said}");
    assert!(
        said.contains("busy.lock"),
        "the words do not name the file to remove: {said}"
    );
}

#[rstest]
fn given_a_last_run_that_left_a_list_when_a_fresh_launch_reads_it_then_the_list_belongs_to_the_old_run()
 {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.jsonl");
    {
        let mut first = pearl::session(&path, false).unwrap();
        first.append_queue(r#"{"nodes":{},"roots":[]}"#).unwrap();
    }
    // A fresh launch writes its own mark, so the snapshot before it belongs to
    // another conversation: restoring it would empty the promise `--fresh`
    // makes.
    let fresh = pearl::session(&path, false).unwrap();
    assert_eq!(
        fresh.last_queue().unwrap(),
        None,
        "a fresh launch read the run before it"
    );
}

#[rstest]
fn given_a_claim_left_by_a_window_that_died_when_the_next_one_opens_then_it_is_cleared() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("killed.jsonl");
    // A pid that cannot be running: the claim a terminal leaves when it kills
    // the window a human opened once and closed with the window manager.
    std::fs::write(path.with_extension("lock"), "999999\n").unwrap();

    let opened = pearl::session(&path, false);

    assert!(
        opened.is_ok(),
        "a claim nobody holds kept the next window out: {:?}",
        opened.err().map(|e| e.to_string())
    );
}

// A claim is a line a dying process may have written half of, and a pid is
// only a pid if it is digits. A claim naming nothing answers "somebody holds
// this" and not "nobody holds this": clearing it is a guess, and the guess
// is wrong exactly when another window made it.
#[rstest]
fn given_a_claim_that_names_no_process_when_the_next_one_opens_then_it_is_told_the_directory_is_taken()
 {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("half.jsonl");
    std::fs::write(path.with_extension("lock"), "a line nobody fini\n").unwrap();

    let opened = pearl::session(&path, false);

    let said = opened.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        said.contains("one window per directory"),
        "a claim it could not read was cleared: {said}"
    );
    assert!(
        said.contains("half.lock"),
        "the words do not name the file to remove: {said}"
    );
}

#[rstest]
fn given_a_new_conversation_when_the_ground_is_written_then_the_model_is_told_where_writes_land() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ground.jsonl");
    let store = pearl::session(&path, false).unwrap();
    let root = dir.path().join("work");
    std::fs::create_dir_all(&root).unwrap();

    let wrote = pearl::ground(&store, &root, pearl::RUNNER_POLICY).unwrap();

    assert!(wrote, "the ground was not written");
    let shown = store.read().unwrap();
    let said = shown
        .iter()
        .find(|e| e.content.starts_with("You work inside"))
        .map(|e| e.content.clone())
        .expect("the conversation does not say where the work stands");
    assert!(
        said.contains(&root.display().to_string()),
        "the root is missing: {said}"
    );
    assert!(
        said.contains("refuses"),
        "the model is not told the rest is refused: {said}"
    );
}

#[rstest]
fn given_a_conversation_that_already_says_the_ground_when_it_is_written_again_then_nothing_is_added()
 {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("once.jsonl");
    let store = pearl::session(&path, false).unwrap();
    pearl::ground(&store, dir.path(), pearl::RUNNER_POLICY).unwrap();

    let again = pearl::ground(&store, dir.path(), pearl::RUNNER_POLICY).unwrap();

    assert!(!again, "the ground was written a second time");
}
