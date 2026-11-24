//! Stories for reading the work: the report is on the screen, not in a file.
//!
//! The queue is the object of attention while the agent generates, and what it
//! produced is what the human reads while shaping the next task. A badge alone
//! keeps the first half of that promise and drops the second.

use pearl_core::Queue;
use pearl_tui::render_to_string;
use rstest::rstest;

/// A queue holding one task that has finished with `reply` as its report.
fn finished(reply: &str) -> Queue {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    queue.complete(id, reply).unwrap();
    queue
}

#[rstest]
fn given_a_finished_task_when_the_surface_is_drawn_then_its_reply_is_on_the_screen() {
    let queue = finished("the parser now accepts a trailing comma");
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("the parser now accepts a trailing comma"),
        "the report never reached the eye:\n{screen}"
    );
}

#[rstest]
fn given_a_failed_task_when_the_surface_is_drawn_then_the_reason_is_on_the_screen() {
    let queue = Queue::new();
    let (id, _) = queue.append("call the model");
    queue.start(id).unwrap();
    queue.fail(id, "provider: connection refused").unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("connection refused"),
        "a failure with no reason on screen:\n{screen}"
    );
}

#[rstest]
fn given_a_reply_longer_than_the_pane_when_the_surface_is_drawn_then_its_newest_line_is_visible() {
    let mut reply = String::new();
    for n in 0..40 {
        reply.push_str(&format!("line {n}\n"));
    }
    reply.push_str("the last thing it said");
    let queue = finished(&reply);
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("the last thing it said"),
        "the newest line scrolled out of reach:\n{screen}"
    );
}

#[rstest]
fn given_a_task_that_has_not_run_when_the_surface_is_drawn_then_the_pane_says_so() {
    let queue = Queue::new();
    queue.append("not yet");
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("no report yet"),
        "an empty pane with nothing said:\n{screen}"
    );
}

#[rstest]
fn given_two_finished_tasks_when_the_focus_moves_then_the_pane_shows_the_focused_report() {
    let queue = Queue::new();
    let (first, _) = queue.append("first");
    queue.start(first).unwrap();
    queue.complete(first, "the first answer").unwrap();
    let (second, _) = queue.append("second");
    queue.start(second).unwrap();
    queue.complete(second, "the second answer").unwrap();
    queue.focus_on(second).unwrap();

    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("the second answer"), "{screen}");
    assert!(!screen.contains("the first answer"), "{screen}");
}
