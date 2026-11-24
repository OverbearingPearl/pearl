//! Stories for choosing a model out of a list.
//!
//! Three candidates or fewer, the key cycles: a list would be slower than a
//! press. More than that, cycling is a chore and the human needs to see the
//! rows -- and once the rows are on screen, that is where adding, removing and
//! editing them belongs, not behind a grammar only the initiated know.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Queue, Settings};
use pearl_tui::{screen, App};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app_with(rows: usize) -> App {
    let s = Settings::new("deepseek-chat");
    for i in 1..rows {
        s.apply(&format!("add - model-{i}")).unwrap();
    }
    App::new(Queue::new()).with_settings(s)
}

#[rstest]
fn given_three_candidates_when_the_model_key_is_pressed_then_it_cycles_without_a_list() {
    let mut app = app_with(3);
    app.handle_key(key(KeyCode::Char('M')));
    assert!(app.picker.is_none(), "three rows opened a picker; a press is faster");
    assert_eq!(app.settings.model(), "model-1");
}

#[rstest]
fn given_four_candidates_when_the_model_key_is_pressed_then_the_rows_are_shown() {
    let mut app = app_with(4);
    let before = app.settings.model();
    app.handle_key(key(KeyCode::Char('M')));
    assert!(app.picker.is_some(), "four rows still cycled blind");
    assert_eq!(app.settings.model(), before, "opening the list changed the choice");
    let shown = screen(&app, 100, 24);
    assert!(shown.contains("model-2"), "the list does not name its rows: {shown}");
}

#[rstest]
fn given_an_open_list_when_a_row_is_chosen_then_it_is_in_force_and_the_list_closes() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.settings.model(), "model-2");
    assert!(app.picker.is_none(), "the list stayed open after the choice");
}

#[rstest]
fn given_an_open_list_when_a_row_is_dropped_then_it_is_gone_from_the_list() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('d')));
    let rows: Vec<String> = app.settings.candidates().iter().map(|c| c.model.clone()).collect();
    assert!(!rows.contains(&String::from("model-1")), "d did not drop the row: {rows:?}");
    assert!(app.picker.is_some(), "dropping a row closed the list");
}

#[rstest]
fn given_an_open_list_when_adding_then_the_prompt_arrives_already_started() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    assert!(app.prompt_line().starts_with("set: add "), "a did not start the line: {}", app.prompt_line());
}

#[rstest]
fn given_an_open_list_when_editing_then_the_prompt_carries_the_row_as_it_stands() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('e')));
    let line = app.prompt_line();
    assert!(line.contains("edit 2"), "e did not name the row: {line}");
    assert!(line.contains("model-1"), "e did not carry the row's model: {line}");
}

#[rstest]
fn given_an_open_list_when_it_is_dismissed_then_the_choice_is_untouched() {
    let mut app = app_with(5);
    let before = app.settings.model();
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Esc));
    assert!(app.picker.is_none());
    assert_eq!(app.settings.model(), before);
}

#[rstest]
fn given_a_candidate_on_another_endpoint_when_it_is_chosen_then_the_header_names_its_house() {
    let s = Settings::new("deepseek-chat");
    s.apply("add https://api.openai.com/v1 gpt-4o").unwrap();
    s.apply("choose 2").unwrap();
    let app = App::new(Queue::new()).with_settings(s).with_provider("deepseek");
    let line = app.settings_line();
    assert!(line.contains("openai/gpt-4o"), "the header still names the launch house: {line}");
}

#[rstest]
fn given_a_row_whose_key_cannot_be_found_when_the_list_is_shown_then_the_row_says_so() {
    let s = Settings::new("deepseek-chat");
    for i in 1..5 {
        s.apply(&format!("add https://h{i}.example/v1 model-{i}")).unwrap();
    }
    let mut app = App::new(Queue::new()).with_settings(s).with_key_probe(|_| false);
    app.handle_key(key(KeyCode::Char('M')));
    let shown = screen(&app, 100, 24);
    assert!(shown.contains("no key"), "a row with no key looks like one that has one: {shown}");
}
