//! Stories for live work: a running task says what it is doing right now.
//!
//! The queue holds the attention while the agent generates, and a badge alone
//! says only that something is happening. The runner notes the act itself.

use pearl_core::{NodeState, Queue};
use rstest::rstest;

#[rstest]
fn given_a_running_task_when_the_runner_notes_the_act_then_the_node_carries_it() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.note(id, "read src/parser.rs").unwrap();
    assert_eq!(
        queue.get(id).unwrap().activity.as_deref(),
        Some("read src/parser.rs")
    );
}

#[rstest]
fn given_a_running_task_when_the_runner_notes_again_then_the_newest_act_is_the_one_kept() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.note(id, "read src/parser.rs").unwrap();
    queue.note(id, "edit src/parser.rs").unwrap();
    assert_eq!(
        queue.get(id).unwrap().activity.as_deref(),
        Some("edit src/parser.rs")
    );
}

#[rstest]
fn given_a_task_that_has_not_started_when_noted_then_it_is_refused() {
    let queue = Queue::new();
    let (id, _) = queue.append("later");
    assert!(queue.note(id, "read a.txt").is_err());
    assert_eq!(queue.get(id).unwrap().state, NodeState::Queued);
}

#[rstest]
fn given_a_finished_task_when_the_last_act_is_read_then_the_report_is_still_the_reply() {
    let queue = Queue::new();
    let (id, _) = queue.append("answer");
    queue.start(id).unwrap();
    queue.note(id, "read a.txt").unwrap();
    queue.complete(id, "the reply").unwrap();
    let node = queue.get(id).unwrap();
    assert_eq!(node.result.as_deref(), Some("the reply"));
    assert_eq!(node.activity.as_deref(), Some("read a.txt"));
}
