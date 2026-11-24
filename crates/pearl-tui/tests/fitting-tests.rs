//! Stories for a row that knows what fits in it.
//!
//! Two reports from the same session: a task typed past the width of the bottom
//! row vanished off its end, and a queue row whose title was long pushed the
//! bill -- the up and down tokens and the time -- clean off the screen behind
//! an ellipsis that named nothing. A row that is cut has to keep the part that
//! answers "what did this cost" and the part that says there is more.

use std::time::Duration;

use pearl_core::{NodeState, Queue};
use pearl_tui::{row_fg, screen, App, Mode};
use ratatui::style::Color;
use rstest::rstest;

fn app_with_tasks(titles: &[&str]) -> App {
    let queue = Queue::new();
    for title in titles {
        queue.append(*title);
    }
    App::new(queue)
}

fn typing(app: &mut App, text: &str) {
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('a'),
    ));
    for c in text.chars() {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char(c),
        ));
    }
}

#[rstest]
fn given_a_task_typed_past_the_window_when_the_bottom_row_is_drawn_then_its_end_is_what_is_shown() {
    let mut app = app_with_tasks(&[]);
    let typed = "a request long enough that its last words are the ones worth seeing";
    typing(&mut app, typed);
    let drawn = screen(&app, 60, 20);
    let bottom = drawn.lines().next_back().unwrap_or_default();
    assert!(
        bottom.contains("worth seeing"),
        "the end of what was typed is not on the bottom row:\n{drawn}"
    );
    assert!(bottom.starts_with('…') || bottom.contains("…"), "the cut was not marked: {bottom:?}");
    assert!(bottom.contains("append"), "the prompt stopped saying what it was: {bottom:?}");
}

#[rstest]
fn given_a_task_being_typed_when_the_bottom_row_is_drawn_then_it_is_not_dimmed() {
    let mut app = app_with_tasks(&["work"]);
    typing(&mut app, "say something");
    let drawn = screen(&app, 80, 20);
    let last = drawn.lines().count().saturating_sub(1) as u16;
    let fg = row_fg(&app, 80, 20, last);
    assert_ne!(fg, Color::DarkGray, "what the human is typing is drawn dimmer than the queue");
}

#[rstest]
fn given_a_title_longer_than_its_row_when_the_queue_is_read_then_the_bill_is_still_on_it() {
    let queue = Queue::new();
    let (id, _) = queue.append("a title long enough to run past any narrow window the human opens");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1234, 340, 2400));
    queue.complete(id, "done").unwrap();
    let _ = NodeState::Done;
    let app = App::new(queue);
    let drawn = screen(&app, 60, 20);
    assert!(drawn.contains("↑1.2k"), "the bill was pushed off the row it belongs to:\n{drawn}");
    assert!(drawn.contains("↓340"), "the bill was pushed off the row it belongs to:\n{drawn}");
}

#[rstest]
fn given_a_title_longer_than_its_row_when_the_queue_is_read_then_the_cut_is_marked() {
    let queue = Queue::new();
    queue.append("a title long enough to run past any narrow window the human opens");
    let app = App::new(queue);
    let drawn = screen(&app, 60, 20);
    let row = drawn.lines().find(|l| l.contains('▶')).unwrap_or_default();
    assert!(row.contains('…'), "a cut title was shown as the whole title: {row:?}");
    assert!(!row.contains("opens"), "the row is wider than the window: {row:?}");
}

#[rstest]
fn given_a_title_in_chinese_when_the_queue_is_read_then_the_row_stops_where_the_glass_does() {
    let queue = Queue::new();
    queue.append("这是一个足够长的中文任务标题，长到必须被截断，而且要按显示宽度去算而不是按字符数");
    let app = App::new(queue);
    let drawn = screen(&app, 40, 20);
    let row = drawn.lines().find(|l| l.contains('▶')).unwrap_or_default();
    // A wide character is one symbol and one empty cell in the drawn frame, so
    // the row's own length is the number of cells it took.
    assert!(
        row.chars().count() <= 40,
        "a row of Chinese ran past a 40-column window: {} cells: {row:?}",
        row.chars().count()
    );
}

#[rstest]
fn given_a_prompt_free_surface_when_the_bottom_row_is_drawn_then_the_keys_are_still_dim() {
    let app = app_with_tasks(&["work"]);
    let last = 19u16;
    let fg = row_fg(&app, 80, 20, last);
    assert_eq!(fg, Color::DarkGray, "the keys stopped being furniture");
    let _ = Mode::Normal;
    let _ = Duration::from_millis(0);
}

#[rstest]
fn given_a_title_the_row_had_to_cut_when_the_row_is_read_then_it_says_where_the_rest_is() {
    // A mark says the sentence was cut; it does not say the sentence is still
    // somewhere. A human who has never read the manual learns `v` from the row
    // that needs it, or not at all.
    let long = "read the whole of this request, including the last clause that a narrow row cannot carry";
    let drawn = screen(&app_with_tasks(&[long]), 70, 12);
    assert!(drawn.contains('…'), "the row did not admit the cut:\n{drawn}");
    assert!(drawn.contains("(v shows it all)"), "the cut row does not say how to read the rest:\n{drawn}");
}

#[rstest]
fn given_a_title_that_fits_when_the_row_is_read_then_it_says_nothing_extra() {
    let drawn = screen(&app_with_tasks(&["short"]), 70, 12);
    assert!(!drawn.contains("(v shows it all)"), "an uncut row offers a key it does not need:\n{drawn}");
}
