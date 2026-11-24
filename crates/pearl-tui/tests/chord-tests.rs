//! Stories for chords that decode as letters.
//!
//! A terminal that speaks the kitty keyboard encoding sends `C-M-v` as one
//! event carrying a `Char('v')`, and a byte-level decode can hand the surface
//! a letter with a modifier it never asked for. Every plain arm in the normal
//! key table is a bare key, so a chord that arrives wearing a letter must not
//! be read as one: it opened the request screen, opened an input prompt, or
//! changed the keyboard underneath the human.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{screen, App, Mode};
use rstest::rstest;

fn app_with_task(title: &str) -> App {
    let queue = Queue::new();
    queue.append(title);
    App::new(queue)
}

fn chord(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[rstest]
#[case::emacs(KeyModifiers::CONTROL | KeyModifiers::ALT)]
#[case::alt(KeyModifiers::ALT)]
fn given_the_keyboard_chosen_when_a_chord_arrives_wearing_a_letter_then_the_letter_does_nothing(
    #[case] held: KeyModifiers,
) {
    for letter in ['i', 'a', 'v', 's', 'n', 't', 'e', 'h', 'q', 'z', 'S', 'm', 'w', 'T'] {
        let mut app = app_with_task("a task");
        app.settings.apply("keys plain").unwrap();
        app.handle_key(chord(KeyCode::Char(letter), held));
        assert!(
            matches!(app.mode, Mode::Normal),
            "'{letter}' with {held:?} opened an input prompt"
        );
        assert!(!app.read, "'{letter}' with {held:?} opened the request screen");
        assert!(!app.quit, "'{letter}' with {held:?} quit the surface");
        assert!(!app.undo_requested, "'{letter}' with {held:?} asked to undo");
        assert!(!app.save_requested, "'{letter}' with {held:?} asked to save");
        let drawn = screen(&app, 80, 20);
        assert!(drawn.contains("plain ("), "'{letter}' with {held:?} changed the keyboard:\n{drawn}");
    }
}

#[rstest]
fn given_the_emacs_keyboard_when_control_meta_v_arrives_then_the_report_pages_and_nothing_else_moves() {
    let mut app = app_with_task("a task");
    app.settings.apply("keys emacs").unwrap();
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(!app.read, "C-M-v opened the request screen");
    let drawn = screen(&app, 90, 20);
    assert!(drawn.contains("emacs ("), "C-M-v changed the keyboard:\n{drawn}");
}

#[rstest]
fn given_a_vim_keyboard_when_control_meta_v_arrives_then_the_request_screen_does_not_open() {
    let mut app = app_with_task("a task");
    app.settings.apply("keys vim").unwrap();
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(!app.read, "C-M-v opened the request screen in the vim keyboard");
}

#[rstest]
fn given_the_plain_keyboard_when_control_meta_v_arrives_then_the_request_screen_does_not_open() {
    let mut app = app_with_task("a task");
    app.settings.apply("keys plain").unwrap();
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(!app.read, "C-M-v opened the request screen in the plain keyboard");
}

#[rstest]
fn given_any_keyboard_when_control_c_arrives_then_the_surface_still_quits() {
    let mut app = app_with_task("a task");
    app.handle_key(chord(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit, "Ctrl-C no longer quits");
}

#[rstest]
fn given_an_open_prompt_when_a_control_chord_arrives_then_nothing_is_typed_into_it() {
    // A prompt takes text, not commands: `C-M-i` is a terminal's encoding of
    // one keystroke, and entering it as three letters leaves the human with a
    // line they did not write.
    let mut app = app_with_task("a task");
    app.settings.apply("keys emacs").unwrap();
    app.handle_key(chord(KeyCode::Char('i'), KeyModifiers::NONE));
    let _ = screen(&app, 80, 20);
    app.handle_key(chord(KeyCode::Char('i'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    app.handle_key(chord(KeyCode::Char('i'), KeyModifiers::CONTROL));
    let drawn = screen(&app, 80, 20);
    assert!(drawn.contains("insert now: _"), "a chord was typed into the prompt:\n{drawn}");
}
