//! Stories for the refused key: the surface says why nothing happened.
//!
//! A key that is offered and silently does nothing leaves the human guessing
//! whether the tool is broken or they are. The queue already knows the reason;
//! these stories require it to reach the eye.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{NodeState, Queue};
use pearl_tui::App;
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn press(app: &mut App, c: char) {
    app.handle_key(key(KeyCode::Char(c)));
}

/// A queue with one task in `state`.
fn one(state: NodeState) -> Queue {
    let queue = Queue::new();
    let (id, _) = queue.append("the task");
    match state {
        NodeState::Queued => {}
        NodeState::Running => {
            queue.start(id).unwrap();
        }
        NodeState::Done => {
            queue.start(id).unwrap();
            queue.complete(id, "done").unwrap();
        }
        other => panic!("fixture does not build {other:?}"),
    }
    queue
}

#[rstest]
fn given_a_settled_task_when_pause_is_pressed_then_the_surface_says_why_not() {
    let mut app = App::new(one(NodeState::Done));
    press(&mut app, ' ');
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("done") && notice.contains("queued or running"),
        "pause did nothing and said nothing: {notice:?}"
    );
}

#[rstest]
fn given_a_running_task_when_take_is_pressed_then_the_surface_says_why_not() {
    let mut app = App::new(one(NodeState::Running));
    press(&mut app, 't');
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("take"),
        "take did nothing and said nothing: {notice:?}"
    );
    assert!(app.hand.is_none());
}

#[rstest]
fn given_an_empty_queue_when_take_is_pressed_then_the_surface_says_there_is_nothing() {
    let mut app = App::new(Queue::new());
    press(&mut app, 't');
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("nothing"),
        "an empty queue swallowed the key: {notice:?}"
    );
}

#[rstest]
fn given_a_finished_task_when_take_is_pressed_then_it_lifts_from_the_list() {
    // A finished row can be moved: only work in flight is untouchable.
    let mut app = App::new(one(NodeState::Done));
    press(&mut app, 't');
    assert!(
        app.hand.is_some(),
        "a settled task could not be lifted: {:?}",
        app.notice
    );
}

#[rstest]
fn given_a_running_task_when_take_is_pressed_then_the_surface_says_which_state_it_needed() {
    let mut app = App::new(one(NodeState::Running));
    press(&mut app, 't');
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("take"),
        "a running task swallowed the key: {notice:?}"
    );
    assert!(
        notice.contains("finished"),
        "the refusal does not name the state: {notice:?}"
    );
}

#[rstest]
fn given_a_refused_key_when_the_next_key_works_then_the_notice_is_gone() {
    let mut app = App::new(one(NodeState::Done));
    press(&mut app, 's');
    assert!(app.notice.is_some());
    press(&mut app, 'z');
    assert!(app.notice.is_none(), "a stale complaint outlived its cause");
}

#[rstest]
fn given_a_notice_when_escape_is_pressed_then_it_is_dismissed() {
    let mut app = App::new(one(NodeState::Done));
    press(&mut app, 's');
    app.handle_key(key(KeyCode::Esc));
    assert!(app.notice.is_none());
}

#[rstest]
fn given_a_refused_key_when_the_screen_is_drawn_then_the_reason_is_on_it() {
    let mut app = App::new(one(NodeState::Done));
    press(&mut app, 's');
    let screen = pearl_tui::screen(&app, 80, 24);
    assert!(
        screen.contains("steer:") && screen.contains("needs"),
        "the reason never reached the screen:\n{screen}"
    );
}

#[rstest]
fn given_a_refused_key_when_the_notice_is_read_then_it_speaks_of_the_task_not_its_id() {
    let queue = one(NodeState::Done);
    let id = queue.view().first().unwrap().id;
    let mut app = App::new(queue);
    press(&mut app, ' ');
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        !notice.contains(&id.to_string()),
        "the notice quotes an id the human never sees: {notice}"
    );
    assert!(
        notice.contains("done"),
        "the state should read as a word, not a variant: {notice}"
    );
}
