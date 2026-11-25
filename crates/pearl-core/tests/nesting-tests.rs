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
    assert_eq!(q.next_queued(), None, "a paused part let the whole run without it");
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
    assert_eq!(q.next_queued(), Some(parent), "a failed part stranded the whole");
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
