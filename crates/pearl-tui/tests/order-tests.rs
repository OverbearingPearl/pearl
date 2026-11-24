//! Stories for the order the human sees: what you type last is at the bottom.
//!
//! `a` is append. A queue that inserts each new task next to the cursor instead
//! of at the end shows the human a different order from the one they typed, and
//! nothing on the screen explains why.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::App;
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn send(app: &mut App, text: &str) {
    app.handle_key(key(KeyCode::Char('a')));
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
}

#[rstest]
fn given_three_tasks_sent_in_order_when_the_queue_is_read_then_that_is_the_order() {
    let mut app = App::new(Queue::new());
    send(&mut app, "hello1");
    send(&mut app, "hello2");
    send(&mut app, "hello3");
    let titles: Vec<String> = app.queue.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(titles, ["hello1", "hello2", "hello3"]);
}

#[rstest]
fn given_three_tasks_sent_in_order_when_the_screen_is_drawn_then_that_is_the_order() {
    let mut app = App::new(Queue::new());
    send(&mut app, "hello1");
    send(&mut app, "hello2");
    send(&mut app, "hello3");
    let screen = pearl_tui::screen(&app, 80, 24);
    let one = screen.find("hello1").expect("hello1 missing");
    let two = screen.find("hello2").expect("hello2 missing");
    let three = screen.find("hello3").expect("hello3 missing");
    assert!(one < two && two < three, "the screen reordered the work:\n{screen}");
}

#[rstest]
fn given_a_task_in_the_hand_when_it_is_placed_after_the_focus_then_it_lands_there() {
    // `p` is the deliberate move: after this one, not at the end. It keeps its
    // meaning; only append changed.
    let queue = Queue::new();
    let (first, _) = queue.append("first");
    queue.append("second");
    let (third, _) = queue.append("third");
    let mut app = App::new(queue);

    // Take the last one into the hand, then place it after the first.
    app.queue.focus_on(third).unwrap();
    app.handle_key(key(KeyCode::Char('t')));
    app.queue.focus_on(first).unwrap();
    app.handle_key(key(KeyCode::Char('p')));

    let titles: Vec<String> = app.queue.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(titles, ["first", "third", "second"]);
}

fn focused(app: &App) -> Option<String> {
    let focus = app.queue.focus()?;
    app.queue
        .view()
        .iter()
        .find(|n| n.id == focus)
        .map(|n| n.title.clone())
}

fn id_of(app: &App, title: &str) -> pearl_core::TaskId {
    app.queue
        .view()
        .iter()
        .find(|n| n.title == title)
        .map(|n| n.id)
        .unwrap_or_else(|| panic!("{title} missing"))
}

#[rstest]
fn given_two_tasks_when_the_second_is_sent_then_the_cursor_is_on_it() {
    // The human named it, so it is the one they are waiting for: the report
    // beside the list is the focused task's, and staying on the first showed
    // the previous answer while the new run wrote its own.
    let mut app = App::new(Queue::new());
    send(&mut app, "first");
    send(&mut app, "second");
    assert_eq!(focused(&app).as_deref(), Some("second"));
}

#[rstest]
fn given_two_tasks_when_another_is_inserted_now_then_the_cursor_is_on_it() {
    let mut app = App::new(Queue::new());
    send(&mut app, "first");
    send(&mut app, "second");
    app.handle_key(key(KeyCode::Char('i')));
    for c in "now".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let titles: Vec<String> = app.queue.view().iter().map(|n| n.title.clone()).collect();
    assert_eq!(titles, ["now", "first", "second"]);
    assert_eq!(focused(&app).as_deref(), Some("now"));
}

#[rstest]
fn given_three_tasks_when_each_running_one_settles_then_the_cursor_moves_on_and_stops_at_the_last() {
    // The queue is not only typed into: the runner starts and finishes tasks
    // while the human watches, so the cursor has to move with the queue or the
    // report goes on describing work that is over. The last task keeps it,
    // because the answer it just wrote is the thing there is to read.
    let mut app = App::new(Queue::new());
    send(&mut app, "first");
    send(&mut app, "second");
    send(&mut app, "third");
    app.queue.focus_on(id_of(&app, "first")).unwrap();
    app.follow();
    for (running, next) in [("first", "second"), ("second", "third")] {
        let id = id_of(&app, running);
        app.queue.start(id).unwrap();
        app.follow();
        app.queue.complete(id, "an answer").unwrap();
        app.follow();
        assert_eq!(focused(&app).as_deref(), Some(next), "after {running} settled");
    }
    let last = id_of(&app, "third");
    app.queue.start(last).unwrap();
    app.follow();
    app.queue.complete(last, "the answer").unwrap();
    app.follow();
    assert_eq!(focused(&app).as_deref(), Some("third"));
}

#[rstest]
fn given_a_settled_task_when_the_human_moves_back_to_it_then_the_cursor_stays_there() {
    // Reading is not queued work: a human who walked back to a finished task
    // asked for that task, and the cursor stays where they put it.
    let mut app = App::new(Queue::new());
    send(&mut app, "first");
    send(&mut app, "second");
    let first = id_of(&app, "first");
    app.queue.focus_on(first).unwrap();
    app.follow();
    app.queue.start(first).unwrap();
    app.follow();
    app.queue.complete(first, "answer").unwrap();
    app.follow();
    assert_eq!(focused(&app).as_deref(), Some("second"));
    app.handle_key(key(KeyCode::Up));
    app.follow();
    assert_eq!(focused(&app).as_deref(), Some("first"));
}
