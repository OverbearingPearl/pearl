//! A paste is text, not a burst of keystrokes.
//!
//! Real terminals hand a paste over as one event once bracketed paste is on.
//! Without it every newline in the text is a Return, and one paste of a
//! paragraph places a task, commits whatever the next letter was, and leaves
//! the surface several tasks deep in a prompt nobody asked for.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{App, screen};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app() -> App {
    App::new(Queue::new())
}

#[test]
fn given_a_prompt_holding_a_word_when_a_three_line_paste_arrives_then_the_breaks_stay() {
    // A paste is one task however many lines it has: the draft keeps the
    // breaks the text was written with, because a second line is part of the
    // sentence and not a second Enter.
    let mut app = app();
    app.handle_key(key(KeyCode::Char('a')));

    app.paste("first line\nsecond\ta line\r\nthird");

    assert_eq!(
        app.prompt_line(),
        "append: first line\nsecond a line\nthird",
        "the paste was flattened onto one line"
    );
    let drawn = screen(&app, 60, 14);
    assert!(
        drawn.contains("append: first line") && drawn.contains("third"),
        "the paste is not on the screen:\n{drawn}"
    );

    app.handle_key(key(KeyCode::Enter));
    let nodes = app.queue.view();
    assert_eq!(nodes.len(), 1, "the paste became {} tasks", nodes.len());
    assert_eq!(nodes[0].title, "first line\nsecond a line\nthird");
    // One row, one line: the row opens with the first line and says the rest
    // is elsewhere. The frame is drawn again because the prompt that held the
    // paste is gone now.
    let after = screen(&app, 60, 14);
    let row = after
        .lines()
        .find(|l| l.contains("first line"))
        .unwrap_or_default();
    assert!(
        row.contains("…"),
        "the row hid the rest of the paste: {row:?}"
    );
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
