//! Stories for what a long run holds in memory.
//!
//! Every shell op keeps a before-image so it can be taken back, and the human
//! watches `mem` on the status line while a session runs. The promise is both
//! halves at once: the process does not grow with the number of commands, and
//! the oldest op is still revertible to the exact bytes it started from.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

const MIB: u64 = 1024 * 1024;

#[rstest]
#[tokio::test]
async fn given_a_large_work_file_when_many_shell_ops_run_then_the_process_holds_no_copy_of_each() {
    let dir = TempDir::new().unwrap();
    // A workspace with one substantial file in it: an asset, a fixture, a data
    // file. Nothing here is build output, so a baseline keeps every byte.
    fs::write(dir.path().join("big.bin"), vec![b'x'; 8 * MIB as usize]).unwrap();
    fs::write(dir.path().join("work.txt"), "original").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let before = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");
    for i in 0..12 {
        let out = tools
            .call("bash", json!({"command": format!("echo {i} >> work.txt")}))
            .await;
        assert!(!out.is_error, "the command did not run: {}", out.content);
    }
    let after = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");

    // Twelve ops over an 8 MiB file is 96 MiB if each keeps its own copy.
    assert!(
        after.saturating_sub(before) < 32 * MIB,
        "the ledger is holding a copy per op: mem grew {} MiB over 12 commands",
        after.saturating_sub(before) / MIB
    );

    // And the promise the memory was paying for still holds: the FIRST op
    // goes back to the bytes the human had, twelve commands later.
    let touched = tools.revert_bash(0).unwrap();
    assert!(touched >= 1, "the oldest op could not be taken back");
    assert_eq!(
        fs::read_to_string(dir.path().join("work.txt")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::metadata(dir.path().join("big.bin")).unwrap().len(),
        8 * MIB
    );
}

/// A snapshot as an older window wrote it: every row carrying its answer.
fn fat(snapshot: &str, answer: &str) -> String {
    let mut tree: serde_json::Value = serde_json::from_str(snapshot).unwrap();
    // `tree["nodes"]` is a `Value`, and only the object inside it can be
    // walked: a snapshot with no `nodes` object has nothing to fatten.
    let nodes = tree["nodes"]
        .as_object_mut()
        .expect("a snapshot carries its nodes as an object");
    for node in nodes.values_mut() {
        node["result"] = json!(answer);
    }
    tree.to_string()
}

/// A provider that answers once, so the story is about the question and not
/// about the work.
struct Answerer;

impl pearl_provider::Provider for Answerer {
    async fn complete(
        &self,
        _request: pearl_provider::CompletionRequest,
    ) -> Result<pearl_provider::Completion, pearl_provider::ProviderError> {
        Ok(pearl_provider::Completion::text(
            "here is what I found",
            "mock",
        ))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_night_of_list_snapshots_when_the_next_question_is_asked_then_the_window_does_not_hold_the_list()
 {
    use pearl_core::{Queue, Settings};
    use pearl_session::Role;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    let mut store = pearl::session(&path, false).unwrap();

    // A night of work as an OLDER window wrote it: two dozen tasks, each with
    // an answer worth reading, and every snapshot carrying those answers
    // again. Logs like this are on disk in real directories, and questioning
    // one still has to be cheap -- a fix that only helps new logs helps nobody
    // who has been running the program for a week.
    let night = Queue::new();
    let answer = "答".repeat(1400);
    let mut ids = vec![];
    for i in 0..24 {
        let (id, _) = night.append(format!("task {i}"));
        night.start(id).unwrap();
        night.complete(id, answer.clone()).unwrap();
        ids.push(id);
    }
    let (busy, _) = night.append("the one still running");
    night.start(busy).unwrap();
    for round in 0..200 {
        night
            .note(busy, format!("asking the model, pass {round}"))
            .unwrap();
        store.append_queue(fat(&night.snapshot(), &answer)).unwrap();
    }
    let _ = ids;
    store
        .append(Role::User, "and what did you find out?")
        .unwrap();
    let logged = fs::metadata(&path).unwrap().len();
    assert!(
        logged > 16 * MIB,
        "the fixture is not a night of work: {logged} bytes"
    );

    let mut agent = pearl_agent::Agent::new(Queue::new(), Answerer, store, "mock")
        .with_settings(Settings::new("mock"));
    agent.queue.append("summarize the night");

    let before = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");
    agent.run().await.unwrap();
    let after = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");

    // The list is the surface's state. Asking a question reads the
    // conversation, and a conversation that copies every snapshot on its way
    // past pays for the whole night to answer one line.
    assert!(
        after.saturating_sub(before) < 4 * MIB,
        "asking one question cost {} MiB over a {} MiB log",
        after.saturating_sub(before) / MIB,
        logged / MIB
    );

    // And the answer the human was waiting for still arrives.
    let view = agent.queue.view();
    let done = view
        .iter()
        .find(|t| t.title == "summarize the night")
        .expect("the task is gone");
    assert_eq!(done.result.as_deref(), Some("here is what I found"));
    // Nothing was dropped from the file to pay for this.
    assert_eq!(fs::metadata(&path).unwrap().len() >= logged, true);
}
