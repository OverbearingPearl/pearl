//! Stories for telling the thinking from the answer.
//!
//! Thinking is not the answer and must not read like one. It is folded away by
//! default -- the answer is what the human came for -- and one key opens it.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{App, screen};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn answered() -> App {
    let queue = Queue::new();
    let (id, _) = queue.append("a question");
    queue.start(id).unwrap();
    queue.thought(id, "WEIGHING-THE-OPTIONS").unwrap();
    queue.complete(id, "THE-ANSWER").unwrap();
    App::new(queue)
}

#[rstest]
fn given_a_task_that_thought_when_the_report_is_shown_then_the_answer_is_what_reads() {
    let app = answered();
    let screen = screen(&app, 70, 20);
    assert!(screen.contains("THE-ANSWER"), "{screen}");
    assert!(
        !screen.contains("WEIGHING-THE-OPTIONS"),
        "the thinking was not folded away:\n{screen}"
    );
}

#[rstest]
fn given_a_folded_thought_when_the_report_is_shown_then_it_says_there_is_one() {
    let app = answered();
    let screen = screen(&app, 70, 20);
    assert!(
        screen.contains("thinking"),
        "a folded thought nobody knows about:\n{screen}"
    );
}

#[rstest]
fn given_a_folded_thought_when_it_is_opened_then_both_read_and_are_labelled() {
    let mut app = answered();
    app.handle_key(key(KeyCode::Char('T')));
    let screen = screen(&app, 70, 20);
    assert!(
        screen.contains("WEIGHING-THE-OPTIONS"),
        "the thought did not open:\n{screen}"
    );
    assert!(
        screen.contains("THE-ANSWER"),
        "opening the thought hid the answer:\n{screen}"
    );
    let thought_at = screen.find("WEIGHING").unwrap();
    let answer_at = screen.find("THE-ANSWER").unwrap();
    assert!(
        thought_at < answer_at,
        "the thinking must read before the answer:\n{screen}"
    );
}

#[rstest]
fn given_an_opened_thought_when_the_key_is_pressed_again_then_it_folds_back() {
    let mut app = answered();
    app.handle_key(key(KeyCode::Char('T')));
    app.handle_key(key(KeyCode::Char('T')));
    assert!(!screen(&app, 70, 20).contains("WEIGHING"));
}

#[rstest]
fn given_a_task_that_never_thought_when_the_key_is_pressed_then_it_says_so() {
    let queue = Queue::new();
    let (id, _) = queue.append("plain");
    queue.start(id).unwrap();
    queue.complete(id, "just an answer").unwrap();
    let mut app = App::new(queue);
    app.handle_key(key(KeyCode::Char('T')));
    let notice = app.notice.clone().unwrap_or_default();
    assert!(notice.contains("no thinking"), "{notice}");
}
