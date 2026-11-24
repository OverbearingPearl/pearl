//! Stories for the flow surface: a flat view, badges you can scan, one cursor.
//!
//! Drawn at a size a person's terminal has: a six-row window cannot hold a
//! queue and a report, so a fixture that small tests the layout, not the view.

use pearl_core::{NodeState, Queue};
use pearl_tui::{badge, render_to_string};
use rstest::rstest;

#[rstest]
fn given_states_when_badged_then_each_has_a_distinct_glyph() {
    assert_ne!(badge(NodeState::Running), badge(NodeState::Done));
    assert_eq!(badge(NodeState::AwaitingInput), "?");
    assert_eq!(badge(NodeState::Paused), "‖");
}

#[rstest]
fn given_a_queue_when_rendered_then_titles_appear_in_the_view() {
    let mut queue = Queue::new();
    queue.append("write the parser");
    queue.append("ship the release");
    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("write the parser"));
    assert!(screen.contains("ship the release"));
}

#[rstest]
fn given_a_focused_node_when_rendered_then_the_cursor_marks_it() {
    let mut queue = Queue::new();
    let (id, _) = queue.append("the focused task");
    queue.focus_on(id).unwrap();
    let screen = render_to_string(&queue, 80, 24);
    // The focus cursor precedes the focused title's line.
    assert!(screen.contains("▶"));
    assert!(screen.contains("the focused task"));
}

#[rstest]
fn given_children_when_rendered_then_a_count_badge_shows() {
    let mut queue = Queue::new();
    let (parent, _) = queue.append("parent task");
    queue.add_child(parent, "child one").unwrap();
    queue.add_child(parent, "child two").unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("[2]"));
}

#[rstest]
fn given_a_narrowed_scope_when_rendered_then_the_title_says_so() {
    let mut queue = Queue::new();
    let (parent, _) = queue.append("parent");
    queue.add_child(parent, "child").unwrap();
    queue.narrow(parent).unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("in \"parent\""), "the heading did not name the place: {screen}");
    assert!(screen.contains("child"));
}
