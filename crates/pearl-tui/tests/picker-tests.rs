//! Stories for the three settings keys.
//!
//! One rule for all three: the lowercase letter steps to the next value, the
//! uppercase letter opens the values to choose between. No count decides which
//! one a press means -- a rule that changes with the length of a list is a rule
//! the human has to test before every press.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Queue, Settings};
use pearl_tui::{App, Mode, endpoint_example_lines, screen};
use ratatui::style::Modifier;
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
    assert!(
        app.picker.is_none(),
        "{rows} rows opened a list; the lowercase letter steps"
    );
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
    assert!(
        app.picker.is_some(),
        "{rows} rows cycled blind; the uppercase letter chooses"
    );
    assert_eq!(
        app.settings.model(),
        before,
        "opening the list changed the choice"
    );
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
    assert_eq!(
        app.settings.window(),
        stepped,
        "opening the sizes changed the window"
    );
    let shown = screen(&app, 100, 30);
    assert!(
        shown.contains("1000k") || shown.contains("1000000"),
        "the ladder's top is not on screen: {shown}"
    );
}

#[rstest]
fn given_an_open_size_list_when_a_size_is_chosen_then_it_is_in_force() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('W')));
    // The ladder's last rung is the million; walking to it and taking it is
    // what the list is for. The list starts on the size in force, and it
    // wraps, so "walk to the last" is a loop with a reason to stop rather
    // than a count of presses.
    let last = Settings::windows().len() - 1;
    for _ in 0..Settings::windows().len() {
        if app.picker == Some(last) {
            break;
        }
        app.handle_key(key(KeyCode::Down));
    }
    assert_eq!(app.picker, Some(last), "the cursor never reached the last rung");
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
    assert!(
        shown.contains("medium"),
        "the rungs are not on screen: {shown}"
    );
    assert!(
        shown.contains("auto"),
        "letting the endpoint decide is not a choice offered: {shown}"
    );
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
fn given_a_house_with_no_word_for_thinking_when_the_rung_list_opens_then_the_status_is_still_a_choice()
 {
    // The ladder is one vocabulary, so the menu has one shape: a rung this
    // house cannot say keeps its row and says so, rather than vanishing and
    // leaving a different menu per endpoint.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('E')));
    let shown = screen(&app, 100, 30);
    assert!(
        shown.contains("auto"),
        "letting the endpoint decide is missing: {shown}"
    );
    let rows = app.picker_rows();
    assert_eq!(
        rows.len(),
        1 + pearl_core::Effort::ladder().len(),
        "the ladder is not whole: {rows:?}"
    );
    assert!(
        rows.iter()
            .skip(1)
            .all(|r| r.contains("no setting for this strength")),
        "a rung this house cannot say is shown as choosable: {rows:?}"
    );
}

#[rstest]
fn given_an_empty_hand_when_the_lowercase_effort_key_is_pressed_then_the_effort_steps() {
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('e')));
    assert_eq!(app.settings.effort().map(|e| e.as_str()), Some("off"));
}

#[rstest]
fn given_a_task_in_hand_when_the_effort_key_is_pressed_then_the_hand_is_kept_and_the_setting_waits()
{
    let mut app = app_with(2);
    app.settings.apply("dialect openai").unwrap();
    app.queue.append("held");
    let id = app.queue.view()[0].id;
    app.queue.focus_on(id).unwrap();
    app.handle_key(key(KeyCode::Char('t')));
    let held = app
        .hand
        .and_then(|id| app.queue.get(id))
        .map(|n| n.title.clone())
        .unwrap_or_default();
    let before = app.settings.effort().map(|e| e.as_str());
    app.handle_key(key(KeyCode::Char('e')));
    // The hand is a mode: a setting key is not answered while a move is half
    // made, so the reading cannot change under the human's hands.
    assert_eq!(
        app.settings.effort().map(|e| e.as_str()),
        before,
        "the effort changed while a task was in hand"
    );
    assert_eq!(
        app.hand
            .and_then(|id| app.queue.get(id))
            .map(|n| n.title.clone()),
        Some(held),
        "the hand was edited by the effort key"
    );
}

#[rstest]
fn given_an_open_list_when_a_row_is_chosen_then_it_is_in_force_and_the_list_closes() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.settings.model(), "model-2");
    assert!(
        app.picker.is_none(),
        "the list stayed open after the choice"
    );
}

#[rstest]
fn given_an_open_list_when_a_row_is_dropped_then_it_is_gone_from_the_list() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('d')));
    let rows: Vec<String> = app
        .settings
        .candidates()
        .iter()
        .map(|c| c.model.clone())
        .collect();
    assert!(
        !rows.contains(&String::from("model-1")),
        "d did not drop the row: {rows:?}"
    );
    assert!(app.picker.is_some(), "dropping a row closed the list");
}

#[rstest]
fn given_an_open_list_when_adding_then_the_first_stage_is_the_addresses_on_offer() {
    let mut app = app_with(3);
    app.settings
        .apply("add https://api.openai.com/v1 gpt-4o")
        .unwrap();
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    // A row is two fields, and the address is the half a human reuses: it is
    // offered from what the list already names before an id is typed.
    let rows = app.picker_rows();
    assert!(app.picker.is_some(), "a did not open the address list");
    assert_eq!(app.pick_kind, pearl_tui::Pick::Endpoint(None));
    assert!(
        rows.iter().any(|r| r.contains("https://api.openai.com/v1")),
        "no address on offer: {rows:?}"
    );
    assert!(
        rows.iter().any(|r| r.contains("the launch endpoint")),
        "the launch row is missing: {rows:?}"
    );
    assert_eq!(
        rows.last().map(String::as_str),
        Some("enter an address of your own")
    );
}

#[rstest]
fn given_an_open_list_when_editing_then_the_second_stage_carries_the_row_as_it_stands() {
    let mut app = app_with(5);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('e')));
    assert!(app.picker.is_some(), "e skipped the address list");
    app.handle_key(key(KeyCode::Enter));
    // The address was chosen from the list, so the only thing left to type is
    // the id -- and it arrives filled in, because editing starts from the row.
    let Mode::Input(input) = &app.mode else {
        panic!("no id prompt was opened")
    };
    assert_eq!(input.draft.text(), "model-1");
    let line = app.prompt_line();
    assert!(
        line.contains("edit model 2"),
        "e did not name the row: {line}"
    );
    assert!(
        line.contains("model id"),
        "the second stage did not say what it wants: {line}"
    );
}

#[rstest]
fn given_the_address_list_when_its_last_row_is_chosen_then_the_address_is_typed() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('e')));
    for _ in 0..app.picker_rows().len() - 1 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(
        app.picker.is_none(),
        "the list stayed open over the typing line"
    );
    let Mode::Input(input) = &app.mode else {
        panic!("no address prompt was opened")
    };
    assert_eq!(
        input.draft.text(),
        "",
        "an address typed by hand starts from nothing"
    );
    assert!(
        app.prompt_line().contains("address"),
        "{}",
        app.prompt_line()
    );
    // Entering the second stage from a typed address still leaves the id, and
    // that stage starts from the row's own id rather than from nothing.
    app.handle_key(key(KeyCode::Enter));
    assert!(
        matches!(&app.mode, Mode::Input(input) if input.draft.text() == "deepseek-chat"),
        "the id stage did not follow the address stage: {}",
        app.prompt_line()
    );
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
    let app = App::new(Queue::new())
        .with_settings(s)
        .with_provider("deepseek");
    let line = app.settings_line();
    assert!(
        line.contains("openai/gpt-4o"),
        "the header still names the launch house: {line}"
    );
}

#[rstest]
fn given_a_row_whose_key_cannot_be_found_when_the_list_is_shown_then_the_row_says_so() {
    let s = Settings::new("deepseek-chat");
    for i in 1..5 {
        s.apply(&format!("add https://h{i}.example/v1 model-{i}"))
            .unwrap();
    }
    let mut app = App::new(Queue::new())
        .with_settings(s)
        .with_key_probe(|_| false);
    app.handle_key(key(KeyCode::Char('M')));
    let shown = screen(&app, 100, 24);
    assert!(
        shown.contains("no key"),
        "a row with no key looks like one that has one: {shown}"
    );
}

#[rstest]
fn given_the_address_list_when_an_address_is_chosen_then_the_list_is_gone_before_the_id_is_typed() {
    let mut app = app_with(3);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    // One thing on the screen at a time: a list drawn over the typing line
    // swallows every letter the human presses next.
    assert!(app.picker.is_none(), "the list stayed over the prompt");
    app.handle_key(key(KeyCode::Char('x')));
    let shown = screen(&app, 100, 24);
    assert!(
        shown.contains("model id x"),
        "the keystrokes went nowhere visible:\n{shown}"
    );
    assert!(
        !shown.contains("endpoint  ("),
        "the list is still on screen:\n{shown}"
    );
}

#[rstest]
fn given_a_typed_address_and_an_id_when_they_are_committed_then_the_house_is_read_off_the_address()
{
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('e')));
    for _ in 0..app.picker_rows().len() - 1 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    for c in "https://api.openai.com/v1".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    clear(&mut app);
    for c in "gpt-4o".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    // Nobody typed "openai": the provider is what the address resolves to,
    // and the notice is where the human sees it.
    assert_eq!(
        app.settings.candidates()[0].endpoint,
        "https://api.openai.com/v1"
    );
    assert_eq!(app.settings.candidates()[0].model, "gpt-4o");
    let said = app.notice.clone().unwrap_or_default();
    assert!(
        said.contains("openai/gpt-4o"),
        "the notice hid the house: {said}"
    );
}

#[rstest]
fn given_an_id_alone_when_it_is_committed_against_the_launch_address_then_the_row_follows() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    let launch = rows
        .iter()
        .position(|r| r.contains("the launch endpoint"))
        .expect("no launch row");
    for _ in 0..launch {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(
        matches!(&app.mode, Mode::Input(input) if input.draft.text().is_empty()),
        "the id stage did not follow"
    );
    for c in "qwen3".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let added = app.settings.candidates().last().cloned().unwrap();
    assert_eq!(added.endpoint, "", "the launch address was not kept");
    assert_eq!(added.model, "qwen3");
}

#[rstest]
fn given_an_empty_id_when_the_second_stage_is_committed_then_nothing_changes() {
    let mut app = app_with(2);
    let before = app.settings.candidates().len();
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.settings.candidates().len(),
        before,
        "an empty id added a row"
    );
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
    assert_eq!(
        app.settings.candidates().len(),
        2,
        "the row was not dropped"
    );
}

#[rstest]
fn given_the_last_row_of_the_model_list_when_d_is_pressed_then_the_surface_does_not_fall_over() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('d')));
    // One model and no other: the surface must survive its own arithmetic.
    app.handle_key(key(KeyCode::Esc));
    let shown = screen(&app, 100, 24);
    assert!(
        shown.contains("queue"),
        "the surface did not come back:\n{shown}"
    );
}

#[rstest]
fn given_a_rung_this_endpoint_refused_when_the_list_opens_then_the_row_still_exists_and_says_why() {
    let mut app = app_with(1);
    app.settings.apply("dialect openai").unwrap();
    app.settings.refuse(pearl_core::Effort::High);

    app.handle_key(key(KeyCode::Char('E')));
    let rows = app.picker_rows();
    assert!(
        rows.iter()
            .any(|r| r.starts_with("high") && r.contains("refused")),
        "the refused rung is missing or unmarked: {rows:?}"
    );
    assert_eq!(
        rows.len(),
        1 + pearl_core::Effort::ladder().len(),
        "the ladder is not whole: {rows:?}"
    );
}

#[rstest]
fn given_a_marked_row_when_it_is_chosen_then_the_line_is_what_enters_the_notice() {
    let mut app = app_with(1);
    app.settings.apply("dialect openai").unwrap();
    app.settings.refuse(pearl_core::Effort::High);

    app.handle_key(key(KeyCode::Char('E')));
    let at = app
        .picker_rows()
        .iter()
        .position(|r| r.starts_with("high"))
        .unwrap();
    app.picker = Some(at);
    app.handle_key(key(KeyCode::Enter));

    let said = app.notice.clone().unwrap_or_default();
    assert!(
        said.contains("refused high"),
        "the marked row was applied or silently ignored: {said}"
    );
    assert!(
        said.contains("the launch endpoint refused high"),
        "the notice did not say which address refused: {said}"
    );
    assert_eq!(
        app.settings.effort(),
        None,
        "a refused rung went into force"
    );
}

#[rstest]
fn given_the_address_stage_when_another_apis_path_is_typed_then_it_is_read_as_chat_and_the_stage_says_so()
{
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    for _ in 0..app.picker_rows().len() - 1 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    for c in "https://api.anthropic.com/v1/messages".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    // The path is read as the chat base rather than refused: pearl speaks
    // openai-chat, and the notice says so while the id is asked for.
    let Mode::Input(input) = &app.mode else {
        panic!("the id stage did not follow the corrected address")
    };
    assert!(
        input.draft.text().is_empty(),
        "a new row starts from no id, not from one on the corrected address"
    );
    let said = app.notice.clone().unwrap_or_default();
    assert!(
        said.contains("openai-chat"),
        "the notice did not say what pearl speaks: {said}"
    );
    assert!(
        said.contains("https://api.anthropic.com/v1/chat/completions"),
        "the corrected wire address was not named: {said}"
    );
    assert!(app.picker.is_none(), "the address list stayed over the stage");
}

/// Type a whole string, one keystroke at a time: a story about an address the
/// human typed is a story about what reached the stage.
fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

#[rstest]
fn given_an_address_typed_and_left_behind_when_the_list_is_opened_again_then_it_is_not_offered() {
    // An address that never got a model id is not a row: the config has
    // nothing to say about it, so the next list must not either.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    for _ in 0..10 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    clear(&mut app);
    type_text(&mut app, "https://api.abandoned.test/v9");
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    assert!(
        !rows.iter().any(|r| r.contains("abandoned")),
        "a half-typed address became an offer: {rows:?}"
    );
    assert!(
        app.settings
            .candidates()
            .iter()
            .all(|c| c.endpoint != "https://api.abandoned.test/v9")
    );
}

#[rstest]
fn given_rows_on_disk_when_the_list_is_opened_then_each_address_they_name_is_on_offer() {
    // The offers are the config's rows, not this run's memory: a row saved
    // yesterday is a place the next request can go today, and the launch
    // endpoint appears once however many rows land on it.
    let path = std::env::temp_dir().join(format!("pearl-offers-{}.conf", std::process::id()));
    std::fs::write(
        &path,
        "candidate https://a.example/v1 one\ncandidate https://b.example/v1 two\ncandidate - three\n",
    )
    .unwrap();
    let s = Settings::new("deepseek-chat");
    s.load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let mut app = App::new(Queue::new()).with_settings(s);

    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    for wanted in ["https://a.example/v1", "https://b.example/v1"] {
        assert!(
            rows.iter().any(|r| r.contains(wanted)),
            "{wanted} is not offered: {rows:?}"
        );
    }
    assert_eq!(
        rows.iter()
            .filter(|r| r.contains("the launch endpoint"))
            .count(),
        1,
        "the launch endpoint is offered more than once: {rows:?}"
    );
}

#[rstest]
fn given_the_address_list_when_it_opens_then_the_addresses_the_major_houses_publish_are_on_offer() {
    // A human editing a row is choosing where requests go, and the answer is
    // nearly always a house they already pay. An empty list is not a blank
    // page to fill in; it is a list that forgot what it knows.
    let mut app = App::new(Queue::new()).with_settings(Settings::new("deepseek-chat"));
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    for address in pearl_core::catalog::WELL_KNOWN {
        assert!(
            rows.iter().any(|r| r.contains(address)),
            "{address} is not on offer: {rows:?}"
        );
    }
    assert_eq!(
        rows.last().map(String::as_str),
        Some("enter an address of your own")
    );
}

#[rstest]
fn given_a_config_naming_an_address_the_list_also_knows_when_it_opens_then_it_is_offered_once() {
    let mut app = App::new(Queue::new()).with_settings(Settings::new("deepseek-chat"));
    app.settings
        .apply("add https://api.openai.com/v1 gpt-4o")
        .unwrap();
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    assert_eq!(
        rows.iter()
            .filter(|r| r.contains("https://api.openai.com/v1"))
            .count(),
        1,
        "the same address is offered twice: {rows:?}"
    );
}

#[rstest]
fn given_a_remark_about_the_last_act_when_another_key_is_pressed_then_the_remark_is_gone() {
    // A remark belongs to the act it describes. Leaving it on screen until an
    // act that happens to clear it makes the screen say the wrong thing about
    // what just happened.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_key(key(KeyCode::Char('t')));
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.notice.is_some(), "the refused act left no remark");
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.notice, None, "the remark outlived the act it described");
}

#[rstest]
fn given_the_model_id_stage_when_it_is_abandoned_then_its_remark_goes_with_it() {
    let mut app = App::new(Queue::new()).with_settings(Settings::new("deepseek-chat"));
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    assert!(
        matches!(app.mode, Mode::Input(_)),
        "the id is not being asked for"
    );
    assert_eq!(
        app.notice.as_deref(),
        Some("model id the launch endpoint serves")
    );
    // The stage is abandoned half-way through being answered: a blank line
    // would close on the first Escape and say nothing, which is a different
    // story.
    for c in "deepseek".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Esc));
    assert!(
        app.is_prompting(),
        "the first Escape dropped the id being typed"
    );
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
    // The stage's own remark is gone; what is left is the note that the draft
    // was let go, which is the truth about the key just pressed.
    assert_eq!(
        app.notice.as_deref(),
        Some("the draft was dropped"),
        "the abandoned stage left its remark behind"
    );
}

#[rstest]
fn given_a_launch_endpoint_a_house_also_publishes_when_the_list_opens_then_it_is_offered_once() {
    // Two rows that send to the same place is a list that argues with itself,
    // and the human has no way to tell they are the same address.
    let mut app = App::new(Queue::new())
        .with_settings(Settings::new("deepseek-chat"))
        .with_launch("https://api.deepseek.com/chat/completions");
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    assert_eq!(
        rows.iter()
            .filter(|r| r.contains("https://api.deepseek.com"))
            .count(),
        1,
        "the launch address was offered again under its URL: {rows:?}"
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.contains("the launch endpoint"))
            .count(),
        1,
        "the launch row is not there exactly once: {rows:?}"
    );
}

#[rstest]
fn given_a_launch_endpoint_when_the_list_opens_then_its_row_names_the_address() {
    // "the launch endpoint" alone leaves the human to guess which house pearl
    // is talking to; the row can say it.
    let mut app = App::new(Queue::new())
        .with_settings(Settings::new("deepseek-chat"))
        .with_launch("https://api.deepseek.com");
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    let launch = rows
        .iter()
        .find(|r| r.contains("the launch endpoint"))
        .unwrap_or_else(|| panic!("no launch row: {rows:?}"));
    assert!(
        launch.contains("https://api.deepseek.com"),
        "the launch row did not name the address it sends to: {launch}"
    );
    assert!(
        launch.starts_with("deepseek"),
        "the launch row did not say whose address it is: {launch}"
    );
}

#[rstest]
fn given_a_deployment_address_in_the_list_when_it_is_drawn_then_the_row_names_the_vendor() {
    // The row is the only place the human learns which house a deployment
    // belongs to, and it is also the word the key is filed under.
    let s = Settings::new("qwen3.8-max");
    s.apply(
        "add https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1 qwen3.8-max",
    )
    .unwrap();
    s.apply("choose 2").unwrap();
    let mut app = App::new(Queue::new())
        .with_settings(s)
        .with_launch("https://api.deepseek.com");
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    let row = rows
        .iter()
        .find(|r| r.contains("llm-l5plvn5p70xoh8n5"))
        .unwrap_or_else(|| panic!("the deployment is not on offer: {rows:?}"));
    assert!(
        row.starts_with("dashscope"),
        "the row for the deployment does not name the vendor: {row}"
    );
}

#[rstest]
fn given_the_address_list_when_it_is_drawn_then_each_address_is_named_by_its_house() {
    // A column of URLs is a list of places with no names on it: the human
    // reading `open.bigmodel.cn/api/paas/v4` has to know that host is Zhipu
    // before the row means anything. The house is already the word the header
    // calls the chosen row and the word the key is filed under, so the list
    // says it rather than teaching a second vocabulary.
    let mut app = App::new(Queue::new())
        .with_settings(Settings::new("deepseek-chat"))
        .with_launch("https://api.deepseek.com");
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    for (address, house) in [
        ("https://api.openai.com/v1", "openai"),
        ("https://open.bigmodel.cn/api/paas/v4", "bigmodel"),
        ("https://ark.cn-beijing.volces.com/api/v3", "ark"),
        ("https://api.moonshot.cn/v1", "moonshot"),
    ] {
        let row = rows
            .iter()
            .find(|r| r.contains(address))
            .unwrap_or_else(|| panic!("{address} is not on offer: {rows:?}"));
        assert!(
            row.starts_with(house),
            "the row for {address} does not name {house}: {row}"
        );
    }
}

#[rstest]
fn given_a_list_longer_than_nine_rows_when_it_is_drawn_then_every_address_starts_in_one_column() {
    // The count is a column, so its width may not depend on which row is
    // being drawn: a list whose left edge shifts at the tenth row is a list
    // the eye has to re-find on the way down.
    let mut app = App::new(Queue::new())
        .with_settings(Settings::new("deepseek-chat"))
        .with_launch("https://api.deepseek.com");
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let shown = screen(&app, 100, 40);
    let columns: Vec<usize> = shown.lines().filter_map(|l| l.find("https://")).collect();
    assert!(
        columns.len() > 9,
        "the list does not reach the tenth row:\n{shown}"
    );
    assert!(
        columns.windows(2).all(|w| w[0] == w[1]),
        "the addresses do not line up:\n{shown}"
    );
}

#[rstest]
fn given_the_address_list_opened_from_a_row_when_it_is_drawn_then_the_topic_names_that_row() {
    // The list is opened from a row and the row is off-screen while it is
    // open, so the topic is the only thing that says which row is being
    // sent somewhere new.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('e')));
    let shown = screen(&app, 100, 30);
    assert!(
        shown.contains("address for deepseek-chat"),
        "the topic does not name the row being edited:\n{shown}"
    );
}

#[rstest]
fn given_two_ids_where_one_prefixes_the_other_when_the_shorter_is_chosen_then_the_mark_is_on_it() {
    // The mark says which row is in force, so it has to be found by row and
    // not by a substring of the label: `gpt-4` is inside `gpt-4o`.
    let s = Settings::new("gpt-4o");
    s.apply("add https://api.openai.com/v1 gpt-4").unwrap();
    s.apply("choose 2").unwrap();
    let mut app = App::new(Queue::new()).with_settings(s);
    app.handle_key(key(KeyCode::Char('M')));
    let shown = screen(&app, 100, 30);
    let marked = shown
        .lines()
        .find(|l| l.trim_start().starts_with('*'))
        .unwrap_or_else(|| panic!("no row is marked:\n{shown}"));
    assert!(
        marked.contains("gpt-4") && !marked.contains("gpt-4o"),
        "the mark is on the wrong row: {marked}"
    );
}

#[rstest]
fn given_a_deployment_already_paid_for_in_authinfo_when_the_address_list_opens_then_its_vendor_base_is_offered()
{
    // The human should not have to export PEARL_BASE_URL for a machine they
    // already hold a key for: the list is where an endpoint is chosen, and it
    // can name the address without being told the vendor's path.
    let mut app = App::new(Queue::new())
        .with_settings(Settings::new("deepseek-chat"))
        .with_suggested_endpoints(vec![String::from(
            "https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        )]);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows();
    assert!(
        rows.iter().any(|row| {
            row.starts_with("dashscope")
                && row.contains(
                    "https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
                )
        }),
        "the deployment the human already pays for is missing: {rows:?}"
    );
}

#[rstest]
fn given_the_address_stage_when_it_is_drawn_then_two_whole_urls_show_which_part_is_typed() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    for _ in 0..app.picker_rows().len() - 1 {
        app.handle_key(key(KeyCode::Down));
    }
    app.handle_key(key(KeyCode::Enter));
    let shown = screen(&app, 100, 30);
    assert!(
        shown.contains("https://openrouter.ai/api/v1/chat/completions"),
        "the remote example was not whole:\n{shown}"
    );
    assert!(
        shown.contains("http://localhost:11434/v1/chat/completions"),
        "the local example was not whole:\n{shown}"
    );
    // And the base is the part that carries weight, so the example says which
    // substring is the answer as well as what the whole looks like.
    let examples = app.endpoint_examples().expect("the stage has no examples");
    let lines = endpoint_example_lines(&examples);
    let base = &lines[0].spans[2];
    assert_eq!(base.content, "openrouter.ai/api/v1");
    assert!(
        base.style.add_modifier.contains(Modifier::BOLD)
            && base.style.add_modifier.contains(Modifier::UNDERLINED),
        "the base is not the part that stands out: {:?}",
        base.style
    );
}

#[rstest]
fn given_a_new_row_is_added_from_the_list_when_its_id_is_committed_then_it_is_the_model_in_use() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    // The first address is the launch endpoint, which is enough to add a row.
    app.handle_key(key(KeyCode::Enter));
    for c in "new-model".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    // Adding a row is an act of use, not bookkeeping: the human must not have
    // to find the row again in the list to make it the one in force.
    assert_eq!(app.settings.model(), "new-model");
    let said = app.notice.clone().unwrap_or_default();
    assert!(
        said.contains("now in use"),
        "the notice did not say the new row took over: {said}"
    );
    assert!(
        screen(&app, 100, 30).contains("new-model"),
        "the header still names the old model"
    );
}

#[rstest]
fn given_the_address_list_when_the_cursor_is_moved_past_an_end_then_it_wraps_to_the_other() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('M')));
    app.handle_key(key(KeyCode::Char('a')));
    let rows = app.picker_rows().len();
    assert!(rows > 5, "the address list is too short to be worth wrapping");
    for _ in 0..rows - 1 {
        app.handle_key(key(KeyCode::Down));
    }
    assert_eq!(app.picker, Some(rows - 1), "the cursor did not reach the end");
    app.handle_key(key(KeyCode::Down));
    // Wrapping is what makes the bottom affordable: the list is as long as the
    // houses a human has heard of, and stopping at the end costs every row.
    assert_eq!(app.picker, Some(0), "down at the last row fell off the end");
    assert!(
        screen(&app, 100, 30).contains("the launch endpoint"),
        "the first row did not scroll back into view"
    );
    app.handle_key(key(KeyCode::Up));
    assert_eq!(
        app.picker,
        Some(rows - 1),
        "up at the first row fell off the front"
    );
}
