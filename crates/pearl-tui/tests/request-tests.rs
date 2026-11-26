//! Stories for reading a request the queue row had to cut.
//!
//! The queue row shortens a title where the glass ends and says so with a
//! mark, which is honest but not the whole request: the rest of the sentence
//! was unreachable. `v` opens the focused task's request on a screen of its
//! own, wrapped, so a request longer than the window can still be read.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{App, screen};
use rstest::rstest;

const LONG: &str = "read the whole of this request, including the last clause that a narrow queue row could not carry";

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app_holding(title: &str) -> App {
    let queue = Queue::new();
    queue.append(title);
    App::new(queue)
}

#[rstest]
fn given_a_title_the_row_had_to_cut_when_v_is_pressed_then_the_rest_of_it_is_on_screen() {
    let mut app = app_holding(LONG);
    let narrow = screen(&app, 40, 20);
    assert!(
        narrow.contains('…'),
        "the row did not admit the cut:\n{narrow}"
    );
    assert!(
        !narrow.contains("could not carry"),
        "the whole title fit; this story proves nothing"
    );
    app.handle_key(key(KeyCode::Char('v')));
    let read = screen(&app, 40, 20);
    assert!(
        read.contains("could not carry"),
        "the rest of the request is still hidden:\n{read}"
    );
}

#[rstest]
fn given_the_request_screen_when_another_key_is_pressed_then_the_queue_is_back() {
    let mut app = app_holding(LONG);
    app.handle_key(key(KeyCode::Char('v')));
    let reading = screen(&app, 80, 20);
    assert!(
        reading.contains("request  ("),
        "the request screen did not open:\n{reading}"
    );
    app.handle_key(key(KeyCode::Char('x')));
    let back = screen(&app, 80, 20);
    assert!(
        back.contains(" queue "),
        "the queue did not come back:\n{back}"
    );
    assert!(
        !back.contains("could not carry"),
        "the request screen stayed:\n{back}"
    );
}

#[rstest]
fn given_a_request_taller_than_the_window_when_it_is_scrolled_then_the_later_lines_are_shown() {
    let title = LONG.repeat(4);
    let mut app = app_holding(&title);
    app.handle_key(key(KeyCode::Char('v')));
    let first = screen(&app, 40, 12);
    assert!(
        !first.contains('╯') && !first.is_empty(),
        "no screen to scroll"
    );
    for _ in 0..6 {
        app.handle_key(key(KeyCode::Down));
    }
    let scrolled = screen(&app, 40, 12);
    assert_ne!(first, scrolled, "the request screen did not move");
    for _ in 0..40 {
        app.handle_key(key(KeyCode::Down));
    }
    let bottom = screen(&app, 40, 12);
    assert!(
        bottom.contains('…') || bottom.contains("carry"),
        "the end of the request is unreachable:\n{bottom}"
    );
}

#[rstest]
fn given_a_request_longer_than_the_window_when_it_is_read_then_every_line_fits_the_glass() {
    let mut app = app_holding(&LONG.repeat(2));
    app.handle_key(key(KeyCode::Char('v')));
    let read = screen(&app, 40, 20);
    assert!(
        read.contains("request  ("),
        "the request screen did not open:\n{read}"
    );
    for line in read.lines() {
        assert!(
            line.chars().count() <= 40,
            "a request line ran past the window: {line:?}"
        );
    }
}
