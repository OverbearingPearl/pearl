//! Stories for the mouse: a drag inside pearl's own window.
//!
//! The terminal offers two kinds of selection, and only one of them can see
//! where the window is. With the mouse left to the terminal, a drag that
//! starts inside pearl and ends at the top of the screen takes the shell's
//! scrollback with it -- the build log, the last command, the frame before
//! the window opened. So pearl claims the mouse and does its own selection,
//! and the terminal keeps Shift+drag for its own.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use pearl_core::Queue;
use pearl_core::Settings;
use pearl_tui::{clipboard_escape, copied, App, Drag};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn frame() -> Vec<String> {
    vec![
        String::from(" header "),
        String::from(" queue ───── "),
        String::from(" first task      "),
        String::from(" second task     "),
        String::from(" footer          "),
    ]
}

fn drag(from_row: u16, to_row: u16) -> Drag {
    Drag { from: (0, from_row), to: (7, to_row) }
}

#[rstest]
fn given_a_drag_down_the_window_when_the_button_is_released_then_the_lines_it_covered_are_taken() {
    let text = copied(&frame(), drag(2, 3)).expect("a two-line drag copied nothing");
    assert_eq!(text, " first task\n second task", "the drag took the wrong lines");
}

#[rstest]
fn given_a_drag_made_upwards_when_the_button_is_released_then_the_same_lines_are_taken() {
    // A drag is two corners, not a direction: taking the pointer back over a
    // line it already crossed is how a human changes their mind.
    let down = copied(&frame(), drag(2, 3)).expect("the downward drag copied nothing");
    let up = copied(&frame(), drag(3, 2)).expect("the upward drag copied nothing");
    assert_eq!(down, up);
}

#[rstest]
fn given_a_click_that_never_moved_when_the_button_is_released_then_nothing_is_taken() {
    assert_eq!(copied(&frame(), drag(2, 2)), None);
}

#[rstest]
fn given_a_frame_padded_to_the_window_width_when_a_line_is_taken_then_the_padding_is_not() {
    // Every line is written to the full width, so a copy that kept the
    // padding would paste a wall of spaces after every task.
    let text = copied(&frame(), drag(2, 3)).expect("nothing was taken");
    assert_eq!(text.lines().next(), Some(" first task"), "the line kept its padding: {text:?}");
}

#[rstest]
fn given_the_button_held_down_when_it_is_released_then_the_lines_can_be_taken_from_the_app() {
    let mut app = App::new(Queue::new()).with_settings(Settings::new("deepseek-chat"));
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 2);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 9, 4);
    let drag = app.drag.expect("the drag was forgotten while the button was down");
    app.handle_mouse(MouseEventKind::Up(MouseButton::Left), 9, 4);
    let text = copied(&frame(), drag).expect("the release copied nothing");
    assert_eq!(text, " first task\n second task\n footer");
    assert_eq!(copied(&frame(), app.drag.unwrap()), copied(&frame(), drag), "the mark moved on release");
}

#[rstest]
fn given_lines_taken_when_the_next_key_is_pressed_then_the_mark_is_gone() {
    let mut app = App::new(Queue::new()).with_settings(Settings::new("deepseek-chat"));
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 2);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 9, 4);
    app.handle_mouse(MouseEventKind::Up(MouseButton::Left), 9, 4);
    assert!(app.drag.is_some(), "there is no mark to clear");
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.drag, None, "the mark outlived the next key");
}

#[rstest]
fn given_text_when_it_is_handed_to_the_terminal_clipboard_then_the_escape_carries_the_bytes() {
    // OSC 52 is what every terminal with a clipboard understands, and it is
    // the only route out that needs no dependency and no display server.
    assert_eq!(clipboard_escape("hello"), "\u{1b}]52;c;aGVsbG8=\u{7}");
    assert_eq!(clipboard_escape("轮"), "\u{1b}]52;c;6L2u\u{7}");
    assert_eq!(clipboard_escape(""), "\u{1b}]52;c;\u{7}");
}
