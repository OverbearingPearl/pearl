//! TEMPORARY repro: can a topic be renamed or rewritten, and does the row
//! say so?

use crossterm::event::KeyCode;
use pearl_core::Queue;
use pearl_tui::{App, key};

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
}

#[test]
fn repro_queued_topic_rename_by_r() {
    let q = Queue::new();
    let (topic, _) = q.append("topicA");
    q.promote(topic).unwrap();
    q.add_child(topic, "part one").unwrap();
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();

    let row: Vec<String> = app.key_groups().into_iter().flat_map(|(_, v)| v).collect();
    println!("row for a QUEUED topic: {row:?}");
    println!("can(Rewrite) = {}", app.queue.can(topic, pearl_core::Verb::Rewrite));
    println!("can(Take)    = {}", app.queue.can(topic, pearl_core::Verb::Take));

    app.handle_key(key(KeyCode::Char('r')));
    println!("after r: mode={:?} prompt={:?}", app.mode, app.prompt_line());
    type_text(&mut app, "topicA renamed");
    println!(
        "after typing: view={:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(),
        app.notice
    );
}

#[test]
fn repro_settled_topic_rename_by_r() {
    let q = Queue::new();
    let (topic, _) = q.append("done topic");
    q.promote(topic).unwrap();
    let (part, _) = q.add_child(topic, "part one").unwrap();
    q.start(part).unwrap();
    q.complete(part, "part one's answer").unwrap();
    println!("topic state = {:?}", q.get(topic).unwrap().state);

    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();
    let row: Vec<String> = app.key_groups().into_iter().flat_map(|(_, v)| v).collect();
    println!("row for a SETTLED topic: {row:?}");

    app.handle_key(key(KeyCode::Char('r')));
    println!(
        "after r on a settled topic: notice={:?} view={:?}",
        app.notice,
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn repro_topic_rename_through_the_hand() {
    let q = Queue::new();
    let (topic, _) = q.append("hand topic");
    q.promote(topic).unwrap();
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();

    app.handle_key(key(KeyCode::Char('t')));
    println!("after t: hand={:?} notice={:?}", app.hand, app.notice);
    app.handle_key(key(KeyCode::Char('r')));
    println!("after r: mode={:?} prompt={:?}", app.mode, app.prompt_line());
    type_text(&mut app, "hand topic renamed");
    println!(
        "after typing: view={:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(),
        app.notice
    );
}

#[test]
fn repro_rename_reaches_the_llm_digest() {
    let q = Queue::new();
    let (topic, _) = q.append("old name");
    q.promote(topic).unwrap();
    q.add_child(topic, "a part").unwrap();
    println!("digest before: {:?}", q.digest(None));
    q.retitle(topic, "new name").unwrap();
    println!("digest after retitle: {:?}", q.digest(None));
    q.redirect(topic, "third name").unwrap();
    println!("digest after redirect: {:?}", q.digest(None));
}

#[test]
fn repro_rename_a_topic_with_parts_that_ran() {
    // Does renaming a topic whose parts have run change the conversation the
    // parts read? The turns are stamped with the topic's *id*, so they should
    // not move -- this prints the ids and titles to show the two are separate.
    let q = Queue::new();
    let (topic, _) = q.append("the label");
    q.promote(topic).unwrap();
    let (part, _) = q.add_child(topic, "a part").unwrap();
    q.start(part).unwrap();
    q.complete(part, "said").unwrap();
    println!("topic id before = {topic} title = {}", q.get(topic).unwrap().title);
    q.retitle(topic, "another label").unwrap();
    println!("topic id after  = {topic} title = {}", q.get(topic).unwrap().title);
    println!("owners after = {:?}", q.owners());
    println!("digest after rename: {:?}", q.digest(None));
}

#[test]
fn repro_settled_topic_rename_via_hand_then_place() {
    // The only path left for a finished topic: t, r, a. Does it keep the title,
    // the parts, and the place on the list?
    let q = Queue::new();
    let (topic, _) = q.append("done topic");
    q.promote(topic).unwrap();
    let (part, _) = q.add_child(topic, "part one").unwrap();
    q.start(part).unwrap();
    q.complete(part, "part one's answer").unwrap();

    let (later, _) = q.append("something after");
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();
    println!("before: {:?}", app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>());

    app.handle_key(key(KeyCode::Char('t')));
    app.handle_key(key(KeyCode::Char('r')));
    for _ in 0.."done topic".len() { app.handle_key(key(KeyCode::Backspace)); }
    for c in "renamed topic".chars() { app.handle_key(key(KeyCode::Char(c))); }
    app.handle_key(key(KeyCode::Enter));
    println!("in hand, title now = {:?}", app.queue.get(topic).map(|n| n.title));
    app.handle_key(key(KeyCode::Char('a')));
    println!("after placing: {:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(), app.notice);
    println!("parts of the topic = {:?}",
        app.queue.get(topic).map(|n| n.children.len()));
    let _ = later;
}

#[test]
fn repro_a_on_a_settled_topic() {
    let q = Queue::new();
    let (topic, _) = q.append("done topic");
    q.promote(topic).unwrap();
    let (part, _) = q.add_child(topic, "part one").unwrap();
    q.start(part).unwrap();
    q.complete(part, "answer").unwrap();
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();
    println!("can(Join) on settled topic = {}", app.queue.can(topic, pearl_core::Verb::Join));
    app.handle_key(key(KeyCode::Char('A')));
    println!("after A: mode={:?} notice={:?}", app.mode, app.notice);
}

#[test]
fn repro_what_does_the_row_offer_a_topic_that_works() {
    let q = Queue::new();
    let (other, _) = q.append("before it");
    let (topic, _) = q.append("the topic");
    q.promote(topic).unwrap();
    q.add_child(topic, "a part").unwrap();
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();
    let _ = other;
    println!("view before = {:?}", app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>());

    // `i now` -- offered in the row.
    app.handle_key(key(KeyCode::Char('i')));
    println!("after i: view={:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(), app.notice);

    // `␠ pause` -- offered in the row.
    app.handle_key(key(KeyCode::Char(' ')));
    println!("after space: held={:?} notice={:?}",
        app.queue.get(topic).map(|n| n.held), app.notice);

    // `c` -- not in the row, but the TUI dissolves.
    app.queue.focus_on(topic).unwrap();
    app.handle_key(key(KeyCode::Char('c')));
    println!("after c: view={:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(), app.notice);
}

#[test]
fn repro_row_keys_on_a_queued_topic_cleaned() {
    let q = Queue::new();
    let (_before, _) = q.append("before it");
    let (topic, _) = q.append("the topic");
    q.promote(topic).unwrap();
    q.add_child(topic, "a part").unwrap();
    let mut app = App::new(q);
    app.queue.focus_on(topic).unwrap();
    println!("mode at start = {:?}", app.mode);

    app.handle_key(key(KeyCode::Char(' ')));
    println!(
        "after ␠: mode={:?} held={:?} state={:?} notice={:?}",
        app.mode,
        app.queue.get(topic).map(|n| n.held),
        app.queue.get(topic).map(|n| n.state),
        app.notice
    );

    app.queue.focus_on(topic).unwrap();
    app.handle_key(key(KeyCode::Char('c')));
    println!(
        "after c: view={:?} notice={:?}",
        app.queue.view().iter().map(|n| n.title.clone()).collect::<Vec<_>>(),
        app.notice
    );
}
