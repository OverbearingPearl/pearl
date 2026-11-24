//! Stories for the three settings keys.
//!
//! One rule for all three: the lowercase letter steps to the next value, the
//! uppercase letter opens the values to choose between. No count decides which
//! one a press means -- a rule that changes with the length of a list is a rule
//! the human has to test before every press.

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
#[case(2)]
#[case(3)]
#[case(4)]
#[case(7)]
fn given_any_number_of_candidates_when_the_lowercase_key_is_pressed_then_it_cycles_without_a_list(
    #[case] rows: usize,
) {
    let mut app = app_with(rows);
    app.handle_key(key(KeyCode::Char('m')));
    assert!(app.picker.is_none(), "{rows} rows opened a list; the lowercase letter steps");
    assert_eq!(app.settings.model(), "model-1");
}

#[rstest]
#[case(1)]
#[case(3)]
#[case(4)]
#[case(7)]
fn given_any_number_of_candidates_when_the_uppercase_key_is_pressed_then_the_rows_are_shown(
    #[case] rows: usize,
) {
    let mut app = app_with(rows);
    let before = app.settings.model();
    app.handle_key(key(KeyCode::Char('M')));
    assert!(app.picker.is_some(), "{rows} rows cycled blind; the uppercase letter chooses");
    assert_eq!(app.settings.model(), before, "opening the list changed the choice");
}

#[rstest]
fn given_a_window_when_the_lowercase_key_is_pressed_then_it_steps_and_the_uppercase_lists_the_sizes()
 {
    let mut app = app_with(2);
    let before = app.settings.window();
    app.handle_key(key(KeyCode::Char('w')));
    assert_ne!(app.settings.window(), before, "w did not step the window");
    assert!(app.picker.is_none(), "w opened a list");

    let stepped = app.settings.window();
    app.handle_key(key(KeyCode::Char('W')));
    assert!(app.picker.is_some(), "W did not open the sizes");
    assert_eq!(app.settings.window(), stepped, "opening the sizes changed the window");
    let shown = screen(&app, 100, 30);
    assert!(shown.contains("1000k") || shown.contains("1000000"), "the ladder's top is not on screen: {shown}");
}

#[rstest]
fn given_an_open_size_list_when_a_size_is_chosen_then_it_is_in_force() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('W')));
    // The ladder's last rung is the million; walking to it and taking it is
    // what the list is for.
    for _ in 0..Settings::windows().len() {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.settings.window(), 1_000_000);
    assert!(app.picker.is_none(), "choosing left the list open");
}

#[rstest]
fn given_an_effort_when_the_uppercase_key_is_pressed_then_the_rungs_are_listed() {
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('E')));
    assert!(app.picker.is_some(), "E did not open the rungs");
    let shown = screen(&app, 100, 30);
    assert!(shown.contains("medium"), "the rungs are not on screen: {shown}");
    assert!(shown.contains("auto"), "letting the endpoint decide is not a choice offered: {shown}");
}

#[rstest]
fn given_an_open_rung_list_when_a_rung_is_chosen_then_it_is_in_force() {
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('E')));
    // The list opens on auto, the value in force; the next rung is off.
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.settings.effort().map(|e| e.as_str()), Some("off"));
}

#[rstest]
fn given_a_house_with_no_rungs_when_the_rung_list_opens_then_it_offers_only_letting_the_endpoint_decide()
 {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('E')));
    let shown = screen(&app, 100, 30);
    assert!(shown.contains("auto"), "the one choice this house has is missing: {shown}");
    assert!(!shown.contains("medium"), "a rung this house cannot express was offered: {shown}");
}

#[rstest]
fn given_an_empty_hand_when_the_lowercase_effort_key_is_pressed_then_the_effort_steps() {
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('e')));
    assert_eq!(app.settings.effort().map(|e| e.as_str()), Some("off"));
}

#[rstest]
fn given_a_held_task_when_the_lowercase_effort_key_is_pressed_then_the_hand_is_edited_instead() {
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.queue.append("held");
    let id = app.queue.view()[0].id;
    app.queue.focus_on(id).unwrap();
    app.handle_key(key(KeyCode::Char('t')));
    app.handle_key(key(KeyCode::Char('e')));
    assert_eq!(app.settings.effort(), None, "editing a held task moved a setting instead");
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
