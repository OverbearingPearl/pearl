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
