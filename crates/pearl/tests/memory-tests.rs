//! Stories for what a long run holds in memory.
//!
//! Every shell op keeps a before-image so it can be taken back, and the human
//! watches `mem` on the status line while a session runs. The promise is both
//! halves at once: the process does not grow with the number of commands, and
//! the oldest op is still revertible to the exact bytes it started from.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::TempDir;

const MIB: u64 = 1024 * 1024;

/// What the process holds, counted at the allocator.
///
/// `mem` on the status line is the kernel's figure, and a story that reads it
/// measures every other story in the same process too -- including pages the
/// allocator has freed and kept. Counting live bytes answers the question the
/// story is actually about: how much of a night does the window hold at once.
struct Held;

static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Held {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let at = unsafe { System.alloc(layout) };
        if !at.is_null() {
            let live =
                LIVE.fetch_add(layout.size() as u64, Ordering::SeqCst) + layout.size() as u64;
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        at
    }

    unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        unsafe { System.dealloc(at, layout) }
    }

    unsafe fn realloc(&self, at: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(at, layout, new) };
        if !moved.is_null() {
            LIVE.fetch_sub(layout.size() as u64, Ordering::SeqCst);
            let live = LIVE.fetch_add(new as u64, Ordering::SeqCst) + new as u64;
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        moved
    }
}

#[global_allocator]
static COUNTED: Held = Held;

/// Stories that read the process's memory read it for the whole process, so
/// they take turns: a story about memory may not race another story about
/// memory.
fn alone() -> std::sync::MutexGuard<'static, ()> {
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start a measurement: what is held now, and the high-water mark since.
fn held() -> u64 {
    let now = LIVE.load(Ordering::SeqCst);
    PEAK.store(now, Ordering::SeqCst);
    now
}

/// What the measurement grew by at its highest point.
fn grew(from: u64) -> u64 {
    PEAK.load(Ordering::SeqCst).saturating_sub(from)
}

#[rstest]
#[tokio::test]
async fn given_a_large_work_file_when_many_shell_ops_run_then_the_process_holds_no_copy_of_each() {
    let _turn = alone();
    let dir = TempDir::new().unwrap();
    // A workspace with one substantial file in it: an asset, a fixture, a data
    // file. Nothing here is build output, so a baseline keeps every byte.
    fs::write(dir.path().join("big.bin"), vec![b'x'; 8 * MIB as usize]).unwrap();
    fs::write(dir.path().join("work.txt"), "original").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let before = held();
    for i in 0..12 {
        let out = tools
            .call("bash", json!({"command": format!("echo {i} >> work.txt")}))
            .await;
        assert!(!out.is_error, "the command did not run: {}", out.content);
    }
    let after = grew(before);

    // Twelve ops over an 8 MiB file is 96 MiB if each keeps its own copy.
    assert!(
        after < 32 * MIB,
        "the ledger is holding a copy per op: mem grew {} MiB over 12 commands",
        after / MIB
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
    let _turn = alone();
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

    let before = held();
    agent.run().await.unwrap();
    let after = grew(before);

    // The list is the surface's state. Asking a question reads the
    // conversation, and a conversation that copies every snapshot on its way
    // past pays for the whole night to answer one line.
    assert!(
        after < 4 * MIB,
        "asking one question cost {} MiB over a {} MiB log",
        after / MIB,
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

/// A night that has been folded into one page: forty thousand turns of work a
/// summary already stands in for.
fn a_folded_night(path: &std::path::Path, body: &str) -> pearl_session::SessionStore {
    use pearl_session::{Coverage, Role, SessionStore};

    let mut store = SessionStore::open(path).unwrap();
    store.append(Role::Launch, String::new()).unwrap();
    for i in 0..3_000 {
        store.append(Role::Tool, format!("{i} {body}")).unwrap();
    }
    let last = store
        .read_for_view()
        .unwrap()
        .last()
        .map(|entry| entry.seq)
        .unwrap_or(1);
    store
        .append_compaction(
            String::from("the night folded into one page"),
            Coverage { first: 1, last },
        )
        .unwrap();
    store
}

/// A provider that remembers what it was asked, so the story can say what the
/// question saw as well as what it cost.
struct Note {
    seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl pearl_provider::Provider for Note {
    async fn complete(
        &self,
        request: pearl_provider::CompletionRequest,
    ) -> Result<pearl_provider::Completion, pearl_provider::ProviderError> {
        let words: Vec<String> = request.messages.iter().map(|m| m.content.clone()).collect();
        *self.seen.lock().unwrap() = words;
        Ok(pearl_provider::Completion::text("ok", "mock"))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_folded_night_when_the_next_question_is_asked_then_the_window_does_not_hold_it() {
    let _turn = alone();
    use pearl_core::{Queue, Settings};

    let dir = TempDir::new().unwrap();
    let store = a_folded_night(&dir.path().join("night.jsonl"), &"x".repeat(8 * 1024));
    let log = std::fs::metadata(dir.path().join("night.jsonl"))
        .unwrap()
        .len();
    assert!(log > 16 * MIB, "the night is not the size the story needs");

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let queue = Queue::new();
    let mut agent =
        pearl_agent::Agent::new(queue.clone(), Note { seen: seen.clone() }, store, "mock")
            .with_settings(Settings::new("mock"));

    let before = held();
    queue.append("what did the night come to?");
    agent.step().await.unwrap();
    let after = grew(before);

    // Twenty-four MiB of turns a summary already stands in for. Reading them
    // back to throw them away is the same mistake the list snapshots made.
    assert!(
        after < 8 * MIB,
        "one question over a folded night cost {} MiB",
        after / MIB
    );
    let asked = seen.lock().unwrap().join("\n");
    assert!(
        asked.contains("the night folded into one page"),
        "the question was asked without the summary that stands in for the night"
    );
}
#[rstest]
#[tokio::test]
async fn given_a_large_file_when_it_is_rewritten_many_times_then_the_process_holds_no_copy_of_each()
{
    let _turn = alone();
    let dir = TempDir::new().unwrap();
    let body = "line\n".repeat(400_000); // two megabytes
    fs::write(dir.path().join("big.txt"), &body).unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let before = held();
    for i in 0..20 {
        let out = tools
            .call(
                "write",
                json!({"path": "big.txt", "content": format!("{body}edit {i}\n")}),
            )
            .await;
        assert!(!out.is_error, "the write did not run: {}", out.content);
    }
    let after = grew(before);
    // Twenty writes of a two megabyte file is 40 MiB of before-images and
    // 40 MiB of after-images if each keeps both.
    assert!(
        after < 24 * MIB,
        "the ledger is holding a copy per write: mem grew {} MiB",
        after / MIB
    );

    // And the write before the last one still comes back, byte for byte.
    let ops = tools.ops();
    let (last, _) = ops.last().expect("nothing was recorded");
    tools
        .ledger()
        .revert(*last)
        .expect("the newest write could not be taken back");
    let now = fs::read_to_string(dir.path().join("big.txt")).unwrap();
    assert_eq!(
        now,
        format!("{body}edit 18\n"),
        "the bytes did not come back"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_night_of_tool_results_when_the_window_comes_back_then_the_list_comes_back_without_the_night()
 {
    let _turn = alone();
    use pearl_core::{Cost, Queue, Turn};
    use pearl_session::Role;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    let mut store = pearl::session(&path, false).unwrap();

    // A night as it is really written: a list that finished nine tasks, each
    // answer in the log where it happened, a bill for each round, and then
    // thousands of tool results on the way to the next one.
    let night = Queue::new();
    for i in 0..9 {
        let (id, _) = night.append(format!("task {i}"));
        night.start(id).unwrap();
        let answer = format!("answer {i}");
        night.complete(id, answer.clone()).unwrap();
        store.in_task(Some(id.to_string()));
        store.append(Role::Assistant, answer).unwrap();
        let bill = Turn::doing(Cost::turn(10, 5, 7), "asking the model");
        store
            .append(Role::Bill, serde_json::to_string(&bill).unwrap())
            .unwrap();
        store.in_task(None);
    }
    let body = "y".repeat(8192);
    for i in 0..3_000 {
        store.append(Role::Tool, format!("{i} {body}")).unwrap();
    }
    store.append_queue(night.snapshot()).unwrap();
    let logged = fs::metadata(&path).unwrap().len();
    assert!(
        logged > 16 * MIB,
        "the fixture is not a night of work: {logged} bytes"
    );

    // Coming back is a reading of the list, and the answer to every task in
    // it. It is not a reading of the night that produced them.
    let queue = Queue::new();
    let before = held();
    pearl::restore_queue(&store, &queue).unwrap();
    let after = grew(before);
    assert!(
        after < 4 * MIB,
        "coming back held {} MiB of a {} MiB night",
        after / MIB,
        logged / MIB
    );

    // And the list is really there, with what each row came to.
    let view = queue.view();
    let task = view
        .iter()
        .find(|t| t.title == "task 3")
        .expect("the list did not come back");
    assert_eq!(task.result.as_deref(), Some("answer 3"));
    assert_eq!(
        task.turns.len(),
        1,
        "the bill for the round did not come back"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_night_of_tool_results_when_the_window_opens_then_the_ground_is_set_without_the_night()
 {
    let _turn = alone();
    use pearl_session::Role;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("night.jsonl");
    let mut store = pearl::session(&path, false).unwrap();
    let body = "z".repeat(8192);
    for i in 0..3_000 {
        store.append(Role::Tool, format!("{i} {body}")).unwrap();
    }
    let logged = fs::metadata(&path).unwrap().len();
    assert!(
        logged > 16 * MIB,
        "the fixture is not a night of work: {logged} bytes"
    );

    let before = held();
    let said = pearl::ground(&store, dir.path(), Policy::WorkspaceWrite).unwrap();
    let after = grew(before);
    assert!(said, "the ground was not said on a log that never said it");
    assert!(
        after < 4 * MIB,
        "opening held {} MiB of a {} MiB night",
        after / MIB,
        logged / MIB
    );
}

#[rstest]
fn given_a_large_file_when_it_is_written_then_the_process_keeps_no_copy_of_it() {
    // A tool that writes has to read what it replaces, and an undo has to be
    // able to put it back. Both of those can hold one file for a moment. The
    // window holding it for the rest of the session is what this refuses.
    let _turn = alone();
    let dir = TempDir::new().unwrap();
    let old = "old line\n".repeat(800_000);
    let new = "new line\n".repeat(800_000);
    std::fs::write(dir.path().join("big.txt"), &old).unwrap();
    let mut ws = pearl_tools::Workspace::new(dir.path());
    let before = held();
    ws.write("big.txt", &new).unwrap();
    let kept = LIVE.load(Ordering::SeqCst).saturating_sub(before);
    assert!(
        kept < 2 * MIB,
        "writing kept a copy of the file it replaced: held {kept} bytes after"
    );
    ws.revert("big.txt").unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("big.txt")).unwrap(),
        old,
        "the undo came back with something other than what was there"
    );
}

/// A provider that does one round of shell work and then answers, so a story
/// can count what a row costs rather than guess.
struct Rower {
    said: AtomicU64,
}

impl pearl_provider::Provider for Rower {
    async fn complete(
        &self,
        _request: pearl_provider::CompletionRequest,
    ) -> Result<pearl_provider::Completion, pearl_provider::ProviderError> {
        // One round of shell work, then the answer: a row that costs a command
        // as well as a reply, which is the shape a real task has.
        if self.said.fetch_add(1, Ordering::SeqCst) % 2 == 0 {
            return Ok(pearl_provider::Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![pearl_provider::ToolCall {
                    id: "1".into(),
                    kind: "function".into(),
                    function: pearl_provider::Function {
                        name: "bash".into(),
                        arguments: r#"{"command":"echo one-more-line >> work.txt"}"#.into(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(pearl_provider::Completion::text(
            "I appended a line to work.txt and read what was there",
            "mock",
        ))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_list_of_work_when_it_all_runs_then_the_window_grows_with_the_rows_and_not_the_steps()
 {
    // The figure `mem` shows has a floor and a slope. The floor is the process;
    // the slope is the list's own text, because a step's bytes end up in the
    // log and on disk rather than in the window. A session of eighty rounds of
    // shell work grew under a tenth of a megabyte, so the slope is about a
    // kilobyte a row -- which is what this story pins, with room to spare.
    let _turn = alone();
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("work.txt"), "start\n").unwrap();
    let store = pearl_session::SessionStore::open(dir.path().join("pearl-session.jsonl")).unwrap();
    let queue = pearl_core::Queue::new();
    let mut agent = pearl_agent::Agent::new(
        queue.clone(),
        Rower {
            said: AtomicU64::new(0),
        },
        store,
        "mock",
    )
    .with_settings(pearl_core::Settings::new("mock"))
    .with_tools(dir.path(), Policy::WorkspaceWrite);
    for i in 0..40 {
        queue.append(format!("task {i}"));
    }
    let ran = agent.run().await.unwrap();
    assert_eq!(ran, 40, "the first forty rows did not all run");
    let before = held();
    for i in 40..80 {
        queue.append(format!("task {i}"));
    }
    let ran = agent.run().await.unwrap();
    assert_eq!(ran, 40, "the second forty rows did not all run");
    // What is held at the highest point of the forty rows, not only what
    // survives them: the request in flight is the largest transient, and the
    // story is that it does not grow with the number of rows behind it.
    let after = grew(before);
    assert!(
        after < 400 * 1024,
        "forty more rows cost {after} bytes, which is the steps and not the rows"
    );
}
