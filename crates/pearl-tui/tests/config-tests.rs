//! Stories for turning a knob at the surface, while the work runs.
//!
//! The human notices the model is wrong mid-task. `:` opens a line, the setting
//! changes, and the screen says what is now in force -- no restart, no file.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Queue, Settings};
use pearl_tui::App;
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn set(app: &mut App, line: &str) {
    app.handle_key(key(KeyCode::Char(':')));
    for c in line.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
}

#[rstest]
fn given_a_surface_when_the_model_is_named_then_the_shared_settings_change() {
    let settings = Settings::new("deepseek-chat");
    let mut app = App::new(Queue::new()).with_settings(settings.clone());
    set(&mut app, "model deepseek-reasoner");
    assert_eq!(settings.model(), "deepseek-reasoner");
}

#[rstest]
fn given_a_surface_when_the_effort_is_named_then_the_shared_settings_change() {
    let settings = Settings::new("m");
    let mut app = App::new(Queue::new()).with_settings(settings.clone());
    set(&mut app, "effort high");
    assert_eq!(settings.effort().map(|e| e.as_str()), Some("high"));
}

#[rstest]
fn given_a_setting_that_does_not_parse_when_it_is_entered_then_the_notice_says_why() {
    let mut app = App::new(Queue::new());
    set(&mut app, "effort enormous");
    let notice = app.notice.clone().unwrap_or_default();
    assert!(notice.contains("effort"), "{notice}");
}

#[rstest]
fn given_a_surface_when_it_is_drawn_then_the_screen_names_what_is_in_force() {
    let mut app = App::new(Queue::new());
    set(&mut app, "window 64000");
    let screen = pearl_tui::screen(&app, 100, 24);
    assert!(screen.contains("64000"), "the window in force is invisible:\n{screen}");
    assert!(screen.contains("deepseek-chat"), "the model in force is invisible:\n{screen}");
}
