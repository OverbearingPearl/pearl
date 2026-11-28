//! Stories for the log: one writer, appended, resumable.

use pearl_log::{Entry, Role, Store, view};
use std::fs::OpenOptions;
use std::io::Write;
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

/// The queue watcher and a run append to the same file from two threads. A
/// record whose newline is a second write lets the next record land inside it,
/// and the log then opens as `malformed log line ... trailing characters` --
/// one bad line and the product will not start.
#[rstest]
fn given_two_threads_appending_at_once_when_the_log_is_read_then_no_line_is_torn(dir: TempDir) {
    let path = dir.path().join("log.jsonl");
    let mut first = Store::open(&path).unwrap();
    let mut second = first.clone();
    let filler = "the line a fold keeps ".repeat(100);
    let a = std::thread::spawn(move || {
        for i in 0..200 {
            first.append(Role::User, format!("{filler}{i}")).unwrap();
        }
    });
    let b = std::thread::spawn(move || {
        for i in 0..200 {
            second.append(Role::Queue, format!("{{\"i\":{i}}}")).unwrap();
        }
    });
    a.join().unwrap();
    b.join().unwrap();
    let read = Store::peek(&path);
    assert!(
        read.is_ok(),
        "two writers tore a line: {:?}",
        read.err().map(|e| e.to_string())
    );
    assert_eq!(read.unwrap().len(), 400);
}

/// A kill in the middle of an append leaves the record's first bytes behind and
/// no newline. Those bytes were never a record, so the open drops them rather
/// than refusing the whole log for them.
#[rstest]
fn given_a_write_cut_short_when_the_log_is_opened_then_the_rest_is_readable(dir: TempDir) {
    let path = dir.path().join("log.jsonl");
    let mut store = Store::open(&path).unwrap();
    store.append(Role::User, "before the kill").unwrap();
    store.append(Role::Assistant, "also before").unwrap();
    drop(store);
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(br#"{"seq":2,"role":"user","content":"the write tha"#)
        .unwrap();
    drop(file);
    let mut store = Store::open(&path).unwrap();
    let entries = store.read().unwrap();
    assert_eq!(entries.len(), 2, "a torn record was kept or the log refused");
    assert_eq!(store.append(Role::User, "after the kill").unwrap(), 2);
}

/// A record whose bytes all landed but whose newline did not is a whole record:
/// the open writes the newline it lost instead of dropping a message that is
/// there.
#[rstest]
fn given_a_record_whose_newline_was_lost_when_the_log_is_opened_then_it_is_kept(dir: TempDir) {
    let path = dir.path().join("log.jsonl");
    let mut store = Store::open(&path).unwrap();
    store.append(Role::User, "before the kill").unwrap();
    let last = r#"{"seq":1,"role":"assistant","content":"the answer that landed"}"#;
    drop(store);
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(last.as_bytes()).unwrap();
    drop(file);
    let mut store = Store::open(&path).unwrap();
    let entries = store.read().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].content, "the answer that landed");
    // And the next record lands on a line of its own, not inside it.
    store.append(Role::User, "after the kill").unwrap();
    assert_eq!(store.read().unwrap().len(), 3);
}
