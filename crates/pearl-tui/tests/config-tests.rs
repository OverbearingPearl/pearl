//! Stories for setting a knob at the surface, while the work runs.
//!
//! The human notices the model is wrong mid-task. `:` opens a line, the setting
//! changes, and the screen says what is now in force -- no restart, no file.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Queue, Settings};
use pearl_tui::{screen, App};
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
    assert!(
        screen.contains("64000"),
        "the window in force is invisible:\n{screen}"
    );
    assert!(
        screen.contains("deepseek-chat"),
        "the model in force is invisible:\n{screen}"
    );
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
    app.settle_undo(Ok(String::from(
        "undone op 2 (echo x > x.txt): 1 path(s) restored",
    )));
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
fn given_a_window_partly_full_when_the_screen_is_drawn_then_the_fill_the_ceiling_and_the_share_are_on_it()
{
    let gauge = pearl_core::Gauge::new();
    gauge.spent(24_000);
    gauge.folded();
    let app = App::new(Queue::new()).with_gauge(gauge);
    let screen = pearl_tui::screen(&app, 110, 24);
    assert!(
        screen.contains("ctx 24k/128k·⅜"),
        "the fill, its ceiling and where it folds were invisible:\n{screen}"
    );
    assert!(
        screen.contains("folded 1x"),
        "history was folded and the screen hid it:\n{screen}"
    );
}

#[rstest]
fn given_a_log_that_folded_and_hit_cache_when_the_header_is_read_then_the_window_key_stays_beside_the_window()
 {
    // The reading grew facts of its own after the key was written, and the
    // key ended up after them: `w/W` was printed beside `cache 87%`, which it
    // does not change. The window is what the key moves, so the window is what
    // the key is beside.
    let gauge = pearl_core::Gauge::new();
    gauge.spent_with_cache(22_000, 19_000);
    gauge.folded();
    let app = App::new(Queue::new()).with_gauge(gauge);
    let items = app.settings_items();
    let knob = items
        .iter()
        .position(|i| i.contains("(w/W)"))
        .unwrap_or_else(|| panic!("no window key in {items:?}"));
    assert_eq!(
        items[knob], "ctx 22k/128k·⅜ (w/W)",
        "the window key is not on the window: {:?}",
        items[knob]
    );
    for item in &items {
        assert!(
            !item.contains("folded ") || !item.contains('('),
            "a key is named beside a reading it does not change: {item}"
        );
    }
    assert!(items.iter().any(|i| i.contains("folded ")), "{items:?}");
}

// The row ended with `cache 86%` early on and `cache 89%` at the end, one from
// the window and one from the directory, and neither said which it was. A rate
// read twice on one line reads as a stutter, and the two numbers disagree, so
// the human cannot tell which one the work is being charged at. There is no
// rate on the row at all now: the money says the two halves the bill was made
// of, and says them once.
#[rstest]
fn given_a_log_that_folded_and_hit_cache_when_the_status_row_is_read_then_the_money_is_named_once() {
    let gauge = pearl_core::Gauge::new();
    gauge.spent_with_cache(22_000, 19_000);
    gauge.folded();
    gauge.charge(pearl_core::Cost {
        up: 601_157,
        down: 17_702,
        ms: 150_000,
        rounds: 22,
        cached: 540_000,
    });
    let app = App::new(Queue::new()).with_gauge(gauge);
    let items = app.settings_items();
    assert!(
        !items.iter().any(|i| i.contains("cache ")),
        "a rate stands on the row beside the split that says the same thing: {items:?}"
    );
    let line = app.settings_line();
    assert_eq!(
        line.matches("cache ").count(),
        0,
        "the row still names a rate of its own: {line}"
    );
    assert_eq!(
        line.matches('↑').count(),
        1,
        "the row names its money more than once: {line}"
    );
    // Sixty-one thousand new and five hundred and forty thousand served: the
    // directory's own split, in one unit, where the total used to stand.
    assert!(
        line.contains("↑61.2k+540.0k") && line.contains("↓17.7k"),
        "the money left the row: {line}"
    );
}

// A night's cost is the reading the human asked to see, and a long directory
// name used to be able to push it off the row: the ground is fitted first and
// everything after it yielded, so three quarters of a screen of path cost the
// bill its place. The money is drawn last and does not yield to a path.
#[rstest]
fn given_a_launch_that_was_billed_when_a_wide_header_is_drawn_then_the_money_closes_the_row() {
    let gauge = pearl_core::Gauge::new();
    gauge.charge(pearl_core::Cost {
        up: 601_157,
        down: 17_702,
        ms: 150_000,
        rounds: 22,
        cached: 540_000,
    });
    let mut app = App::new(Queue::new()).with_gauge(gauge);
    app.place = Some(String::from(
        "write:~/Projects/rust/pearl  git:main  sandbox:workspace-write",
    ));
    let drawn = screen(&app, 140, 20);
    let row = drawn.lines().next().unwrap();
    assert!(
        row.trim_end().ends_with("↑61.2k+540.0k ↓17.7k 2m30s"),
        "the money did not close the row:\n{row}"
    );
}

// The keyed readings do not yield to the money. A row too narrow for both keeps
// the window and the way out of a mode, and the bill waits for a wider screen;
// the money's own time is the first thing it gives up when it has a little more
// room than that.
#[rstest]
fn given_a_row_with_no_room_for_both_when_the_header_is_drawn_then_the_window_keeps_its_key() {
    let gauge = pearl_core::Gauge::new();
    gauge.charge(pearl_core::Cost {
        up: 601_157,
        down: 17_702,
        ms: 150_000,
        rounds: 22,
        cached: 540_000,
    });
    let app = App::new(Queue::new()).with_gauge(gauge);
    let drawn = screen(&app, 100, 20);
    let row = drawn.lines().next().unwrap();
    assert!(
        row.contains("ctx 0k/128k·⅜ (w/W)") && row.contains("plain (\\"),
        "a keyed reading was dropped for the money:\n{row}"
    );
    assert!(
        !row.contains("↑"),
        "the money squeezed a keyed reading out:\n{row}"
    );
}

// `ctx 22k/128k` answered "how full is the window" and quietly said "plenty of
// room", when the history in fact resets at three eighths of that. The number
// the night is actually run at was nowhere on the surface: it could only be
// learned by typing `:window` and reading the reply, or by seeing `folded 1x`
// after it had already happened. A token count for the fold point was shown
// next and read as a second ceiling -- `22k→48k` says "48k is the wall", when
// 48k is where the page turns inside a window of 128k. The share is spelled
// against the ceiling instead, so the two numbers the row names are the two
// knobs a person owns: `w`/`W` the ceiling, the trigger the share of it.
#[rstest]
fn given_a_fold_point_inside_the_window_when_the_status_is_read_then_the_share_it_folds_at_is_named()
{
    let settings = pearl_core::Settings::new("deepseek-chat");
    let app = App::new(Queue::new()).with_settings(settings.clone());
    let line = app.settings_line();
    assert!(
        line.contains("ctx 0k/128k·⅜"),
        "the status names the fill but not the share it folds at: {line}"
    );

    // The ceiling is what `w`/`W` move, and the share is a fraction of it, so
    // the share stays and the ceiling moves.
    settings.apply("window 64000").unwrap();
    let line = app.settings_line();
    assert!(
        line.contains("ctx 0k/64k·⅜"),
        "the ceiling moved without the share: {line}"
    );

    // The trigger is a knob of its own: a human who folds earlier sees the
    // number change in the same place, against the same ceiling.
    settings.apply("trigger 0.5").unwrap();
    let line = app.settings_line();
    assert!(
        line.contains("ctx 0k/64k·½"),
        "the share did not follow the trigger: {line}"
    );
}

#[rstest]
fn given_a_reading_with_a_key_when_the_status_is_shown_then_the_key_is_named_beside_it() {
    let app = App::new(Queue::new());
    let line = app.settings_line();
    for pair in [("deepseek-chat", "M"), ("think", "E"), ("ctx", "W")] {
        let at = line
            .find(pair.0)
            .unwrap_or_else(|| panic!("{} missing: {line}", pair.0));
        let after = &line[at..(at + 40).min(line.len())];
        assert!(
            after.contains(pair.1),
            "no key named beside {}: {line}",
            pair.0
        );
    }
}

#[rstest]
fn given_the_status_when_it_is_shown_then_the_window_is_not_said_twice() {
    let app = App::new(Queue::new());
    let line = app.settings_line();
    assert!(
        !line.contains("window"),
        "the window is stated twice: {line}"
    );
}

#[rstest]
fn given_the_model_key_when_it_is_pressed_then_the_next_model_is_in_force() {
    let mut app = App::new(Queue::new());
    app.settings.apply("add - deepseek-reasoner").unwrap();
    app.settings.apply("dialect openai").unwrap();
    app.handle_key(key(KeyCode::Char('m')));
    assert_eq!(app.settings.model(), "deepseek-reasoner");
    app.handle_key(key(KeyCode::Char('e')));
    assert_eq!(app.settings.effort().map(|e| e.as_str()), Some("off"));
    let before = app.settings.window();
    app.handle_key(key(KeyCode::Char('w')));
    assert_ne!(app.settings.window(), before);
}

#[rstest]
fn given_edited_knobs_when_the_save_key_is_pressed_then_the_surface_asks_for_one_save() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_key(key(KeyCode::Char('w')));
    assert!(!app.save_requested, "saving happened without being asked");
    app.handle_key(key(KeyCode::Char('S')));
    assert!(
        app.save_requested,
        "one key must save every knob, not one key per knob"
    );
}

#[rstest]
fn given_a_save_that_worked_when_it_settles_then_the_surface_says_where_it_went() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('S')));
    app.settle_save(Ok(String::from("/home/me/.pearl/config")));
    assert!(!app.save_requested);
    assert!(
        app.notice
            .clone()
            .unwrap_or_default()
            .contains(".pearl/config")
    );
}

#[rstest]
fn given_a_dialect_that_was_stated_when_the_header_is_drawn_then_it_names_it() {
    let app = App::new(Queue::new());
    let plain = app.settings_items().join(" | ");
    assert!(
        !plain.contains("silent"),
        "a derived default was reported as news: {plain}"
    );
    app.settings.apply("dialect silent").unwrap();
    let stated = app.settings_items().join(" | ");
    assert!(
        stated.contains("silent"),
        "a stated dialect is why the ladder is short: {stated}"
    );
}

// A fold written by a cheaper model is a saving the human cannot see unless
// the window says which model writes it. Otherwise the status names one model
// and the log quietly pays another for every summary.
#[rstest]
fn given_a_cheaper_model_named_for_the_fold_when_the_status_is_shown_then_the_line_names_it() {
    let settings = pearl_core::Settings::new("deepseek-reasoner");
    let app = App::new(Queue::new()).with_settings(settings.clone());
    assert!(
        !app.settings_line().contains("fold "),
        "a fold nobody named is on the status: {}",
        app.settings_line()
    );

    settings.apply("fold deepseek-chat").unwrap();
    let line = app.settings_line();
    assert!(
        line.contains("folded by deepseek-chat"),
        "the status does not say which model writes the summary: {line}"
    );
}

/// The count rides with the money, but it is not the money's passenger: when
/// the bill cannot be named beside the keys, the count still can. It is the
/// only sign the window gave history up, so a narrow row keeps it and drops
/// the bill -- and the keys it must not drop stay.
#[rstest]
fn given_a_bill_that_will_not_fit_when_the_row_is_narrow_then_the_fold_count_rides_alone() {
    let gauge = pearl_core::Gauge::new();
    gauge.spent_with_cache(22_000, 19_000);
    for _ in 0..12 {
        gauge.folded();
    }
    gauge.charge(pearl_core::Cost {
        up: 601_157,
        down: 17_702,
        ms: 150_000,
        rounds: 22,
        cached: 540_000,
    });
    let app = App::new(Queue::new()).with_gauge(gauge);
    let drawn = pearl_tui::screen(&app, 118, 24);
    assert!(
        drawn.contains("folded 12x"),
        "a row too tight for the bill hid the fold:\n{drawn}"
    );
    assert!(
        !drawn.contains('↑'),
        "the bill was named where the count alone was owed:\n{drawn}"
    );
    assert!(
        drawn.contains("(w/W)") && drawn.contains("(\\)"),
        "the count was kept at the cost of the keys:\n{drawn}"
    );
}

/// The fold count is the only sign that the window gave history up. A row
/// that runs out of width drops the footprint first, and keeps the count.
#[rstest]
fn given_a_tight_header_when_a_log_has_folded_then_the_count_is_still_on_the_row() {
    let gauge = pearl_core::Gauge::new();
    gauge.spent_with_cache(22_000, 19_000);
    for _ in 0..12 {
        gauge.folded();
    }
    let app = App::new(Queue::new()).with_gauge(gauge);
    let drawn = pearl_tui::screen(&app, 96, 24);
    assert!(
        drawn.contains("folded 12x"),
        "a tight row hid the fold:\n{drawn}"
    );
    assert!(
        drawn.contains("ctx 22k/128k"),
        "the window's own reading went with it:\n{drawn}"
    );
}
