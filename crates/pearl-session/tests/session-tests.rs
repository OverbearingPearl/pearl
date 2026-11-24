//! Stories for the session log: one writer, appended, resumable.

use pearl_session::{Role, SessionStore};
use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Given a fresh temporary directory for a session file.
#[fixture]
fn dir() -> TempDir {
    TempDir::new().unwrap()
}

#[rstest]
fn given_new_store_when_append_then_read_back_in_order(dir: TempDir) {
    let path = dir.path().join("session.jsonl");
    let mut store = SessionStore::open(&path).unwrap();
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
    let path = dir.path().join("session.jsonl");
    {
        let mut store = SessionStore::open(&path).unwrap();
        store.append(Role::User, "before the kill").unwrap();
    }
    // A new process opens the same file: the history is there, and the next
    // append continues the sequence rather than restarting it.
    let mut store = SessionStore::open(&path).unwrap();
    assert_eq!(store.read().unwrap().len(), 1);
    let seq = store.append(Role::Assistant, "after reopen").unwrap();
    assert_eq!(seq, 1);
    assert_eq!(store.read().unwrap().len(), 2);
}
