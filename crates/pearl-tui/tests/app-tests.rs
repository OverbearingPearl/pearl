//! Stories for the interactive surface. Each drives the pure key handler with
//! synthetic keys and asserts on the queue, the mode, or the hand. The story is
//! the specification: it has to fail without the feature it names.

use crossterm::event::KeyCode;
use pearl_core::{NodeState, Queue};
use pearl_tui::{key, App, Mode};
use rstest::{fixture, rstest};

/// Given a surface over an empty queue.
#[fixture]
fn surface() -> App {
    App::new(Queue::new())
}

/// Given a surface with one queued task focused.
#[fixture]
fn one_queued() -> App {
    let mut q = Queue::new();
    q.append("write the parser");
    App::new(q)
}

#[rstest]
fn given_empty_surface_when_append_typed_then_a_task_appears(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    assert!(matches!(surface.mode, Mode::Input(_)));
    for c in "new task".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    assert_eq!(surface.mode, Mode::Normal);
    assert_eq!(surface.queue.len(), 1);
    assert_eq!(surface.queue.view()[0].title, "new task");
}

#[rstest]
fn given_input_when_escape_then_back_to_normal_without_commit(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    surface.handle_key(key(KeyCode::Char('x')));
    surface.handle_key(key(KeyCode::Esc));
    assert_eq!(surface.mode, Mode::Normal);
    assert_eq!(surface.queue.len(), 0);
}

#[rstest]
fn given_queued_task_when_taken_then_it_leaves_the_queue_for_the_hand(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    assert_eq!(one_queued.queue.len(), 0);
    assert!(one_queued.hand.is_some());
    assert_eq!(one_queued.hand.as_ref().unwrap().title, "write the parser");
    assert!(one_queued.prompt_line().contains("in hand"));
}

#[rstest]
fn given_hand_when_edited_and_placed_now_then_it_returns_at_the_front(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    one_queued.handle_key(key(KeyCode::Char('e')));
    // Clear the prefilled title and type a new one.
    for _ in 0.."write the parser".len() {
        one_queued.handle_key(key(KeyCode::Backspace));
    }
    for c in "rewritten".chars() {
        one_queued.handle_key(key(KeyCode::Char(c)));
    }
    one_queued.handle_key(key(KeyCode::Enter));
    one_queued.handle_key(key(KeyCode::Char('P')));
    assert!(one_queued.hand.is_none());
    assert_eq!(one_queued.queue.len(), 1);
    assert_eq!(one_queued.queue.view()[0].title, "rewritten");
}

#[rstest]
fn given_two_tasks_when_first_held_then_second_dispatches(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    for c in "A".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    surface.handle_key(key(KeyCode::Char('a')));
    surface.handle_key(key(KeyCode::Char('B')));
    surface.handle_key(key(KeyCode::Enter));
    // Focus is on A (the first); hold it.
    surface.queue.focus_on(surface.queue.view()[0].id).unwrap();
    surface.handle_key(key(KeyCode::Char('h')));
    let a = surface.queue.view()[0].id;
    assert!(surface.queue.get(a).unwrap().held);
    assert_eq!(surface.queue.next_queued(), Some(surface.queue.view()[1].id));
}

#[rstest]
fn given_running_task_when_steered_then_annotation_waits(mut one_queued: App) {
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.handle_key(key(KeyCode::Char('s')));
    for c in "also handle errors".chars() {
        one_queued.handle_key(key(KeyCode::Char(c)));
    }
    one_queued.handle_key(key(KeyCode::Enter));
    assert_eq!(one_queued.queue.get(id).unwrap().steering.len(), 1);
    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_surface_when_quit_then_quit_flag_set(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('q')));
    assert!(surface.quit);
}

#[rstest]
fn given_two_tasks_when_down_is_pressed_then_focus_moves_down(mut surface: App) {
    surface.queue.append("first");
    surface.queue.append("second");
    surface.queue.focus_on(surface.queue.view()[0].id).unwrap();
    surface.handle_key(key(KeyCode::Down));
    assert_eq!(surface.queue.focus(), Some(surface.queue.view()[1].id));
}
