//! Stories for nesting: a task's children are its decomposition.
//!
//! `add_child` existed and the scheduler only walked roots, so a child nested
//! under a queued parent was work nobody would ever pick up. Nesting has to
//! mean something the runner does: the parts run, then the whole.

use pearl_core::{NodeState, Queue};
use rstest::rstest;

#[rstest]
fn given_a_parent_with_a_child_when_the_next_task_is_chosen_then_the_child_goes_first() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    assert_eq!(q.next_queued(), Some(child));
}

#[rstest]
fn given_a_child_that_settled_when_the_next_task_is_chosen_then_the_parent_goes() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.start(child).unwrap();
    q.complete(child, "found it").unwrap();
    assert_eq!(q.next_queued(), Some(parent));
}

#[rstest]
fn given_two_children_when_the_work_is_taken_in_order_then_both_parts_precede_the_whole() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (a, _) = q.add_child(parent, "part a").unwrap();
    let (b, _) = q.add_child(parent, "part b").unwrap();
    let mut order = Vec::new();
    while let Some(id) = q.next_queued() {
        order.push(id);
        q.start(id).unwrap();
        q.complete(id, "done").unwrap();
    }
    assert_eq!(order, vec![a, b, parent]);
}

#[rstest]
fn given_a_grandchild_when_the_next_task_is_chosen_then_the_deepest_part_goes_first() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    let (grand, _) = q.add_child(child, "a detail").unwrap();
    assert_eq!(q.next_queued(), Some(grand));
}

#[rstest]
fn given_a_held_child_when_the_next_task_is_chosen_then_the_parent_waits() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.hold(child, "paused by hand").unwrap();
    assert_eq!(
        q.next_queued(),
        None,
        "a paused part let the whole run without it"
    );
    q.run(child).unwrap();
    assert_eq!(q.next_queued(), Some(child));
}

#[rstest]
fn given_a_failed_child_when_the_next_task_is_chosen_then_the_parent_still_goes() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.start(child).unwrap();
    q.fail(child, "could not").unwrap();
    assert_eq!(
        q.next_queued(),
        Some(parent),
        "a failed part stranded the whole"
    );
    assert_eq!(q.get(child).unwrap().state, NodeState::Failed);
}

#[rstest]
fn given_a_running_child_when_the_next_task_is_chosen_then_the_parent_waits() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.start(child).unwrap();
    assert_eq!(q.next_queued(), None);
}

#[rstest]
fn given_two_roots_each_with_a_child_when_the_work_is_taken_then_each_whole_follows_its_parts() {
    let q = Queue::new();
    let (p1, _) = q.append("whole one");
    let (c1, _) = q.add_child(p1, "part one").unwrap();
    let (p2, _) = q.append("whole two");
    let (c2, _) = q.add_child(p2, "part two").unwrap();
    let mut order = Vec::new();
    while let Some(id) = q.next_queued() {
        order.push(id);
        q.start(id).unwrap();
        q.complete(id, "done").unwrap();
    }
    assert_eq!(order, vec![c1, p1, c2, p2]);
}

#[rstest]
fn given_a_topic_brought_back_held_when_a_task_is_added_to_it_then_it_runs() {
    // A resumed session brings every open topic back held, so nothing spends
    // money on its own. Work the human then puts into that topic is the hand
    // saying run it -- otherwise the new task waits for a start nobody knows.
    let queue = Queue::new();
    let (topic, _) = queue.topic("parser").unwrap();
    queue.hold(topic, "after lunch").unwrap();

    let (child, _) = queue
        .place(
            pearl_core::Request::task("write the lexer"),
            pearl_core::Position::Into(topic),
        )
        .unwrap();

    assert_eq!(queue.next_queued(), Some(child), "the new part never ran");
    assert!(!queue.get(topic).unwrap().held, "the topic is still stopped");
}

#[rstest]
fn given_a_part_lifted_out_of_a_topic_when_it_is_made_a_topic_then_it_becomes_one() {
    // The lift detaches the node but used to leave its parent behind, so `A`
    // answered "needs a root task" about a row the human had just taken out.
    // Out of the topic, the part is a row of its own and can lead one.
    let queue = Queue::new();
    let (topic, _) = queue.topic("parser").unwrap();
    let (part, _) = queue
        .place(
            pearl_core::Request::task("write the lexer"),
            pearl_core::Position::Into(topic),
        )
        .unwrap();
    queue.take(part).unwrap();

    queue.promote(part).unwrap();

    assert_eq!(queue.get(part).unwrap().form, pearl_core::Form::Topic);
    assert_eq!(queue.get(topic).unwrap().children.len(), 0);
}

#[rstest]
fn given_a_row_lifted_from_the_middle_when_it_is_put_back_then_it_returns_to_its_place() {
    // A mis-pressed `t` must not cost a place: the queue remembers where the
    // row stood and gives it back, its parts with it.
    let queue = Queue::new();
    queue.append("first");
    let (second, _) = queue.append("second");
    queue.append("third");
    let before: Vec<_> = queue.view().into_iter().map(|n| n.id).collect();
    queue.take(second).unwrap();
    assert!(!queue.view().iter().any(|n| n.id == second), "the row did not lift");

    queue.put_back(second).unwrap();

    let after: Vec<_> = queue.view().into_iter().map(|n| n.id).collect();
    assert_eq!(after, before, "the row did not return to its place");
    assert_eq!(queue.focus(), Some(second), "the cursor did not follow it back");
}

/// A topic is as finished as its parts are: put work into one that has closed
/// and the row opens again, instead of saying "done" over a waiting task.
#[test]
fn given_a_finished_topic_when_work_is_placed_into_it_then_the_topic_opens_again() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    let (part, _) = q.add_child(topic, "write the lexer").unwrap();
    q.start(part).unwrap();
    q.complete(part, "ok").unwrap();
    assert_eq!(q.get(topic).unwrap().state, NodeState::Done, "the topic did not close");

    q.add_child(topic, "write the parser").unwrap();

    assert_eq!(q.get(topic).unwrap().state, NodeState::Queued, "the topic still reads done");
    assert!(q.next_queued().is_some(), "nothing is ready to run");
}

/// Where a waiting row is put decides when it runs, and the running row is not
/// interrupted: put in front of it, the new row is the next one taken.
#[test]
fn given_a_running_row_when_a_queued_row_is_placed_in_front_then_the_new_row_is_taken_next() {
    let q = Queue::new();
    let (running, _) = q.append("long one");
    let (waiting, _) = q.append("short one");
    q.start(running).unwrap();
    q.hold(waiting, "").unwrap();

    q.take(waiting).unwrap();
    q.place_node(waiting, pearl_core::Position::Now).unwrap();
    let _ = q.run(waiting);

    assert_eq!(q.next_queued(), Some(waiting), "the row put in front is not taken next");
    assert_eq!(q.get(running).unwrap().state, NodeState::Running, "the running row was interrupted");
}

/// What the model is handed instead of another line of work's conversation:
/// the rows, the one sentence each finished task concluded in, and the work
/// still waiting -- including the work it must not take.
#[test]
fn given_a_finished_topic_and_a_waiting_row_when_the_digest_is_read_then_both_are_named() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    let (lexer, _) = q.add_child(topic, "write the lexer").unwrap();
    q.start(lexer).unwrap();
    q.complete(lexer, "tokens come out of the lexer").unwrap();
    q.append("write the docs");

    let said = q.digest(None);

    assert!(said.contains("▸ parser (1/1) — write the lexer: tokens come out"), "the topic is not named with its conclusion: {said}");
    assert!(said.contains("· write the docs"), "the waiting row is missing: {said}");
}

/// A failed task says so in the digest, and a long topic is counted rather
/// than printed: the list is a page, not the whole window.
#[test]
fn given_a_failed_task_and_a_long_topic_when_the_digest_is_read_then_both_are_summarised() {
    let q = Queue::new();
    let (bad, _) = q.append("break the build");
    q.start(bad).unwrap();
    q.fail(bad, "the compiler said no").unwrap();

    let (topic, _) = q.topic("parser").unwrap();
    for i in 0..10 {
        let _ = q.add_child(topic, format!("part {i}"));
    }

    let said = q.digest(Some(bad));
    assert!(!said.contains("break the build"), "the row read itself: {said}");

    let other = q.digest(None);
    assert!(other.contains("✗ break the build"), "the failure is not named: {other}");
    assert!(other.contains("▸ parser (0/10)"), "the count is wrong: {other}");
    assert!(!other.contains("part 8"), "a topic outside the line should be counted, not printed: {other}");
}

/// The row in flight is not told what it is about to do, and its parts are not
/// its business either: the digest leaves that whole subtree out.
#[test]
fn given_a_task_with_parts_in_flight_when_the_digest_is_read_then_it_and_its_parts_are_left_out() {
    let q = Queue::new();
    let (busy, _) = q.append("write the parser");
    let _ = q.add_child(busy, "read the tokens");
    q.append("write the docs");

    let said = q.digest(Some(busy));

    assert!(!said.contains("write the parser"), "the row read itself: {said}");
    assert!(!said.contains("read the tokens"), "the row read its own parts: {said}");
    assert!(said.contains("write the docs"), "the other row is missing: {said}");
}
