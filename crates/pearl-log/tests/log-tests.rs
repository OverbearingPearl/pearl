//! Stories for the log: one writer, appended, resumable.

use pearl_log::{Entry, Role, Store, view};
use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Given a fresh temporary directory for a log file.
#[fixture]
fn dir() -> TempDir {
    TempDir::new().unwrap()
}

#[rstest]
fn given_new_store_when_append_then_read_back_in_order(dir: TempDir) {
    let path = dir.path().join("log.jsonl");
    let mut store = Store::open(&path).unwrap();
    store.append(Role::User, "hello").unwrap();
    store.append(Role::Assistant, "hi").unwrap();
    let entries = store.read().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].seq, 0);
    assert_eq!(entries[1].seq, 1);
    assert_eq!(entries[1].content, "hi");
}

#[rstest]
fn given_closed_store_when_reopened_then_history_resumes(dir: TempDir) {
    let path = dir.path().join("log.jsonl");
    {
        let mut store = Store::open(&path).unwrap();
        store.append(Role::User, "before the kill").unwrap();
    }
    // A new process opens the same file: the history is there, and the next
    // append continues the sequence rather than restarting it.
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.read().unwrap().len(), 1);
    let seq = store.append(Role::Assistant, "after reopen").unwrap();
    assert_eq!(seq, 1);
    assert_eq!(store.read().unwrap().len(), 2);
}

// A log is every run in a directory; a request is one conversation.
// The launch mark is where the view starts, so what a person types today is not
// answered against what an earlier run typed -- a resumed run still reads
// the real history, it just says which stretch of it this request is in.
#[rstest]
fn given_a_log_with_an_earlier_run_when_the_view_is_read_then_it_begins_at_the_launch() {
    let log = vec![
        Entry::new(0, Role::User, "last week's task"),
        Entry::new(1, Role::Assistant, "last week's answer"),
        Entry::new(2, Role::Launch, ""),
        Entry::new(3, Role::User, "today's task"),
    ];
    let shown = view(&log);
    assert_eq!(
        shown.len(),
        1,
        "the view carried the earlier run: {shown:?}"
    );
    assert_eq!(shown[0].content, "today's task");
}

// Before the mark existed a log had no way to say where a run started, and such
// a file is still the conversation it holds: dropping those messages would lose
// the history of every run written until now.
#[rstest]
fn given_a_log_written_before_the_mark_when_the_view_is_read_then_it_holds_every_message() {
    let log = vec![
        Entry::new(0, Role::User, "an older task"),
        Entry::new(1, Role::Assistant, "an older answer"),
    ];
    assert_eq!(view(&log).len(), 2);
}
