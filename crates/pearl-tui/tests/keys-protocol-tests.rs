//! Stories for the keys a terminal can send: ask for the enhanced encoding.
//!
//! A legacy terminal sends `Ctrl+v` and `Ctrl+Shift+v` as the same byte, so a
//! key that needs Shift under Control cannot arrive at all. The kitty encoding
//! puts the modifiers in the sequence instead. Asking for it is a stack the
//! surface pushes and must pop, or the human's terminal is left holding flags
//! long after the last frame.

use pearl_tui::{pop_key_flags, push_key_flags, restore};

/// What the terminal is told when the flags are pushed: `CSI > 1 u`.
const PUSHED: &str = "\x1b[>1u";
/// What it is told when the flags are popped: `CSI < 1 u`.
const POPPED: &str = "\x1b[<1u";

fn written(run: impl FnOnce(&mut Vec<u8>)) -> String {
    let mut out = Vec::new();
    run(&mut out);
    String::from_utf8(out).expect("the terminal was sent bytes that are not text")
}

#[test]
fn given_a_terminal_that_speaks_the_encoding_when_the_surface_asks_then_the_flags_are_pushed_once()
{
    let sent = written(|out| push_key_flags(out).expect("the flags did not push"));

    assert_eq!(
        sent.matches(PUSHED).count(),
        1,
        "the flags were sent {sent:?}"
    );
}

#[test]
fn given_the_flags_are_held_when_the_surface_stops_then_they_are_popped_once() {
    let sent = written(|out| pop_key_flags(out).expect("the flags did not pop"));

    assert_eq!(
        sent.matches(POPPED).count(),
        1,
        "the flags were sent {sent:?}"
    );
}

#[test]
fn given_the_flags_were_pushed_when_the_screen_is_restored_then_the_flags_are_popped_and_the_screen_is_left()
 {
    let mut out = Vec::new();
    restore(&mut out, true);
    let sent = String::from_utf8(out).expect("the terminal was sent bytes that are not text");

    assert!(
        sent.contains(POPPED),
        "the flags were left behind: {sent:?}"
    );
    assert!(
        sent.contains("\x1b[?1049l"),
        "the screen was left in the alternate buffer: {sent:?}"
    );
    assert!(
        sent.contains("\x1b[?2004l"),
        "paste stayed bracketed: {sent:?}"
    );
}

#[test]
fn given_the_flags_were_never_pushed_when_the_screen_is_restored_then_nothing_is_popped() {
    let mut out = Vec::new();
    restore(&mut out, false);
    let sent = String::from_utf8(out).expect("the terminal was sent bytes that are not text");

    assert!(
        !sent.contains(POPPED),
        "a pop was sent for a push that never happened: {sent:?}"
    );
    assert!(
        sent.contains("\x1b[?1049l"),
        "the screen was not left: {sent:?}"
    );
}
