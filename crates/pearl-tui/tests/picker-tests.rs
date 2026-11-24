//! Stories for the three settings keys.
//!
//! One rule for all three: the lowercase letter steps to the next value, the
//! uppercase letter opens the values to choose between. No count decides which
//! one a press means -- a rule that changes with the length of a list is a rule
//! the human has to test before every press.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Queue, Settings};
use pearl_tui::{screen, App, Mode};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Empty the prompt: a row's fields arrive filled in, and a story that types
/// its own must say so.
fn clear(app: &mut App) {
    for _ in 0..80 {
        app.handle_key(key(KeyCode::Backspace));
    }
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
    let line = app.prompt_line();
    assert!(line.starts_with("add model: endpoint + id"), "a did not start a two-field line: {line}");
    assert!(line.ends_with(": _"), "the line arrived with words in it: {line}");
}

#[rstest]
fn given_an_open_list_when_editing_then_the_prompt_carries_the_row_as_it_stands() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('e')));
    let line = app.prompt_line();
    assert!(line.contains("edit model 2"), "e did not name the row: {line}");
    assert!(line.ends_with("- model-1_"), "e did not carry the two fields: {line}");
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

#[rstest]
fn given_the_model_list_when_a_is_pressed_then_the_typing_line_is_what_the_screen_shows() {
    let mut app = app_with(3);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    // The list does not outlive the prompt: a screen that draws the list
    // instead of the line swallows every letter the human types next.
    assert!(app.picker.is_none(), "the list stayed over the prompt");
    app.handle_key(key(KeyCode::Char('-')));
    app.handle_key(key(KeyCode::Char('x')));
    let shown = screen(&app, 100, 24);
    assert!(shown.contains("-x"), "the keystrokes went nowhere visible:\n{shown}");
    assert!(!shown.contains("models  ("), "the list is still on screen:\n{shown}");
}

#[rstest]
fn given_the_model_list_when_e_is_pressed_then_the_line_holds_the_endpoint_and_the_model_id() {
    let mut app = app_with(3);
    app.settings.apply("add https://api.openai.com/v1 gpt-4o").unwrap();
    app.handle_key(key(KeyCode::Char('M')));
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Char('e')));
    assert!(app.picker.is_none(), "the list stayed over the prompt");
    let Mode::Input(input) = &app.mode else { panic!("no prompt was opened") };
    // Two fields, in the order the request needs them. The row number is the
    // surface's business: the human edits what goes in the request.
    assert_eq!(input.buffer, "https://api.openai.com/v1 gpt-4o");
    let shown = screen(&app, 120, 24);
    assert!(shown.contains("edit model 4"), "the prompt does not say which row:\n{shown}");
}

#[rstest]
fn given_a_row_edited_to_another_endpoint_when_it_is_committed_then_the_house_is_read_off_it() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('e')));
    clear(&mut app);
    for c in "https://api.openai.com/v1 gpt-4o".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    // Nobody typed "openai": the provider is what the endpoint resolves to,
    // and the notice is where the human sees it.
    assert_eq!(app.settings.candidates()[1].endpoint, "https://api.openai.com/v1");
    assert_eq!(app.settings.candidates()[1].model, "gpt-4o");
    let said = app.notice.clone().unwrap_or_default();
    assert!(said.contains("openai/gpt-4o"), "the notice hid the house: {said}");
}

#[rstest]
fn given_the_add_prompt_when_it_opens_then_it_asks_for_two_fields_and_nothing_else() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    for c in "https://api.openai.com/v1 gpt-4o".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let added = app.settings.candidates().last().cloned().unwrap();
    assert_eq!(added.endpoint, "https://api.openai.com/v1");
    assert_eq!(added.model, "gpt-4o");
    assert_eq!(app.settings.candidates().len(), 2);
}

#[rstest]
fn given_a_row_on_the_launch_endpoint_when_e_is_pressed_then_the_line_still_carries_the_field() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('e')));
    let Mode::Input(input) = &app.mode else { panic!("no prompt was opened") };
    assert!(
        input.buffer.starts_with("- "),
        "an omitted endpoint looks like a one-field row: {}",
        input.buffer
    );
    let shown = screen(&app, 120, 24);
    assert!(shown.contains("launch endpoint"), "`-` is not explained anywhere:\n{shown}");
}

#[rstest]
fn given_the_model_list_when_d_is_pressed_then_the_screen_says_which_row_was_dropped() {
    let mut app = app_with(3);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('d')));
    let shown = screen(&app, 100, 24);
    assert!(
        shown.contains("removed") || shown.contains("remove"),
        "dropping a row said nothing a human can read:\n{shown}"
    );
    assert_eq!(app.settings.candidates().len(), 2, "the row was not dropped");
}

#[rstest]
fn given_the_last_row_of_the_model_list_when_d_is_pressed_then_the_surface_does_not_fall_over() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('d')));
    // One model and no other: the surface must survive its own arithmetic.
    app.handle_key(key(KeyCode::Esc));
    let shown = screen(&app, 100, 24);
    assert!(shown.contains("queue"), "the surface did not come back:\n{shown}");
}
