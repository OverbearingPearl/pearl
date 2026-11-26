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

/// The same app on a terminal that answered the enhanced-keyboard query, so
/// Shift survives beside Ctrl and Meta. The answer is a field the surface
/// sets once at startup, so a story that asks for the enhanced spelling has
/// to say the terminal gave it.
fn capable_app(style: Keys) -> App {
    let mut app = reading_app(style);
    app.keys_capable = true;
    app
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
    let mut app = capable_app(Keys::Emacs);
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
    let (queue, report) = app.motion_hints();
    let legend = format!("{queue}  {report}");
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
fn given_emacs_keys_when_ctrl_g_is_pressed_then_the_prompt_is_put_down_and_kept() {
    // C-g is emacs's way out of a command, and a draft is the one thing on the
    // screen a second keypress cannot bring back: it goes down, not away.
    let mut app = app_with(2, Keys::Emacs);
    app.handle_key(key(KeyCode::Char('a')));
    for c in "half".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(ctrl('g'));
    assert!(app.is_prompting(), "C-g threw the words away");
    assert!(app.prompt_line().contains("half"), "C-g lost the draft");
    let screen = screen(&app, 90, 20);
    assert!(screen.contains("append:"), "the draft is not on the screen:\n{screen}");
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
fn given_any_style_when_the_motion_hints_are_read_then_each_window_has_its_own_keys() {
    for style in [Keys::Plain, Keys::Vim, Keys::Emacs] {
        let app = app_with(2, style);
        let (queue, report) = app.motion_hints();
        // Each rule is drawn on the window it moves, so the hint never has to
        // say which window: two lists that had to repeat their names cost the
        // row the width its operations needed.
        assert!(queue.contains("level"), "{style:?}: the queue rule names no level key: {queue}");
        assert!(!report.contains("level"), "{style:?}: the report rule names level keys: {report}");
        assert!(!queue.contains("report"), "{style:?}: the queue rule names the report: {queue}");
        assert!(!queue.contains("pane cycle"), "{style:?}: a pane cycle implies a focus we do not have");
        assert!(!queue.is_empty() && !report.is_empty(), "{style:?}: a window rule names nothing");
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
fn given_a_task_of_two_turns_when_its_report_is_read_then_each_turn_names_its_work() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Turn::doing(pearl_core::Cost::turn(1000, 100, 500), "read crates/x.rs"));
    queue.charge(id, pearl_core::Turn::doing(pearl_core::Cost::turn(500, 50, 250), "answer"));
    queue.complete(id, "the answer").unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 120, 24);
    assert!(
        screen.contains("turn 1  read crates/x.rs  ↑1.0k ↓100 0.5s"),
        "a bill with no subject is not a reading:\n{screen}"
    );
    assert!(screen.contains("turn 2  answer  ↑500 ↓50 0.2s"), "the answer turn was unnamed:\n{screen}");
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
    // Home and End are the plain keyboard's keys: the other two reach the same
    // two rows from the letters their hands already rest on.
    let (first, last): (Vec<KeyEvent>, Vec<KeyEvent>) = match style {
        Keys::Plain => (vec![key(KeyCode::Home)], vec![key(KeyCode::End)]),
        Keys::Vim => (
            vec![key(KeyCode::Char('g')), key(KeyCode::Char('g'))],
            vec![key(KeyCode::Char('G'))],
        ),
        Keys::Emacs => (
            vec![chord(KeyCode::Char('/'), KeyModifiers::ALT)],
            vec![chord(KeyCode::Char('\\'), KeyModifiers::ALT)],
        ),
    };
    for k in last {
        app.handle_key(k);
    }
    assert_eq!(focused(&app), rows[rows.len() - 1], "{style:?} could not reach the last row");
    for k in first {
        app.handle_key(k);
    }
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
    match style {
        Keys::Plain => {
            app.handle_key(alt(KeyCode::Home));
            assert!(app.report_back > 100, "{style:?} could not reach the oldest line");
            app.handle_key(alt(KeyCode::End));
        }
        Keys::Vim => {
            app.handle_key(alt(KeyCode::Char('g')));
            app.handle_key(alt(KeyCode::Char('g')));
            assert!(app.report_back > 100, "{style:?} could not reach the oldest line");
            app.handle_key(alt(KeyCode::Char('G')));
        }
        Keys::Emacs => {
            app.handle_key(alt(KeyCode::Char('<')));
            assert!(app.report_back > 100, "{style:?} could not reach the oldest line");
            app.handle_key(alt(KeyCode::Char('>')));
        }
    }
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
    let mut app = capable_app(Keys::Emacs);
    app.report_page.set(10);
    app.handle_key(chord(KeyCode::Char('V'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 10, "C-M-S-v did not page the report back");
}

#[rstest]
fn given_emacs_keys_when_the_other_window_is_jumped_then_the_report_shows_its_ends() {
    let mut app = capable_app(Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('<'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert!(app.report_back > 100, "C-M-< did not reach the top of the report");
    app.handle_key(chord(KeyCode::Char('>'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 0, "C-M-> did not come back to the newest");
}

#[rstest]
fn given_emacs_keys_on_a_terminal_without_the_encoding_when_alt_shift_p_is_pressed_then_the_report_pages_back() {
    // Shift beside Ctrl is what the encoding cannot carry; Shift beside Alt
    // arrives as the capital letter, which is the spelling vim's report page
    // already uses for the same motion.
    let mut app = reading_app(Keys::Emacs);
    app.report_page.set(10);
    app.handle_key(chord(KeyCode::Char('P'), KeyModifiers::ALT | KeyModifiers::SHIFT));
    assert_eq!(app.report_back, 10, "M-P did not page the report back");
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(app.report_back, 0, "C-M-v did not come forward");
}

#[rstest]
fn given_emacs_keys_on_a_terminal_without_the_encoding_when_alt_less_is_pressed_then_the_report_shows_its_ends() {
    let mut app = reading_app(Keys::Emacs);
    app.handle_key(alt(KeyCode::Char('<')));
    assert!(app.report_back > 100, "M-< did not reach the top of the report");
    app.handle_key(alt(KeyCode::Char('>')));
    assert_eq!(app.report_back, 0, "M-> did not come back to the newest");
}

#[rstest]
fn given_emacs_keys_when_the_legend_is_read_then_it_names_only_keys_the_terminal_can_send() {
    let legacy = reading_app(Keys::Emacs).motion_hints().1;
    assert!(legacy.contains("C-M-v|M-P"), "the fallback page is not named: {legacy}");
    assert!(!legacy.contains("C-M-S-v"), "a terminal without the encoding was promised Shift: {legacy}");
    let capable = capable_app(Keys::Emacs).motion_hints().1;
    assert!(capable.contains("C-M-v|C-M-S-v"), "a capable terminal lost its own page: {capable}");
    assert!(!capable.contains("M-P"), "a capable terminal was also offered the fallback: {capable}");
}

#[rstest]
fn given_emacs_keys_on_a_terminal_without_the_encoding_when_the_help_is_read_then_the_report_row_names_the_page_it_can_send() {
    let app = reading_app(Keys::Emacs);
    let row = app
        .help_groups()
        .into_iter()
        .flat_map(|(_, _, rows)| rows)
        .find(|row| row.keys == "report")
        .expect("no report row in the help");
    assert!(row.en.contains("C-M-v|M-P"), "the help promised a key this terminal cannot send: {}", row.en);
}

#[rstest]
fn given_a_terminal_that_sends_command_when_command_n_is_pressed_then_the_report_moves_one_line() {
    // Command arrives as `SUPER` where the encoding names its modifiers, and
    // Meta is a modifier rather than a key: the row prints `M-`, and either
    // Command or Option answers it.
    let mut app = capable_app(Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('p'), KeyModifiers::SUPER));
    assert_eq!(app.report_back, 1, "Command+p did not scroll the report back");
    app.handle_key(chord(KeyCode::Char('n'), KeyModifiers::SUPER));
    assert_eq!(app.report_back, 0, "Command+n did not come forward");
}

#[rstest]
fn given_a_terminal_that_sends_command_when_command_less_is_pressed_then_the_report_shows_its_ends() {
    let mut app = capable_app(Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('<'), KeyModifiers::SUPER));
    assert!(app.report_back > 100, "Command+< did not reach the top of the report");
    app.handle_key(chord(KeyCode::Char('>'), KeyModifiers::SUPER));
    assert_eq!(app.report_back, 0, "Command+> did not come back to the newest");
}

#[rstest]
fn given_vim_keys_when_command_j_is_pressed_then_the_report_moves_one_line() {
    let mut app = capable_app(Keys::Vim);
    app.handle_key(chord(KeyCode::Char('k'), KeyModifiers::SUPER));
    assert_eq!(app.report_back, 1, "Command+k did not scroll the vim report back");
    app.handle_key(chord(KeyCode::Char('j'), KeyModifiers::SUPER));
    assert_eq!(app.report_back, 0, "Command+j did not come forward");
}

#[rstest]
fn given_a_prompt_with_no_text_when_return_is_pressed_then_no_task_is_placed() {
    let mut app = App::new(Queue::new());
    app.handle_key(chord(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key(chord(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.queue.view().len(), 0, "Enter on nothing made a row");
    let said = app.notice.clone().unwrap_or_default();
    assert!(said.contains("needs a word"), "the refusal said nothing useful: {said}");
}

#[rstest]
fn given_a_prompt_when_command_letter_is_pressed_then_no_letter_is_typed() {
    let mut app = capable_app(Keys::Emacs);
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(chord(KeyCode::Char('x'), KeyModifiers::SUPER));
    let screen = screen(&app, 90, 20);
    assert!(!screen.contains("append: x"), "a Command chord was typed as text:\n{screen}");
}

#[rstest]
#[case(Keys::Emacs, true)]
#[case(Keys::Emacs, false)]
fn given_a_look_when_the_help_is_read_then_every_row_names_a_key(#[case] style: Keys, #[case] capable: bool) {
    // The help is a table of keys: what the terminal can send was never a key,
    // and a row about it reads like a binding that can be pressed.
    let mut app = reading_app(style);
    app.keys_capable = capable;
    for (_, _, rows) in app.help_groups() {
        for row in rows {
            assert!(
                !row.keys.is_empty() && !row.keys.starts_with("this terminal"),
                "the help grew a row that is not a key: {:?} {:?}",
                row.keys,
                row.en
            );
        }
    }
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_style_when_its_legend_is_read_then_it_names_the_ends_of_both_windows(
    #[case] style: Keys,
) {
    let app = app_with(2, style);
    let (q, r) = app.motion_hints();
    // The ends are the pair that crosses the window, and each rule names its
    // own: the queue's last task / first task, the report's oldest / newest.
    let (queue, report) = match style {
        Keys::Plain => ("Home|End ends", "Alt+Home|End ends"),
        Keys::Vim => ("gg|G ends", "M-gg|M-G ends"),
        Keys::Emacs => ("M-/|M-\\ ends", "M-<|M-> ends"),
    };
    assert!(q.contains(queue), "{style:?} queue legend does not name its ends: {q}");
    assert!(r.contains(report), "{style:?} report legend does not name its ends: {r}");
}

/// The key screen, whole. Every row is a sentence, so the screen is taller
/// than a terminal and a story about what it teaches has to read all of it.
fn help_text(app: &mut App) -> String {
    app.handle_key(key(KeyCode::Char('?')));
    let mut all = String::new();
    let mut last = usize::MAX;
    while app.help_at != last {
        last = app.help_at;
        all.push_str(&screen(app, 140, 45));
        app.handle_key(key(KeyCode::PageDown));
    }
    all.push_str(&screen(app, 140, 45));
    all
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_a_task_is_in_hand_and_enter_is_pressed_then_the_hand_opens_for_editing(
    #[case] style: Keys,
) {
    let mut app = app_with(2, style);
    app.handle_key(key(KeyCode::Char('t')));
    app.handle_key(key(KeyCode::Enter));
    let line = app.prompt_line();
    assert!(
        line.starts_with("in hand: task 0"),
        "{style:?} sent Enter into a level instead of the hand: {line:?}"
    );
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_space_is_pressed_then_the_focused_task_stops_and_goes_on(
    #[case] style: Keys,
) {
    let mut app = app_with(2, style);
    let id = app.queue.focus().expect("nothing is focused");
    app.handle_key(key(KeyCode::Char(' ')));
    let paused = app.queue.get(id).expect("the task left the queue");
    assert!(
        paused.state == pearl_core::NodeState::Queued && paused.held,
        "{style:?} left space unbound or paused the wrong task"
    );
    app.handle_key(key(KeyCode::Char(' ')));
    assert!(
        !app.queue.get(id).expect("the task left the queue").held,
        "{style:?} could stop a task and not start it again"
    );
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_rewrite_is_pressed_then_the_focused_words_open_for_editing(
    #[case] style: Keys,
) {
    let mut app = app_with(2, style);
    let words = focused(&app);
    app.handle_key(key(KeyCode::Char('r')));
    let line = app.prompt_line();
    assert!(
        line.starts_with(&format!("rewrite: {words}")),
        "{style:?} spent the rewrite key on something else: {line:?}"
    );
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_place_is_pressed_with_an_empty_hand_then_it_says_why(
    #[case] style: Keys,
) {
    let mut app = app_with(2, style);
    app.handle_key(key(KeyCode::Char('p')));
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("the hand is empty"),
        "{style:?} answered place with silence: {notice:?}"
    );
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_help_is_read_then_it_teaches_that_keyboards_motions_and_no_other(
    #[case] style: Keys,
) {
    let mut app = app_with(2, style);
    let help = help_text(&mut app);
    let (own, another) = match style {
        Keys::Plain => ("⇧⇩ one task", "j|k one task"),
        Keys::Vim => ("j|k one task", "C-n|C-p one task"),
        Keys::Emacs => ("C-n|C-p one task", "j|k one task"),
    };
    assert!(help.contains(own), "{style:?} help does not teach its own queue motion");
    assert!(!help.contains(another), "{style:?} help teaches another keyboard's motion");
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_the_queue_rule_is_read_then_it_shows_that_keyboards_motions(
    #[case] style: Keys,
) {
    let app = app_with(2, style);
    let frame = screen(&app, 200, 12);
    let rule = frame
        .lines()
        .find(|l| l.contains(" queue "))
        .unwrap_or_default()
        .to_string();
    let (own, another) = match style {
        Keys::Plain => ("⇧⇩ task", "j|k task"),
        Keys::Vim => ("j|k task", "⇧⇩ task"),
        Keys::Emacs => ("C-n|C-p task", "j|k task"),
    };
    assert!(rule.contains(own), "{style:?} queue rule does not show its own motions: {rule}");
    assert!(!rule.contains(another), "{style:?} queue rule shows another keyboard's motions: {rule}");
    // And the row has stopped paying for them: what moves a window is written
    // on the window, so the row's width is the queue's own verbs.
    let row = frame.lines().last().unwrap_or_default().to_string();
    assert!(!row.contains("queue "), "{style:?} row still names the queue's motions: {row}");
    assert!(
        !rule.contains("panes"),
        "{style:?} queue rule still carries the panes keys: {rule}"
    );
    assert!(row.contains("1|2|3 panes"), "{style:?} row does not name the panes keys: {row}");
}

#[rstest]
#[case(Keys::Plain)]
#[case(Keys::Vim)]
#[case(Keys::Emacs)]
fn given_any_keyboard_when_help_is_read_then_the_key_that_enters_a_task_is_named(
    #[case] style: Keys,
) {
    // Enter walks in from every keyboard -- emacs has no other in-key -- so
    // every keyboard has to print it, and the hand line overrides it only
    // while a request is held.
    let mut app = app_with(2, style);
    let help = help_text(&mut app);
    assert!(
        help.contains("in to the tasks inside"),
        "{style:?} help does not say how to go into a task"
    );
}

#[rstest]
fn given_emacs_keys_when_help_is_read_then_the_report_ends_are_two_keys_not_one_word() {
    // `M-</M->` is a key nobody can press: the slash that separates the pair
    // is the same slash that is part of `M-/` next door. The bar is not in
    // either name, so it is the one separator these keys can share.
    let mut app = app_with(2, Keys::Emacs);
    let help = help_text(&mut app);
    assert!(help.contains("M-<|M->"), "the report's ends print as one token:
{help}");
    assert!(!help.contains("M-</M->"), "the ambiguous spelling came back:
{help}");
    assert!(help.contains("M-/|M-\\"), "the queue's ends print as one token:
{help}");
}

// A terminal that speaks the kitty encoding names the key and the modifiers
// apart, so Shift arrives as a bit rather than as the capital or the symbol it
// makes on a keyboard. Every arm below spells what the legacy encoding puts on
// the wire -- the capital, the question mark, the bracket -- because that is
// the spelling the help offers and the one a terminal without the encoding
// sends. The two spellings have to be one press before anything looks at it,
// or a key that is printed in the help is a key that cannot be pressed.
#[test]
fn given_a_terminal_that_sends_shift_as_a_modifier_when_shift_slash_is_pressed_then_the_help_opens() {
    let mut app = app_with(2, Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('/'), KeyModifiers::SHIFT));
    assert!(app.help, "Shift+/ did not open the help it is spelled as");
}

#[test]
fn given_a_terminal_that_sends_shift_as_a_modifier_when_shift_s_is_pressed_then_a_save_is_asked_for() {
    let mut app = app_with(2, Keys::Vim);
    app.handle_key(chord(KeyCode::Char('s'), KeyModifiers::SHIFT));
    assert!(app.save_requested, "Shift+s did not ask for the save it is spelled as");
}

#[test]
fn given_a_terminal_that_sends_shift_as_a_modifier_when_shift_g_is_pressed_then_the_queue_lists_its_last_task() {
    let mut app = app_with(6, Keys::Vim);
    app.handle_key(chord(KeyCode::Char('g'), KeyModifiers::SHIFT));
    let last = focused(&app);
    app.handle_key(key(KeyCode::Char('g')));
    app.handle_key(key(KeyCode::Char('g')));
    assert_ne!(focused(&app), last, "Shift+G did not reach the last task");
}

#[test]
fn given_a_terminal_that_sends_shift_as_a_modifier_when_the_emacs_page_back_is_pressed_then_the_report_pages() {
    let mut app = capable_app(Keys::Emacs);
    app.report_page.set(10);
    for _ in 0..12 {
        app.handle_key(alt(KeyCode::Char('p')));
    }
    assert_eq!(app.report_back, 12);
    app.handle_key(chord(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT));
    assert_eq!(app.report_back, 22, "C-M-S-v did not page the report back");
}

#[test]
fn given_a_terminal_that_sends_shift_as_a_modifier_when_the_emacs_ends_are_pressed_then_the_report_shows_them() {
    let mut app = capable_app(Keys::Emacs);
    app.handle_key(chord(KeyCode::Char('.'), KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT));
    let oldest = app.report_back;
    app.handle_key(chord(KeyCode::Char(','), KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT));
    assert!(app.report_back > oldest, "C-M-< did not reach the oldest line");
}
