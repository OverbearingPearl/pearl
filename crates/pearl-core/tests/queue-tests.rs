//! Stories for the task queue. Each is a Given-When-Then: a fixture sets the
//! given, one action is the when, and the assertions are the then. The story is
//! the specification; it has to fail without the feature it names.

use pearl_core::{NodeState, Queue, QueueError, Scheduler};
use rstest::{fixture, rstest};

/// Given an empty queue.
#[fixture]
fn empty() -> Queue {
    Queue::new()
}

/// Given a queue with one running task.
#[fixture]
fn running() -> (Queue, pearl_core::TaskId) {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    (q, id)
}

#[rstest]
fn given_empty_queue_when_append_then_a_queued_node_appears(mut empty: Queue) {
    let (id, events) = empty.append("refactor the loop");
    assert_eq!(empty.len(), 1);
    assert_eq!(empty.get(id).unwrap().state, NodeState::Queued);
    assert_eq!(empty.get(id).unwrap().title, "refactor the loop");
    assert!(matches!(events[0], pearl_core::Event::Appended { .. }));
}

#[rstest]
fn given_running_task_when_steer_then_it_waits_for_the_boundary(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    let events = q.steer(id, "also handle errors").unwrap();
    // Arrival is not injection: nothing is reported yet.
    assert!(events.is_empty());
    assert_eq!(q.get(id).unwrap().steering.len(), 1);
    assert_eq!(q.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_pending_steering_when_boundary_then_it_is_injected(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.steer(id, "also handle errors").unwrap();
    let scheduler = Scheduler::new();
    let events = scheduler.at_boundary(&mut q, id);
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], pearl_core::Event::SteeringInjected { text, .. } if text == "also handle errors"));
    assert!(q.get(id).unwrap().steering.is_empty());
}

#[rstest]
fn given_running_task_when_preempt_then_frame_saved_and_urgent_takes_focus(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    let (urgent, _) = q.preempt(id, "fix the build", "halfway through the parser").unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::Paused);
    assert_eq!(q.get(id).unwrap().frame.as_ref().unwrap().progress, "halfway through the parser");
    assert_eq!(q.focus(), Some(urgent));
    assert_eq!(q.get(urgent).unwrap().state, NodeState::Queued);
}

#[rstest]
fn given_paused_task_when_resume_then_running_with_frame_restored(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.preempt(id, "fix the build", "halfway").unwrap();
    let events = q.resume(id).unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::Running);
    assert!(events.iter().any(|e| matches!(e, pearl_core::Event::Resumed { .. })));
    assert_eq!(q.get(id).unwrap().frame.as_ref().unwrap().progress, "halfway");
}

#[rstest]
fn given_parent_with_child_when_parent_completes_first_then_it_waits(running: (Queue, pearl_core::TaskId)) {
    let (mut q, parent) = running;
    let (child, _) = q.add_child(parent, "write the tests").unwrap();
    q.start(child).unwrap();
    let events = q.complete(parent, "parser done").unwrap();
    // The parent recorded its result but cannot finish while the child runs.
    assert!(events.is_empty());
    assert_eq!(q.get(parent).unwrap().state, NodeState::Running);
    assert_eq!(q.get(parent).unwrap().result.as_deref(), Some("parser done"));
}

#[rstest]
fn given_waiting_parent_when_last_child_settles_then_parent_finishes(running: (Queue, pearl_core::TaskId)) {
    let (mut q, parent) = running;
    let (child, _) = q.add_child(parent, "write the tests").unwrap();
    q.start(child).unwrap();
    q.complete(parent, "parser done").unwrap();
    let events = q.complete(child, "tests done").unwrap();
    assert_eq!(q.get(child).unwrap().state, NodeState::Done);
    assert_eq!(q.get(parent).unwrap().state, NodeState::Done);
    assert!(events.iter().any(|e| matches!(e, pearl_core::Event::ChildSettled { .. })));
}

#[rstest]
fn given_running_task_when_ask_then_awaiting_and_answer_resumes(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.ask(id, "which file?").unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::AwaitingInput);
    assert_eq!(q.get(id).unwrap().question.as_deref(), Some("which file?"));
    q.answer(id, "src/main.rs").unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::Running);
    assert!(q.get(id).unwrap().question.is_none());
    assert_eq!(q.get(id).unwrap().steering.len(), 1);
}

#[rstest]
fn given_parent_with_child_when_cancel_parent_then_child_cancelled_too(running: (Queue, pearl_core::TaskId)) {
    let (mut q, parent) = running;
    let (child, _) = q.add_child(parent, "subtask").unwrap();
    q.start(child).unwrap();
    q.cancel(parent).unwrap();
    assert_eq!(q.get(parent).unwrap().state, NodeState::Cancelled);
    assert_eq!(q.get(child).unwrap().state, NodeState::Cancelled);
}

#[rstest]
fn given_failed_task_when_retry_then_back_to_queued(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.fail(id, "boom").unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::Failed);
    q.retry(id).unwrap();
    assert_eq!(q.get(id).unwrap().state, NodeState::Queued);
    assert!(q.get(id).unwrap().error.is_none());
}

#[rstest]
fn given_queued_task_when_steer_then_pending_steering_accepted(running: (Queue, pearl_core::TaskId)) {
    let (mut q, _) = running;
    let (queued, _) = q.append("not started");
    // The human may shape a task before it starts; the steering waits to drain.
    q.steer(queued, "also consider X").unwrap();
    assert_eq!(q.get(queued).unwrap().steering.len(), 1);
}

#[rstest]
fn given_done_task_when_steer_then_rejected(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.complete(id, "finished").unwrap();
    let err = q.steer(id, "too late").unwrap_err();
    assert!(matches!(err, QueueError::WrongState { .. }));
}

#[rstest]
fn given_queue_when_serialized_and_read_back_then_nodes_survive(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.steer(id, "remember this").unwrap();
    let json = serde_json::to_string(&q).unwrap();
    let back: Queue = serde_json::from_str(&json).unwrap();
    assert_eq!(back.len(), q.len());
    assert_eq!(back.get(id).unwrap().steering.len(), 1);
    assert_eq!(back.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_nested_tree_when_narrow_then_view_is_flat_children(running: (Queue, pearl_core::TaskId)) {
    let (mut q, parent) = running;
    let (child, _) = q.add_child(parent, "subtask").unwrap();
    assert_eq!(q.view().len(), 1); // top level: just the parent
    q.narrow(parent).unwrap();
    let view = q.view();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].id, child);
    q.unnarrow();
    assert_eq!(q.view()[0].id, parent);
}

#[rstest]
fn given_two_queued_when_dispatch_next_then_first_in_order_runs(mut empty: Queue) {
    let (first, _) = empty.append("first");
    let (second, _) = empty.append("second");
    let scheduler = Scheduler::new();
    let (id, _) = scheduler.dispatch_next(&mut empty).unwrap();
    assert_eq!(id, first);
    assert_eq!(empty.get(first).unwrap().state, NodeState::Running);
    assert_eq!(empty.get(second).unwrap().state, NodeState::Queued);
}
