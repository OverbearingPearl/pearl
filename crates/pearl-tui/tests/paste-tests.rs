//! A paste is text, not a burst of keystrokes.
//!
//! Real terminals hand a paste over as one event once bracketed paste is on.
//! Without it every newline in the text is a Return, and one paste of a
//! paragraph places a task, commits whatever the next letter was, and leaves
//! the surface several tasks deep in a prompt nobody asked for.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{screen, App};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app() -> App {
    App::new(Queue::new())
}

#[test]
fn given_a_prompt_holding_a_word_when_a_three_line_paste_arrives_then_it_is_one_task_on_one_line() {
    let mut app = app();
    app.handle_key(key(KeyCode::Char('a')));

    app.paste("first line\nsecond\ta line\r\nthird");

    let drawn = screen(&app, 60, 14);
    assert!(
        drawn.contains("append: first line second a line third_"),
        "the paste did not stay on the prompt's line:\n{drawn}"
    );

    app.handle_key(key(KeyCode::Enter));
    let nodes = app.queue.view();
    assert_eq!(nodes.len(), 1, "the paste became {} tasks", nodes.len());
    assert_eq!(nodes[0].title, "first line second a line third");
}

#[test]
fn given_nothing_prompting_when_a_paste_arrives_then_no_task_is_placed_and_no_prompt_opens() {
    let mut app = app();
    app.paste("a task\na second task\n");
    assert!(app.queue.view().is_empty(), "a paste placed a task");
    assert!(!app.is_prompting(), "a paste opened a prompt");
}

#[test]
fn given_a_prompt_when_a_paste_carries_no_end_of_line_then_it_is_taken_whole() {
    let mut app = app();
    app.handle_key(key(KeyCode::Char('a')));
    app.paste("https://api.deepseek.com/v1");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.queue.view()[0].title, "https://api.deepseek.com/v1");
}
