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
    settings.apply("dialect openai").unwrap();
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

#[rstest]
fn given_a_write_behind_you_when_undo_is_pressed_then_the_surface_asks_for_it() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('z')));
    assert!(app.undo_requested, "the undo key did nothing at all");
}

#[rstest]
fn given_an_undo_that_worked_when_it_settles_then_the_surface_says_what_went_back() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('z')));
    app.settle_undo(Ok(String::from("undone op 2 (echo x > x.txt): 1 path(s) restored")));
    assert!(!app.undo_requested);
    let screen = pearl_tui::screen(&app, 90, 24);
    assert!(screen.contains("undone op 2"), "{screen}");
}

#[rstest]
fn given_nothing_to_undo_when_it_settles_then_the_surface_says_so() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('z')));
    app.settle_undo(Ok(String::new()));
    let notice = app.notice.clone().unwrap_or_default();
    assert!(notice.contains("nothing"), "{notice}");
}

#[rstest]
fn given_a_window_partly_full_when_the_screen_is_drawn_then_the_fill_is_on_it() {
    let gauge = pearl_core::Gauge::new();
    gauge.spent(48_000);
    gauge.folded();
    let app = App::new(Queue::new()).with_gauge(gauge);
    let screen = pearl_tui::screen(&app, 110, 24);
    assert!(screen.contains("ctx 48k/128k"), "the window's fill was invisible:\n{screen}");
    assert!(screen.contains("summarized 1x"), "history was summarized and the screen hid it:\n{screen}");
}

#[rstest]
fn given_a_reading_with_a_key_when_the_status_is_shown_then_the_key_is_named_beside_it() {
    let app = App::new(Queue::new());
    let line = app.settings_line();
    for pair in [("deepseek-chat", "M"), ("think", "E"), ("ctx", "W")] {
        let at = line.find(pair.0).unwrap_or_else(|| panic!("{} missing: {line}", pair.0));
        let after = &line[at..(at + 40).min(line.len())];
        assert!(after.contains(pair.1), "no key named beside {}: {line}", pair.0);
    }
}

#[rstest]
fn given_the_status_when_it_is_shown_then_the_window_is_not_said_twice() {
    let app = App::new(Queue::new());
    let line = app.settings_line();
    assert!(!line.contains("window"), "the window is stated twice: {line}");
}

#[rstest]
fn given_the_model_key_when_it_is_pressed_then_the_next_model_is_in_force() {
    let mut app = App::new(Queue::new());
    app.settings.apply("add - deepseek-reasoner").unwrap();
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('M')));
    assert_eq!(app.settings.model(), "deepseek-reasoner");
    app.handle_key(key(KeyCode::Char('E')));
    assert_eq!(app.settings.effort().map(|e| e.as_str()), Some("off"));
    let before = app.settings.window();
    app.handle_key(key(KeyCode::Char('W')));
    assert_ne!(app.settings.window(), before);
}

#[rstest]
fn given_edited_knobs_when_the_save_key_is_pressed_then_the_surface_asks_for_one_save() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('E')));
    app.handle_key(key(KeyCode::Char('W')));
    assert!(!app.save_requested, "saving happened without being asked");
    app.handle_key(key(KeyCode::Char('S')));
    assert!(app.save_requested, "one key must save every knob, not one key per knob");
}

#[rstest]
fn given_a_save_that_worked_when_it_settles_then_the_surface_says_where_it_went() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('S')));
    app.settle_save(Ok(String::from("/home/me/.pearl/config")));
    assert!(!app.save_requested);
    assert!(app.notice.clone().unwrap_or_default().contains(".pearl/config"));
}

#[rstest]
fn given_a_dialect_that_was_stated_when_the_header_is_drawn_then_it_names_it() {
    let mut app = App::new(Queue::new());
    let plain = app.settings_items().join(" | ");
    assert!(!plain.contains("silent"), "a derived default was reported as news: {plain}");
    app.settings.apply("dialect silent").unwrap();
    let stated = app.settings_items().join(" | ");
    assert!(stated.contains("silent"), "a stated dialect is why the ladder is short: {stated}");
}
