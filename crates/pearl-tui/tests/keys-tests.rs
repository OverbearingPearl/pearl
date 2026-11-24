//! Stories for the three keyboards.
//!
//! A style owns motion; the product's actions keep their letters. Each style
//! has to be the real thing -- a vim user's C-d and an emacs user's C-v are not
//! decorations -- and none of them may need a key a laptop does not have.

use pearl_core::{Keys, Queue};
use pearl_tui::{screen, App, Panes};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn chord(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    chord(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(code: KeyCode) -> KeyEvent {
    chord(code, KeyModifiers::ALT)
}

/// An app whose focused task has a long answer, so scrolling has something to
/// hold back: a pane with two lines in it cannot scroll, and a story that
/// pretends otherwise tests nothing.
fn reading_app(style: Keys) -> App {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    queue.complete(id, (0..200).map(|i| format!("line {i}\n")).collect::<String>()).unwrap();
    let settings = pearl_core::Settings::new("m");
    settings.apply(&format!("keys {}", style.as_str())).unwrap();
    App::new(queue).with_settings(settings)
}

fn app_with(tasks: usize, style: Keys) -> App {
    let queue = Queue::new();
    for i in 0..tasks {
        let (id, _) = queue.append(format!("task {i}"));
        if i == 0 {
            let _ = queue.add_child(id, "a part");
        }
    }
    let settings = pearl_core::Settings::new("m");
    settings.apply(&format!("keys {}", style.as_str())).unwrap();
    App::new(queue).with_settings(settings)
}

fn focused(app: &App) -> String {
    app.queue.focus().and_then(|id| app.queue.get(id)).map(|n| n.title).unwrap_or_default()
}

#[rstest]
fn given_plain_keys_when_the_arrows_move_then_the_focus_moves() {
    let mut app = app_with(4, Keys::Plain);
    let first = focused(&app);
    app.handle_key(key(KeyCode::Down));
    assert_ne!(focused(&app), first);
    app.handle_key(key(KeyCode::Up));
    assert_eq!(focused(&app), first);
}

#[rstest]
fn given_plain_keys_on_a_laptop_when_the_report_is_scrolled_then_no_page_key_is_needed() {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    queue.complete(id, (0..200).map(|i| format!("line {i}\n")).collect::<String>()).unwrap();
    let mut app = App::new(queue);
    app.handle_key(alt(KeyCode::Up));
    assert!(app.report_back > 0, "a MacBook has no PageUp; Alt+Up must scroll");
}

#[rstest]
fn given_vim_keys_when_j_and_k_are_pressed_then_the_focus_moves() {
    let mut app = app_with(4, Keys::Vim);
    let first = focused(&app);
    app.handle_key(key(KeyCode::Char('j')));
    assert_ne!(focused(&app), first);
    app.handle_key(key(KeyCode::Char('k')));
    assert_eq!(focused(&app), first);
}

#[rstest]
fn given_vim_keys_when_f_and_b_are_pressed_then_the_queue_pages_by_name() {
    let mut app = app_with(40, Keys::Vim);
    app.queue_page.set(10);
    let first = app.queue.focus().unwrap();
    app.handle_key(key(KeyCode::Char('f')));
    assert_ne!(app.queue.focus().unwrap(), first, "f did not page the queue");
    app.handle_key(key(KeyCode::Char('b')));
    assert_eq!(app.queue.focus().unwrap(), first, "b did not page back");
    // The old spelling is gone rather than shadowed: two names for one
    // motion, one of which the row cannot print, is a legend that lies.
    app.handle_key(ctrl('f'));
    assert_eq!(app.queue.focus().unwrap(), first, "C-f still pages, so the row is not the truth");
}

#[rstest]
fn given_vim_keys_when_ctrl_u_and_ctrl_d_are_pressed_then_the_queue_pages() {
    let mut app = app_with(40, Keys::Vim);
    app.queue_page.set(10);
    let first = app.queue.focus().unwrap();
    app.handle_key(ctrl('d'));
    assert_ne!(app.queue.focus().unwrap(), first, "C-d did not page the queue");
    app.handle_key(ctrl('u'));
    assert_eq!(app.queue.focus().unwrap(), first, "C-u did not page back");
}

#[rstest]
fn given_vim_keys_when_alt_carries_the_chord_then_the_report_pages_instead() {
    let mut app = reading_app(Keys::Vim);
    app.report_page.set(10);
    app.handle_key(chord(KeyCode::Char('u'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(app.report_back > 0, "Alt+C-u did not page the report back");
    app.handle_key(chord(KeyCode::Char('d'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 0, "Alt+C-d did not come forward");
}

#[rstest]
fn given_vim_keys_when_gg_and_shift_g_are_pressed_then_the_ends_are_reached() {
    let mut app = app_with(6, Keys::Vim);
    app.handle_key(key(KeyCode::Char('G')));
    let last = focused(&app);
    app.handle_key(key(KeyCode::Char('g')));
    app.handle_key(key(KeyCode::Char('g')));
    assert_ne!(focused(&app), last, "gg did not reach the other end");
}

#[rstest]
fn given_vim_keys_when_h_and_l_are_pressed_then_they_move_through_the_levels() {
    let mut app = app_with(3, Keys::Vim);
    // Focus the task that has a part, then drill in and back out.
    while app.queue.get(app.queue.focus().unwrap()).map(|n| n.children.is_empty()).unwrap_or(true) {
        app.handle_key(key(KeyCode::Char('j')));
    }
    app.handle_key(key(KeyCode::Char('l')));
    assert!(app.queue.scope().is_some(), "l did not drill in");
    app.handle_key(key(KeyCode::Char('h')));
    assert!(app.queue.scope().is_none(), "h did not come back out");
}

#[rstest]
fn given_emacs_keys_when_ctrl_n_and_ctrl_p_are_pressed_then_the_focus_moves() {
    let mut app = app_with(4, Keys::Emacs);
    let first = focused(&app);
    app.handle_key(ctrl('n'));
    assert_ne!(focused(&app), first);
    app.handle_key(ctrl('p'));
    assert_eq!(focused(&app), first);
}

#[rstest]
fn given_emacs_keys_when_ctrl_v_and_alt_v_are_pressed_then_the_queue_pages() {
    let mut app = app_with(40, Keys::Emacs);
    app.queue_page.set(10);
    let first = app.queue.focus().unwrap();
    app.handle_key(ctrl('v'));
    assert_ne!(app.queue.focus().unwrap(), first, "C-v did not page the queue");
    app.handle_key(alt(KeyCode::Char('v')));
    assert_eq!(app.queue.focus().unwrap(), first, "M-v did not page back");
}

#[rstest]
fn given_emacs_keys_when_ctrl_alt_v_are_pressed_then_the_other_window_scrolls() {
    let mut app = reading_app(Keys::Emacs);
    app.report_page.set(10);
    app.handle_key(chord(KeyCode::Char('V'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(app.report_back > 0, "C-M-S-v did not scroll the other window back");
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 0, "C-M-v did not come forward");
}

#[rstest]
fn given_any_style_when_its_report_line_keys_are_pressed_then_the_report_moves_one_line() {
    // Each style keeps its own letters: arrows in plain, j/k in vim, n/p in
    // emacs. Alt is the report in all three.
    let cases = [
        (Keys::Plain, alt(KeyCode::Up), alt(KeyCode::Down)),
        (Keys::Vim, alt(KeyCode::Char('k')), alt(KeyCode::Char('j'))),
        (Keys::Emacs, alt(KeyCode::Char('p')), alt(KeyCode::Char('n'))),
    ];
    for (style, back, forward) in cases {
        let mut app = reading_app(style);
        app.handle_key(back);
        assert_eq!(app.report_back, 1, "{style:?}: not one line back");
        app.handle_key(forward);
        assert_eq!(app.report_back, 0, "{style:?}: not one line forward");
    }
}

#[rstest]
fn given_plain_keys_when_a_vim_letter_is_pressed_then_it_does_not_move_the_queue() {
    let mut app = app_with(4, Keys::Plain);
    let first = focused(&app);
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(focused(&app), first, "a vim key acted in the plain style");
    let legend = app.motion_line();
    assert!(!legend.contains("j/k"), "the plain legend advertised vim keys: {legend}");
}

#[rstest]
fn given_any_style_when_shift_j_is_pressed_then_the_queue_pages_and_the_cursor_stays_in_view() {
    for style in [Keys::Plain, Keys::Vim, Keys::Emacs] {
        let mut app = app_with(40, style);
        app.queue_page.set(8);
        app.handle_key(key(KeyCode::Char('J')));
        let focus = app.queue.focus().unwrap();
        let screen = screen(&app, 60, 20);
        let title = app.queue.get(focus).unwrap().title;
        assert!(screen.contains(&title), "{style:?}: the cursor left the window: {title}");
    }
}

#[rstest]
fn given_emacs_keys_when_ctrl_g_is_pressed_then_the_pending_thing_is_quit() {
    let mut app = app_with(2, Keys::Emacs);
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(ctrl('g'));
    let screen = screen(&app, 90, 20);
    assert!(!screen.contains("append:"), "C-g did not quit the prompt:\n{screen}");
}

#[rstest]
fn given_emacs_keys_when_ctrl_x_2_is_pressed_then_the_report_is_taken() {
    let mut app = app_with(2, Keys::Emacs);
    app.handle_key(ctrl('x'));
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.panes, Panes::Report, "C-x 2 did not give the screen to the report");
    app.handle_key(ctrl('x'));
    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.panes, Panes::Queue, "C-x 1 did not give the screen to the queue");
}

#[rstest]
fn given_any_style_when_the_motion_legend_is_read_then_each_key_names_its_window() {
    for style in [Keys::Plain, Keys::Vim, Keys::Emacs] {
        let app = app_with(2, style);
        let legend = app.motion_line();
        assert!(legend.contains("queue "), "{style:?}: no key says it moves the queue");
        assert!(legend.contains("report "), "{style:?}: no key says it reads the report");
        assert!(!legend.contains("pane"), "{style:?}: a pane cycle implies a focus we do not have");
    }
}

#[rstest]
fn given_any_style_when_the_action_letters_are_pressed_then_the_product_is_the_same() {
    for style in [Keys::Plain, Keys::Vim, Keys::Emacs] {
        let mut app = app_with(2, style);
        app.handle_key(key(KeyCode::Char('a')));
        let screen = screen(&app, 90, 20);
        assert!(screen.contains("append:"), "{style:?} lost the append key");
    }
}

#[rstest]
fn given_any_style_when_the_style_key_is_pressed_then_the_next_keyboard_is_taken_and_named() {
    let mut app = app_with(2, Keys::Plain);
    app.handle_key(key(KeyCode::Char('\\')));
    assert_eq!(app.settings.keys(), Keys::Vim);
    let screen = screen(&app, 120, 20);
    assert!(screen.contains("vim (\\)"), "the style was not named with its key:\n{screen}");
}

#[rstest]
fn given_a_key_from_authinfo_when_the_status_is_read_then_the_house_is_named_with_the_model() {
    let app = app_with(2, Keys::Plain).with_provider("deepseek");
    let screen = screen(&app, 120, 20);
    assert!(screen.contains("deepseek/m"), "the provider was not named: {screen}");
}

#[rstest]
fn given_a_running_process_when_the_status_is_read_then_its_footprint_is_shown() {
    let mut app = app_with(2, Keys::Plain);
    app.footprint = Some(String::from("42M"));
    let screen = screen(&app, 130, 22);
    assert!(screen.contains("mem 42M"), "the footprint was not shown: {screen}");
}

#[rstest]
fn given_a_task_that_cost_something_when_the_queue_is_read_then_its_bill_is_on_its_row() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1234, 340, 2400));
    let app = App::new(queue);
    let screen = screen(&app, 110, 22);
    assert!(screen.contains("↑1.2k ↓340 2.4s"), "the task's bill was not on its row:\n{screen}");
}

#[rstest]
fn given_a_task_that_never_ran_when_the_queue_is_read_then_no_bill_is_claimed() {
    let queue = Queue::new();
    let _ = queue.append("work");
    let app = App::new(queue);
    let screen = screen(&app, 110, 22);
    assert!(!screen.contains("↑0"), "an unrun task claimed a zero cost:\n{screen}");
}

#[rstest]
fn given_a_session_that_cost_something_when_the_status_is_read_then_the_total_is_there() {
    let gauge = pearl_core::Gauge::new();
    gauge.charge(pearl_core::Cost::turn(2000, 400, 3000));
    let app = App::new(Queue::new()).with_gauge(gauge);
    let screen = screen(&app, 130, 22);
    assert!(screen.contains("↑2.0k ↓400 3.0s"), "the session total was missing:\n{screen}");
}

#[rstest]
fn given_a_task_of_two_turns_when_its_report_is_read_then_each_turn_is_costed() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1000, 100, 500));
    queue.charge(id, pearl_core::Cost::turn(500, 50, 250));
    queue.complete(id, "the answer").unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 110, 24);
    assert!(screen.contains("turn 1"), "the turns were not itemised:\n{screen}");
    assert!(screen.contains("turn 2"), "the second turn was missing:\n{screen}");
    assert!(screen.contains("the answer"), "the answer was pushed out by the bills");
}

#[rstest]
fn given_a_place_when_the_surface_is_read_then_the_ground_is_named() {
    let mut app = app_with(2, Keys::Plain);
    app.place = Some(String::from("~/proj  git:trunk*2  sandbox:workspace-write"));
    let screen = screen(&app, 120, 22);
    assert!(screen.contains("~/proj"), "the directory was not shown:\n{screen}");
    assert!(screen.contains("git:trunk*2"), "the branch was not shown:\n{screen}");
    assert!(screen.contains("sandbox:workspace-write"), "the sandbox was not shown:\n{screen}");
}

#[rstest]
fn given_a_place_and_a_bill_when_the_header_is_read_then_both_are_on_the_first_line() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(2000, 400, 3000));
    let gauge = pearl_core::Gauge::new();
    gauge.charge(pearl_core::Cost::turn(2000, 400, 3000));
    let mut app = App::new(queue).with_gauge(gauge);
    app.place = Some(String::from("write:~/proj  git:trunk*2"));
    app.footprint = Some(String::from("16M"));
    let screen = screen(&app, 150, 22);
    let first = screen.lines().next().unwrap_or_default();
    assert!(first.contains("write:~/proj"), "the ground was not on the first line: {first}");
    assert!(first.contains("↑2.0k"), "the bill was not on the first line: {first}");
    assert!(first.contains(pearl_tui::wordmark()), "the name did not lead: {first}");
}

/// The eight motions every style owes the human: a line, a page and the two
/// ends of each window, and the level in and out. A style that offers seven
/// of them is a style with a hole in it, and the hole is always the same one
/// -- the far end of the report, which nobody remembers to bind.
#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_style_when_the_queue_is_jumped_to_either_end_then_the_focus_lands_there(
    #[case] style: Keys,
) {
    let mut app = app_with(40, style);
    let rows: Vec<String> = app.queue.view().iter().map(|n| n.title.clone()).collect();
    app.handle_key(key(KeyCode::End));
    assert_eq!(focused(&app), rows[rows.len() - 1], "{style:?} could not reach the last row");
    app.handle_key(key(KeyCode::Home));
    assert_eq!(focused(&app), rows[0], "{style:?} could not reach the first row");
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_style_when_the_report_is_jumped_to_either_end_then_both_ends_are_reachable(
    #[case] style: Keys,
) {
    let mut app = reading_app(style);
    app.handle_key(alt(KeyCode::Home));
    assert!(app.report_back > 100, "{style:?} could not reach the oldest line");
    app.handle_key(alt(KeyCode::End));
    assert_eq!(app.report_back, 0, "{style:?} could not come back to the newest");
}

#[rstest]
fn given_vim_keys_when_alt_gg_is_typed_then_the_report_shows_its_oldest_line() {
    let mut app = reading_app(Keys::Vim);
    app.handle_key(alt(KeyCode::Char('g')));
    app.handle_key(alt(KeyCode::Char('g')));
    assert!(app.report_back > 100, "Alt+gg did not reach the top of the report");
}

#[rstest]
fn given_emacs_keys_when_the_other_window_is_paged_up_then_the_report_goes_back() {
    let mut app = reading_app(Keys::Emacs);
    app.report_page.set(10);
    app.handle_key(chord(KeyCode::Char('V'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 10, "C-M-S-v did not page the report back");
}

#[rstest]
fn given_emacs_keys_when_the_other_window_is_jumped_then_the_report_shows_its_ends() {
    let mut app = reading_app(Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('<'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(app.report_back > 100, "C-M-< did not reach the top of the report");
    app.handle_key(chord(KeyCode::Char('>'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 0, "C-M-> did not come back to the newest");
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_style_when_its_legend_is_read_then_it_names_the_ends_of_both_windows(
    #[case] style: Keys,
) {
    let app = app_with(2, style);
    let line = app.motion_line();
    // One slash list per window, so the ends are the last two entries of
    // each rather than a group of their own.
    let (queue, report) = match style {
        Keys::Plain => ("Home/End", "Alt+↑↓/PgUp/Dn/Home/End"),
        Keys::Vim => ("gg/G", "M-gg/M-G"),
        Keys::Emacs => ("M-</M->", "C-M-</C-M->"),
    };
    let mut parts = line.split("  ");
    let q = parts.next().unwrap_or_default();
    let r = parts.next().unwrap_or_default();
    assert!(q.ends_with(queue), "{style:?} queue legend does not end at its ends: {q}");
    assert!(r.ends_with(report), "{style:?} report legend does not end at its ends: {r}");
}
