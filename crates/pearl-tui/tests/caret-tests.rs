//! Where the terminal puts the input method's unconfirmed spelling.
//!
//! A terminal draws composition at the cursor. While nothing is being typed
//! the surface hides the cursor and the terminal has nothing to anchor to; a
//! prompt that leaves the cursor hidden invited the spelling to land at the
//! last column of the last row, and a character written past the last column
//! wraps the row and scrolls the screen.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{caret, screen, App};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn given_a_prompt_with_words_in_it_when_the_frame_is_drawn_then_the_caret_sits_after_them() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    app.paste("write a function");

    let drawn = screen(&app, 60, 14);
    let line = drawn
        .lines()
        .last()
        .expect("a bottom row")
        .trim_end()
        .to_string();
    let (at, row) = caret(&app, 60, 14).expect("a caret while typing");
    assert_eq!(row, 13, "the caret is not on the prompt's row:\n{drawn}");
    assert_eq!(
        at as usize,
        line.chars().count(),
        "the caret is not where the typing ends: {line:?}"
    );
}

#[test]
fn given_a_prompt_filled_to_the_glass_when_it_is_drawn_then_one_column_stays_empty() {
    // The column is reserved so the first character of a composition has
    // somewhere to land that is not past the end of the row.
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    app.paste(&"x".repeat(200));
    let drawn = screen(&app, 40, 12);
    let bottom = drawn.lines().last().unwrap_or_default().trim_end();
    assert!(
        bottom.chars().count() < 40,
        "the prompt filled the whole row, so a composition would wrap it"
    );
}

#[test]
fn given_nothing_prompting_when_the_frame_is_drawn_then_no_caret_is_placed() {
    let app = App::new(Queue::new());
    let _ = screen(&app, 40, 12);
    assert_eq!(caret(&app, 40, 12), None, "a hidden cursor is anchored nowhere");
}
