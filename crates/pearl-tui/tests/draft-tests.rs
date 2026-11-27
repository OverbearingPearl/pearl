//! Stories for writing into the surface: a draft of many lines, the two
//! editors, and the way back to the queue without losing the words.
//!
//! The draft is the one thing on the screen nobody can rebuild: the queue is
//! written before it is lost and the report is already on disk, but a paragraph
//! half thought out exists only in the row it sits in. So the draft takes its
//! rows before the panes do, keeps them while the keys go elsewhere, and is
//! given up only when the human says so.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{App, caret, screen};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn emacs_app() -> App {
    let settings = pearl_core::Settings::new("m");
    settings.apply("keys emacs").unwrap();
    App::new(Queue::new()).with_settings(settings)
}

fn text(app: &App) -> String {
    match &app.mode {
        pearl_tui::Mode::Input(input) => input.draft.text().to_string(),
        other => panic!("no draft is open: {other:?}"),
    }
}

fn typing(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

fn rows(app: &App, width: u16, height: u16) -> Option<usize> {
    app.draft_placed(width as usize - 2, app.draft_cap(height as usize))
        .map(|placed| placed.lines.len())
}

fn finished(reply: &str) -> Queue {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    queue.complete(id, reply).unwrap();
    queue
}

#[test]
fn given_a_draft_of_five_lines_when_the_screen_is_read_then_the_draft_has_five_rows() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    app.paste("one\ntwo\nthree\nfour\nfive");

    assert_eq!(
        rows(&app, 80, 24),
        Some(5),
        "the draft did not take its rows"
    );

    let drawn = screen(&app, 80, 24);
    for line in ["one", "two", "three", "four", "five"] {
        assert!(
            drawn.contains(line),
            "line {line:?} is not on the screen:\n{drawn}"
        );
    }
    assert!(
        drawn.contains(" queue "),
        "the queue's rule was cut:\n{drawn}"
    );
}

#[test]
fn given_a_draft_longer_than_the_cap_when_it_is_drawn_then_the_prompt_stops_at_the_cap() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    let text: String = (0..30).map(|i| format!("l{i:02}\n")).collect();
    app.paste(&text);

    let height: u16 = 40;
    let cap = app.draft_cap(height as usize);
    assert_eq!(
        rows(&app, 80, height),
        Some(cap),
        "the draft outgrew its cap"
    );

    let drawn = screen(&app, 80, height);
    assert!(
        drawn.contains("l29"),
        "the line being written is not on the screen"
    );
    assert!(
        !drawn.contains("l00"),
        "the draft shows its first line, not the caret's"
    );
}

#[test]
fn given_a_draft_open_when_tab_is_pressed_then_the_keys_work_the_queue_and_tab_brings_it_back() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "half a thought");

    app.handle_key(key(KeyCode::Tab));
    assert!(
        app.prompt_line().contains("half a thought"),
        "Tab dropped the words: {:?}",
        app.prompt_line()
    );

    // The keys work the queue now, and a second draft would be a second set of
    // words: the surface says where the first one is instead.
    app.handle_key(key(KeyCode::Char('i')));
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("a draft is open"),
        "the surface opened a second draft: {notice:?}"
    );
    assert!(
        app.prompt_line().contains("half a thought"),
        "the words were replaced"
    );

    app.handle_key(key(KeyCode::Tab));
    typing(&mut app, " more");
    assert!(
        app.prompt_line().contains("half a thought more"),
        "the caret did not come back: {:?}",
        app.prompt_line()
    );
    assert!(caret(&app, 80, 24).is_some(), "the caret did not come back");
}

#[test]
fn given_nothing_being_written_when_tab_is_pressed_then_it_says_so() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Tab));
    assert_eq!(app.notice.as_deref(), Some("tab: nothing is being written"));
}

#[test]
fn given_a_plain_draft_when_shift_enter_arrives_then_the_line_breaks_and_enter_places_it() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "first");
    app.handle_key(with(KeyCode::Enter, KeyModifiers::SHIFT));
    typing(&mut app, "second");

    assert_eq!(
        rows(&app, 80, 24),
        Some(2),
        "the break did not make a second row"
    );

    app.handle_key(key(KeyCode::Enter));
    let node = app.queue.view()[0].clone();
    assert_eq!(
        node.title, "first\nsecond",
        "the break was lost in the queue"
    );
}

#[test]
fn given_emacs_keys_when_c_j_arrives_then_the_line_breaks() {
    let mut app = App::new(Queue::new());
    app.settings.apply("keys emacs").unwrap();
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "one");
    app.handle_key(with(KeyCode::Char('j'), KeyModifiers::CONTROL));
    typing(&mut app, "two");
    assert_eq!(app.prompt_line(), "append: one\ntwo");
}

#[test]
fn given_emacs_keys_when_the_line_is_moved_and_killed_then_the_words_come_back() {
    let mut app = App::new(Queue::new());
    app.settings.apply("keys emacs").unwrap();
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "abc");

    app.handle_key(with(KeyCode::Char('a'), KeyModifiers::CONTROL));
    typing(&mut app, "X");
    assert_eq!(
        app.prompt_line(),
        "append: Xabc",
        "C-a did not reach the line's start"
    );
    app.handle_key(with(KeyCode::Char('k'), KeyModifiers::CONTROL));
    app.handle_key(with(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(
        app.prompt_line(),
        "append: Xabc",
        "C-k then C-y lost the words"
    );
}

#[test]
fn given_vim_keys_when_the_caret_is_moved_and_the_letter_killed_then_undo_brings_it_back() {
    let mut app = App::new(Queue::new());
    app.settings.apply("keys vim").unwrap();
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "abc");

    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('0')));
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(
        app.prompt_line(),
        "append: bc",
        "0 then x did not delete the first letter"
    );
    app.handle_key(key(KeyCode::Char('u')));
    assert_eq!(
        app.prompt_line(),
        "append: abc",
        "u did not put the letter back"
    );
}

#[test]
fn given_vim_keys_when_escape_arrives_then_the_hint_names_the_way_back_to_typing() {
    let mut app = App::new(Queue::new());
    app.settings.apply("keys vim").unwrap();
    app.handle_key(key(KeyCode::Char('a')));
    // Which key breaks the line depends on what the terminal can send apart
    // from `⏎`; that the hint names one is what this story is about.
    assert!(
        app.draft_hint()
            .is_some_and(|hint| hint.ends_with("newline")),
        "the draft does not say how to break the line: {:?}",
        app.draft_hint()
    );

    app.handle_key(key(KeyCode::Esc));
    assert_eq!(
        app.draft_hint().as_deref(),
        Some("i insert"),
        "Normal mode looks exactly like Insert mode"
    );
    app.handle_key(key(KeyCode::Char('i')));
    typing(&mut app, "x");
    assert!(
        app.prompt_line().ends_with('x'),
        "i did not go back to typing"
    );
}

#[test]
fn given_a_finished_task_when_y_is_pressed_then_its_document_is_handed_over() {
    let mut app = App::new(finished("the parser is done"));
    app.handle_key(key(KeyCode::Char('y')));
    let doc = app.pending_copy.clone().expect("nothing was copied");
    assert!(
        doc.contains("write the parser"),
        "the words are not in the copy: {doc:?}"
    );
    assert!(
        doc.contains("the parser is done"),
        "the answer is not in the copy: {doc:?}"
    );
    assert!(!doc.contains("turn"), "the copy carries the bills: {doc:?}");
}

#[test]
fn given_the_text_copied_when_it_is_wrapped_then_it_asks_the_terminal_in_base64() {
    assert_eq!(pearl_tui::clipboard::osc52("hi"), "\u{1b}]52;c;aGk=\u{7}");
}

#[test]
fn given_a_draft_put_down_when_escape_arrives_then_the_words_are_dropped() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "words");
    app.handle_key(key(KeyCode::Esc));
    assert!(
        app.is_prompting(),
        "Escape threw the draft away at the first press"
    );
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.is_prompting(), "Escape did not let the draft go");
}

#[test]
fn given_a_line_nobody_wrote_in_when_escape_arrives_then_it_closes_and_nothing_is_kept() {
    // A line opened and left holds no words at all. Putting it down would
    // promise `Tab` back to a blank row and say the draft was kept, which is
    // two sentences about nothing: it closes, and the keys come back.
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Esc));

    assert!(!app.is_prompting(), "a blank line was put down, not closed");
    assert_eq!(app.mode, pearl_tui::Mode::Normal);
    assert_eq!(app.notice, None, "a blank line was reported as kept");
    let screen = screen(&app, 80, 20);
    assert!(
        !screen.contains("append:"),
        "the blank line is still drawn:\n{screen}"
    );
    assert!(
        !screen.contains("Tab to return"),
        "the frame offers a way back to nothing:\n{screen}"
    );
}

#[test]
fn given_a_line_holding_only_spaces_when_escape_arrives_then_it_closes_as_an_empty_one() {
    // Spaces are not words: the queue refuses a title of spaces as no title,
    // and a draft that called them kept words would be arguing with it.
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "   ");
    app.handle_key(key(KeyCode::Esc));

    assert!(!app.is_prompting(), "a line of spaces was put down as words");
    assert_eq!(app.notice, None, "spaces were reported as kept words");
}

#[test]
fn given_words_taken_back_out_one_by_one_when_escape_arrives_then_the_line_closes_too() {
    // The same line, arrived at the other way: written and then taken back
    // out. What is left is the same nothing, and it is answered the same way.
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "wip");
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Backspace));
    }
    app.handle_key(key(KeyCode::Esc));

    assert!(!app.is_prompting(), "an emptied line was kept");
    assert_eq!(app.notice, None, "an emptied line was reported as kept");
}

#[test]
fn given_a_line_nobody_wrote_in_when_tab_is_pressed_then_it_closes_instead_of_waiting() {
    // `Tab` takes the keys to the queue and leaves the draft behind it. A
    // draft with nothing in it has nothing to wait there: it closes, and the
    // hint that would say `Tab to return` is never drawn over a blank row.
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Tab));

    assert!(
        !app.is_prompting(),
        "a blank line went to the background instead of closing"
    );
    assert_eq!(app.notice, None, "a line with no words was remarked on");
}

#[test]
fn given_a_draft_with_nothing_in_it_when_enter_arrives_then_it_says_a_task_needs_a_word() {
    let mut app = App::new(Queue::new());
    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(text(&app), "", "the line did not open empty");

    app.handle_key(key(KeyCode::Enter));

    // An empty draft is not a task. Nothing was typed, so nothing is lost, and
    // the line closes -- but it says which word was missing instead of
    // answering as though a task had been placed.
    assert!(
        app.queue.view().is_empty(),
        "an empty draft put a row on the queue"
    );
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("needs a word"),
        "the refusal does not say what is missing: {notice:?}"
    );
}

#[rstest]
fn given_emacs_keys_when_the_caret_is_mid_line_then_control_k_kills_the_rest_and_control_y_puts_it_back()
 {
    let mut app = emacs_app();
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "one two");
    app.handle_key(with(KeyCode::Char('b'), KeyModifiers::CONTROL));
    app.handle_key(with(KeyCode::Char('b'), KeyModifiers::CONTROL));
    app.handle_key(with(KeyCode::Char('b'), KeyModifiers::CONTROL));
    app.handle_key(with(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(
        text(&app),
        "one ",
        "C-k did not kill from the caret to the end of the line"
    );
    app.handle_key(with(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(
        text(&app),
        "one two",
        "C-y did not put the killed words back"
    );
}

#[rstest]
fn given_emacs_keys_when_control_d_and_meta_shifted_angle_arrive_then_they_edit_and_jump() {
    let mut app = emacs_app();
    app.handle_key(key(KeyCode::Char('a')));
    typing(&mut app, "one two");
    app.handle_key(with(KeyCode::Char('a'), KeyModifiers::CONTROL));
    app.handle_key(with(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(
        text(&app),
        "ne two",
        "C-d did not delete the character under the caret"
    );
    let shifted = KeyModifiers::ALT | KeyModifiers::SHIFT;
    app.handle_key(with(KeyCode::Char('>'), shifted));
    let placed = app
        .draft_placed(78, app.draft_cap(24))
        .expect("no draft is placed");
    // The row is the margin, the label and the words; the hint sits at the
    // far end and the caret belongs before it.
    let end = " append: ne two".chars().count();
    assert_eq!(
        placed.caret,
        (0, end),
        "M-> did not reach the end of the draft"
    );
}

#[rstest]
fn given_a_terminal_that_cannot_tell_control_j_apart_when_a_draft_opens_then_the_hint_names_meta_return()
 {
    let mut app = emacs_app();
    app.keys_capable = false;
    app.handle_key(key(KeyCode::Char('a')));
    let placed = app.draft_placed(78, 4).expect("no draft is placed");
    let line = &placed.lines[0];
    assert!(
        line.contains("M-⏎ newline"),
        "the hint still promises C-j:\n{line}"
    );
    assert!(
        !line.contains("C-j newline"),
        "the hint names a key the terminal folds:\n{line}"
    );
    assert!(
        app.draft_note().contains("C-j arrives as ⏎"),
        "the note does not say what arrives: {}",
        app.draft_note()
    );
}

#[rstest]
fn given_a_terminal_that_tells_control_j_apart_when_a_draft_opens_then_the_hint_names_it_and_nothing_is_reported()
 {
    let mut app = emacs_app();
    app.keys_capable = true;
    app.handle_key(key(KeyCode::Char('a')));
    let placed = app.draft_placed(78, 4).expect("no draft is placed");
    assert!(
        placed.lines[0].contains("C-j newline"),
        "the hint does not name C-j on a terminal that can send it:\n{}",
        placed.lines[0]
    );
    assert!(
        app.draft_note().is_empty(),
        "a reachable key was reported: {}",
        app.draft_note()
    );
}
