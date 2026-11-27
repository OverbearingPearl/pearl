//! TEMPORARY repro: what an empty draft is told when it is put down or dropped.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Keys, Queue, Settings};
use pearl_tui::{App, Mode, screen};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn settings(style: Keys) -> Settings {
    let s = Settings::new("m");
    s.apply(&format!("keys {}", style.as_str())).unwrap();
    s
}

fn words(app: &App) -> String {
    match &app.mode {
        Mode::Input(input) => input.draft.clone().into_text(),
        Mode::Normal => String::from("<no draft>"),
    }
}

fn focused(app: &App) -> Option<bool> {
    match &app.mode {
        Mode::Input(input) => Some(input.focused),
        Mode::Normal => None,
    }
}

fn show(label: &str, app: &App) {
    println!(
        "{label:<26} focused={:<6} draft={:<12} notice={:?}",
        format!("{:?}", focused(app)),
        format!("{:?}", words(app)),
        app.notice.as_deref().unwrap_or("<none>"),
    );
}

#[test]
fn repro_an_empty_draft_is_told_its_words_are_kept() {
    for style in [Keys::Emacs, Keys::Vim, Keys::Plain] {
        println!("--- {style:?} ---");
        let mut app = App::new(Queue::new()).with_settings(settings(style));
        app.queue.append("a task");

        // `a` opens the line with nothing written in it.
        app.handle_key(key(KeyCode::Char('a')));
        show("after 'a' (empty draft)", &app);
        assert_eq!(words(&app), "", "the line did not open empty");

        // `Tab` gives the keys back to the queue.
        app.handle_key(key(KeyCode::Tab));
        show("after Tab", &app);
        let kept = app.notice.clone().unwrap_or_default();

        // `Esc` drops it -- there is nothing in it to drop.
        app.handle_key(key(KeyCode::Esc));
        show("after Esc", &app);
        let dropped = app.notice.clone().unwrap_or_default();

        // And the screen, so the words a human actually meets are checked.
        let out = screen(&app, 120, 20);
        let line = out
            .lines()
            .find(|l| l.contains("dropped") || l.contains("kept") || l.contains("draft"))
            .unwrap_or("<no notice row>")
            .trim_end();
        println!("notice row on screen: {line:?}");

        println!("claims words are kept:   {kept:?}");
        println!("claims something dropped: {dropped:?}");
        println!();
    }
}

#[test]
fn repro_escape_twice_on_an_empty_draft() {
    let mut app = App::new(Queue::new()).with_settings(settings(Keys::Emacs));
    app.queue.append("a task");
    app.handle_key(key(KeyCode::Char('a')));
    show("open, empty", &app);
    // The editor answers Esc first: in emacs's map, Esc is Cancel? Let us see.
    app.handle_key(key(KeyCode::Esc));
    show("after Esc 1", &app);
    app.handle_key(key(KeyCode::Esc));
    show("after Esc 2", &app);
    app.handle_key(key(KeyCode::Esc));
    show("after Esc 3", &app);
}

#[test]
fn repro_the_row_while_an_empty_draft_waits() {
    let mut app = App::new(Queue::new()).with_settings(settings(Keys::Emacs));
    app.queue.append("a task");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Tab));
    println!("row: {}", app.key_items().join(" · "));
    let out = screen(&app, 120, 20);
    println!("\n{sout}", sout = out);
}

#[test]
fn repro_an_empty_draft_blocks_a_second_one_and_says_words_are_at_stake() {
    let mut app = App::new(Queue::new()).with_settings(settings(Keys::Emacs));
    app.queue.append("a task");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Tab)); // blurred, still empty
    show("empty draft put down", &app);
    app.handle_key(key(KeyCode::Char('a'))); // a second line
    show("after 'a' again", &app);
    app.handle_key(key(KeyCode::Char('A'))); // a topic
    show("after 'A'", &app);
    app.handle_key(key(KeyCode::Char('/'))); // an aside
    show("after '/'", &app);
    app.handle_key(key(KeyCode::Char('s'))); // steer
    show("after 's'", &app);
}

#[test]
fn repro_an_empty_draft_can_be_submitted() {
    let mut app = App::new(Queue::new()).with_settings(settings(Keys::Emacs));
    let (id, _) = app.queue.append("a task");
    app.queue.focus_on(id).unwrap();
    println!("rows before: {}", app.queue.view().len());
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    show("after ⏎ on an empty line", &app);
    println!("rows after:  {}", app.queue.view().len());
    for row in app.queue.view() {
        println!("  {:?} state={:?}", row.title, row.state);
    }
}

#[test]
fn repro_escape_shapes_across_the_three_keyboards() {
    for style in [Keys::Plain, Keys::Emacs, Keys::Vim] {
        println!("--- {style:?} ---");
        let mut app = App::new(Queue::new()).with_settings(settings(style));
        app.queue.append("a task");
        app.handle_key(key(KeyCode::Char('a')));
        show("open, empty", &app);
        for n in 1..=4 {
            app.handle_key(key(KeyCode::Esc));
            show(&format!("Esc {n}"), &app);
        }
        println!();
    }
}

#[test]
fn repro_a_space_is_not_nothing() {
    let mut app = App::new(Queue::new()).with_settings(settings(Keys::Emacs));
    app.queue.append("a task");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char(' ')));
    show("a space typed", &app);
    app.handle_key(key(KeyCode::Esc));
    show("after Esc", &app);
    app.handle_key(key(KeyCode::Esc));
    show("after Esc 2", &app);
}
