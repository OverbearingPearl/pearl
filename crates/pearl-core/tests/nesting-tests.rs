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
