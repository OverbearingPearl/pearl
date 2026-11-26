//! What the terminal actually sent, when a key report cannot be reproduced.
//!
//! A report like "typing pinyin floods the console with Returns" cannot be
//! answered from the code alone: either the terminal hands those keystrokes
//! over or it does not. `PEARL_EVENT_LOG` names a file, and every event the
//! surface read is written there one per line.

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use pearl_tui::describe_event;

#[test]
fn given_a_letter_when_the_event_is_described_then_the_line_names_code_and_modifiers() {
    let event = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert_eq!(describe_event(&event), "key Char('q') NONE");
}

#[test]
fn given_a_chord_when_the_event_is_described_then_both_modifiers_are_named() {
    let event = Event::Key(KeyEvent::new(
        KeyCode::Char('v'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ));
    assert_eq!(describe_event(&event), "key Char('v') CONTROL|ALT");
}

#[test]
fn given_a_return_when_the_event_is_described_then_it_says_so() {
    let event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(describe_event(&event), "key Enter NONE");
}

#[test]
fn given_pasted_text_when_the_event_is_described_then_the_count_is_written_not_the_text() {
    let event = Event::Paste(String::from("pinyin\nspelling"));
    assert_eq!(describe_event(&event), "paste 15 chars");
}

#[test]
fn given_a_mouse_event_when_it_is_described_then_the_cell_is_named() {
    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 12,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(describe_event(&event), "mouse Down(Left) 12,3");
}
