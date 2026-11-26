//! Stories for the task queue. Each is a Given-When-Then: a fixture sets the
//! given, one action is the when, and the assertions are the then. The story is
//! the specification; it has to fail without the feature it names.

use pearl_core::{Event, NodeState, Position, Queue, QueueError, Request, Scheduler};
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
fn given_nothing_typed_when_a_task_is_placed_then_it_is_refused(mut empty: Queue) {
    let refused = empty.place(Request::task("   "), Position::Last);
    assert!(
        matches!(refused, Err(QueueError::Empty)),
        "an empty task was accepted: {refused:?}"
    );
    assert!(empty.view().is_empty(), "a row appeared for a task nobody asked for");
}

#[rstest]
fn given_nothing_said_when_a_remark_is_placed_then_it_is_refused(running: (Queue, pearl_core::TaskId)) {
    let (queue, id) = running;
    let refused = queue.place(Request::steering("", id), Position::After);
    assert!(matches!(refused, Err(QueueError::Empty)), "an empty remark was accepted: {refused:?}");
    assert!(queue.get(id).unwrap().steering.is_empty(), "an empty remark reached the task");
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
    let events = q.run(id).unwrap();
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

// --- the two editing verbs: take / place -----------------------------------

#[rstest]
fn given_queued_task_when_taken_and_placed_as_steering_then_it_annotates(running: (Queue, pearl_core::TaskId)) {
    let (mut q, target) = running;
    let (loose, _) = q.append("maybe later");
    // Disassemble the unexecuted node into a request, edit it in hand, and
    // re-place it as steering on the running task.
    let (mut request, _) = q.take(loose).unwrap();
    request.title = "use the new API".into();
    request.form = pearl_core::Form::Steering;
    request.target = Some(target);
    q.place(request, Position::Into(target)).unwrap();
    assert!(q.get(loose).is_none());
    let n = q.get(target).unwrap();
    assert_eq!(n.steering.len(), 1);
    assert_eq!(n.steering[0].text, "use the new API");
}

#[rstest]
fn given_two_queued_when_first_held_and_second_taken_placed_now_then_second_runs_first(mut empty: Queue) {
    let (a, _) = empty.append("A");
    let (b, _) = empty.append("B");
    empty.hold(a, "").unwrap();
    // Pull B out and re-place it at the front, into the gap A left.
    let (request, _) = empty.take(b).unwrap();
    let (b2, _) = empty.place(request, Position::Now).unwrap();
    assert_eq!(empty.next_queued(), Some(b2));
    assert!(empty.get(a).unwrap().held);
    assert_eq!(empty.get(a).unwrap().state, NodeState::Queued);
    empty.run(a).unwrap();
    assert!(!empty.get(a).unwrap().held);
}

#[rstest]
fn given_queued_task_when_held_then_scheduler_skips_it(mut empty: Queue) {
    let (a, _) = empty.append("held work");
    let (b, _) = empty.append("ready work");
    empty.hold(a, "").unwrap();
    assert_eq!(empty.next_queued(), Some(b));
    assert_eq!(empty.get(a).unwrap().state, NodeState::Queued);
    assert!(empty.get(a).unwrap().held);
}

#[rstest]
fn given_running_task_when_redirected_then_title_changes_and_work_kept(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    q.hold(id, "half done").unwrap();
    let events = q.redirect(id, "new objective").unwrap();
    let n = q.get(id).unwrap();
    assert_eq!(n.title, "new objective");
    assert_eq!(n.frame.as_ref().unwrap().progress, "half done");
    assert!(n.steering.iter().any(|s| s.text.contains("new objective")));
    assert!(events.iter().any(|e| matches!(e, Event::Redirected { id: i, .. } if *i == id)));
}

#[rstest]
fn given_queued_task_when_redirected_then_it_keeps_its_place_and_is_not_steered(mut empty: Queue) {
    let (first, _) = empty.append("first");
    let (second, _) = empty.append("second");
    empty.redirect(second, "new words").unwrap();
    let view = empty.view();
    assert_eq!(view.iter().map(|n| n.id).collect::<Vec<_>>(), vec![first, second]);
    let n = empty.get(second).unwrap();
    assert_eq!(n.title, "new words");
    assert!(n.steering.is_empty(), "a task that has not started has no loop to steer");
}

#[rstest]
fn given_done_task_when_redirected_then_it_is_refused_and_keeps_its_words(mut empty: Queue) {
    let (id, _) = empty.append("the words");
    empty.start(id).unwrap();
    empty.complete(id, "done").unwrap();
    let err = empty.redirect(id, "other words").unwrap_err();
    assert!(matches!(err, QueueError::WrongState { found: NodeState::Done, .. }));
    assert_eq!(empty.get(id).unwrap().title, "the words", "a refusal changed the task anyway");
}

#[rstest]
fn given_queue_when_placed_now_then_it_jumps_the_front(mut empty: Queue) {
    let (_first, _) = empty.append("first");
    let (urgent, _) = empty.place(Request::task("urgent"), Position::Now).unwrap();
    assert_eq!(empty.view().first().unwrap().id, urgent);
    assert_eq!(empty.next_queued(), Some(urgent));
}

#[rstest]
fn given_running_task_when_taken_then_rejected(running: (Queue, pearl_core::TaskId)) {
    let (mut q, id) = running;
    let err = q.take(id).unwrap_err();
    assert!(matches!(err, QueueError::WrongState { id: i, found: NodeState::Running, .. } if i == id));
}

#[rstest]
fn given_a_running_task_when_the_environment_says_what_it_did_then_the_line_survives_the_answer(
    running: (Queue, pearl_core::TaskId),
) {
    let (q, id) = running;
    q.notice(id, "sandbox: a write was refused").unwrap();
    q.complete(id, "the answer").unwrap();
    let n = q.get(id).unwrap();
    assert_eq!(
        n.notices,
        vec![String::from("sandbox: a write was refused")],
        "the report lost what happened to the task"
    );
    assert_eq!(n.result.as_deref(), Some("the answer"), "the answer was replaced by the news");
}

#[rstest]
fn given_a_finished_task_when_withdrawn_then_it_leaves_the_list() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "the answer").unwrap();
    q.withdraw(id).unwrap();
    assert_eq!(q.len(), 0, "the finished task is still on the list");
    assert!(q.get(id).is_none());
}

#[rstest]
fn given_a_running_task_when_withdrawn_then_it_is_refused() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    let refused = q.withdraw(id);
    assert!(refused.is_err(), "a running task was thrown away");
    assert_eq!(q.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_a_done_task_when_retried_then_it_is_queued_again_with_no_answer() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "the answer").unwrap();
    q.retry(id).unwrap();
    let node = q.get(id).unwrap();
    assert_eq!(node.state, NodeState::Queued);
    assert!(node.result.is_none(), "the old answer is still on the task");
}

#[rstest]
fn given_a_queued_task_when_retried_then_it_is_refused() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    assert!(q.retry(id).is_err(), "a queued task was retried");
    assert_eq!(q.get(id).unwrap().state, NodeState::Queued);
}

#[rstest]
fn given_a_done_task_when_a_child_is_nested_under_it_then_it_is_refused() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "the answer").unwrap();
    let refused = q.add_child(id, "a part");
    assert!(refused.is_err(), "a finished task grew a child");
    assert_eq!(q.len(), 1);
}

#[rstest]
fn given_a_list_with_work_in_it_when_it_is_snapshotted_and_restored_then_it_comes_back_held() {
    let mut q = Queue::new();
    let (a, _) = q.append("write the parser");
    let (b, _) = q.append("run the suite");
    q.start(a).unwrap();
    q.complete(a, "the parser is written").unwrap();
    let snapshot = q.snapshot();

    let fresh = Queue::new();
    let held = fresh.restore(&snapshot).expect("the snapshot did not read back");

    assert_eq!(fresh.len(), 2, "the list came back short");
    assert_eq!(held, 1, "the count of held tasks is wrong");
    assert_eq!(fresh.get(b).unwrap().state, NodeState::Queued);
    assert!(fresh.get(b).unwrap().held, "a restored task is ready to run");
    assert!(fresh.next_queued().is_none(), "a restored task was ready to run");
    assert_eq!(fresh.get(a).unwrap().state, NodeState::Done);
}

#[rstest]
fn given_a_task_still_running_when_it_is_restored_then_it_reads_as_waiting_not_as_working() {
    let mut q = Queue::new();
    let (a, _) = q.append("write the parser");
    q.start(a).unwrap();
    let snapshot = q.snapshot();

    let fresh = Queue::new();
    fresh.restore(&snapshot).unwrap();

    // Nothing is running it any more, and a row that still says it is being
    // answered is the screen lying about who is working.
    assert_eq!(fresh.get(a).unwrap().state, NodeState::Queued);
    assert!(fresh.get(a).unwrap().held);
}

#[rstest]
fn given_a_narrowed_topic_when_a_task_is_appended_then_it_lands_inside_the_topic() {
    let q = Queue::new();
    let (topic, _) = q.append("parser");
    q.narrow(topic).unwrap();

    let (id, _) = q.append("write the lexer");

    let inside: Vec<_> = q.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(inside, ["write the lexer"], "the words landed outside the screen");
    q.unnarrow();
    let at_root: Vec<_> = q.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(at_root, ["parser"], "the task escaped the topic: {at_root:?}");
    assert_eq!(q.get(id).unwrap().parent, Some(topic));
}

#[rstest]
fn given_a_narrowed_topic_when_a_task_is_inserted_now_then_it_is_first_inside_it() {
    let q = Queue::new();
    let (topic, _) = q.append("parser");
    q.add_child(topic, "second").unwrap();
    q.narrow(topic).unwrap();

    let (id, _) = q.place(pearl_core::Request::task("first"), pearl_core::Position::Now).unwrap();

    let order: Vec<_> = q.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(order, ["first", "second"], "insert now went past the topic");
    assert_eq!(q.get(id).unwrap().parent, Some(topic));
}

#[rstest]
fn given_a_cursor_on_a_child_when_place_after_arrives_then_it_lands_after_that_child() {
    let q = Queue::new();
    let (topic, _) = q.append("parser");
    let (first, _) = q.add_child(topic, "first").unwrap();
    q.add_child(topic, "second").unwrap();
    q.narrow(topic).unwrap();
    q.focus_on(first).unwrap();

    q.place(pearl_core::Request::task("between"), pearl_core::Position::After).unwrap();

    let order: Vec<_> = q.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(
        order,
        ["first", "between", "second"],
        "place after ignored a cursor inside a topic"
    );
}

#[rstest]
fn given_a_topic_when_the_runner_asks_what_is_next_then_the_topic_itself_is_never_offered() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();

    assert_eq!(q.next_queued(), None, "a topic was handed to the runner as work");
    assert_eq!(q.get(topic).unwrap().state, NodeState::Queued);
}

#[rstest]
fn given_a_topic_whose_parts_are_all_done_when_the_last_one_finishes_then_the_topic_is_done() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    let (a, _) = q.add_child(topic, "first").unwrap();
    let (b, _) = q.add_child(topic, "second").unwrap();

    q.start(a).unwrap();
    q.complete(a, "done").unwrap();
    assert_eq!(q.get(topic).unwrap().state, NodeState::Queued, "the topic closed early");

    q.start(b).unwrap();
    q.complete(b, "done").unwrap();
    assert_eq!(q.get(topic).unwrap().state, NodeState::Done);
}

#[rstest]
fn given_a_held_topic_when_the_runner_asks_what_is_next_then_nothing_inside_it_runs() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    q.add_child(topic, "write the lexer").unwrap();

    q.hold(topic, "later").unwrap();

    assert_eq!(q.next_queued(), None, "a paused topic kept handing out its parts");
}

#[rstest]
fn given_a_topic_with_two_unrun_parts_when_dissolve_arrives_then_they_stand_at_its_place() {
    let q = Queue::new();
    let (before, _) = q.place(pearl_core::Request::task("before"), pearl_core::Position::Last).unwrap();
    let (topic, _) = q.topic("parser").unwrap();
    q.add_child(topic, "lexer").unwrap();
    q.add_child(topic, "parser").unwrap();
    q.place(pearl_core::Request::task("after"), pearl_core::Position::Last).unwrap();

    q.dissolve(topic).unwrap();

    let order: Vec<_> = q.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(order, ["before", "lexer", "parser", "after"]);
    assert!(q.get(topic).is_none(), "the heading is still there");
    assert_eq!(q.view()[0].id, before);
}

#[rstest]
fn given_a_topic_whose_part_has_run_when_dissolve_arrives_then_it_is_refused_and_says_why() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    let (part, _) = q.add_child(topic, "lexer").unwrap();
    q.start(part).unwrap();

    let refused = q.dissolve(topic).unwrap_err();

    assert!(
        format!("{refused}").contains("a topic whose parts have not run"),
        "the refusal did not say why: {refused}"
    );
    assert_eq!(q.view()[0].title, "parser");
}
