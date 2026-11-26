//! Stories for the interactive surface. Each drives the pure key handler with
//! synthetic keys and asserts on the queue, the mode, or the hand. The story is
//! the specification: it has to fail without the feature it names.

use crossterm::event::KeyCode;
use pearl_core::{NodeState, Queue};
use pearl_tui::{key, App, Mode};
use rstest::{fixture, rstest};

/// Given a surface over an empty queue.
#[fixture]
fn surface() -> App {
    App::new(Queue::new())
}

/// Given a surface with one queued task focused.
#[fixture]
fn one_queued() -> App {
    let mut q = Queue::new();
    q.append("write the parser");
    App::new(q)
}

#[rstest]
fn given_empty_surface_when_append_typed_then_a_task_appears(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    assert!(matches!(surface.mode, Mode::Input(_)));
    for c in "new task".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    assert_eq!(surface.mode, Mode::Normal);
    assert_eq!(surface.queue.len(), 1);
    assert_eq!(surface.queue.view()[0].title, "new task");
}

#[rstest]
fn given_input_when_escape_is_pressed_twice_then_the_draft_is_let_go_without_commit(mut surface: App) {
    // One Escape puts the draft down and keeps the words -- a paragraph is the
    // one thing on the screen nobody can rebuild. The second lets it go.
    surface.handle_key(key(KeyCode::Char('a')));
    surface.handle_key(key(KeyCode::Char('x')));
    surface.handle_key(key(KeyCode::Esc));
    assert!(surface.is_prompting(), "the first Escape threw the draft away");
    assert!(surface.prompt_line().contains('x'), "the words went with it");
    surface.handle_key(key(KeyCode::Esc));
    assert_eq!(surface.mode, Mode::Normal);
    assert_eq!(surface.queue.len(), 0);
}

#[rstest]
fn given_queued_task_when_taken_then_it_leaves_the_queue_for_the_hand(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    assert_eq!(one_queued.queue.len(), 0);
    assert!(one_queued.hand.is_some());
    assert_eq!(one_queued.hand.as_ref().unwrap().title, "write the parser");
    assert!(one_queued.prompt_line().contains("in hand"));
}

#[rstest]
fn given_hand_when_edited_and_placed_now_then_it_returns_at_the_front(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    one_queued.handle_key(key(KeyCode::Enter));
    // Clear the prefilled title and type a new one.
    for _ in 0.."write the parser".len() {
        one_queued.handle_key(key(KeyCode::Backspace));
    }
    for c in "rewritten".chars() {
        one_queued.handle_key(key(KeyCode::Char(c)));
    }
    one_queued.handle_key(key(KeyCode::Enter));
    one_queued.handle_key(key(KeyCode::Char('P')));
    assert!(one_queued.hand.is_none());
    assert_eq!(one_queued.queue.len(), 1);
    assert_eq!(one_queued.queue.view()[0].title, "rewritten");
}

#[rstest]
fn given_two_tasks_when_first_held_then_second_dispatches(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    for c in "A".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    surface.handle_key(key(KeyCode::Char('a')));
    surface.handle_key(key(KeyCode::Char('B')));
    surface.handle_key(key(KeyCode::Enter));
    // Focus is on A (the first); hold it.
    surface.queue.focus_on(surface.queue.view()[0].id).unwrap();
    surface.handle_key(key(KeyCode::Char(' ')));
    let a = surface.queue.view()[0].id;
    assert!(surface.queue.get(a).unwrap().held);
    assert_eq!(surface.queue.next_queued(), Some(surface.queue.view()[1].id));
}

#[rstest]
fn given_a_held_task_when_space_is_pressed_again_then_it_is_ready(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('a')));
    for c in "held".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    let id = surface.queue.view()[0].id;
    surface.handle_key(key(KeyCode::Char(' ')));
    assert!(surface.queue.get(id).unwrap().held);
    assert_eq!(surface.queue.next_queued(), None, "a held task was still dispatchable");
    surface.handle_key(key(KeyCode::Char(' ')));
    assert!(!surface.queue.get(id).unwrap().held);
    assert_eq!(surface.queue.next_queued(), Some(id));
}

#[rstest]
fn given_a_paused_task_when_space_is_pressed_then_it_runs_again(mut one_queued: App) {
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Paused);
    one_queued.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_a_queued_task_when_rewritten_then_it_keeps_its_place_and_its_neighbours(mut surface: App) {
    let (first, _) = surface.queue.append("first");
    let (second, _) = surface.queue.append("second");
    surface.queue.focus_on(second).unwrap();
    surface.handle_key(key(KeyCode::Char('r')));
    assert!(matches!(surface.mode, Mode::Input(_)), "rewrite did not open an input line");
    assert!(
        surface.prompt_line().contains("rewrite: second"),
        "rewrite did not start from the task's words: {}",
        surface.prompt_line()
    );
    for _ in 0.."second".len() {
        surface.handle_key(key(KeyCode::Backspace));
    }
    for c in "changed".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));
    let view = surface.queue.view();
    assert_eq!(view.iter().map(|n| n.id).collect::<Vec<_>>(), vec![first, second]);
    assert_eq!(view[1].title, "changed");
}

#[rstest]
fn given_running_task_when_steered_then_annotation_waits(mut one_queued: App) {
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.handle_key(key(KeyCode::Char('s')));
    for c in "also handle errors".chars() {
        one_queued.handle_key(key(KeyCode::Char(c)));
    }
    one_queued.handle_key(key(KeyCode::Enter));
    assert_eq!(one_queued.queue.get(id).unwrap().steering.len(), 1);
    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Running);
}

#[rstest]
fn given_surface_when_quit_then_quit_flag_set(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('q')));
    assert!(surface.quit);
}

#[rstest]
fn given_two_tasks_when_down_is_pressed_then_focus_moves_down(mut surface: App) {
    surface.queue.append("first");
    surface.queue.append("second");
    surface.queue.focus_on(surface.queue.view()[0].id).unwrap();
    surface.handle_key(key(KeyCode::Down));
    assert_eq!(surface.queue.focus(), Some(surface.queue.view()[1].id));
}

#[rstest]
fn given_a_task_in_hand_when_another_is_taken_then_the_first_is_kept_and_named(mut one_queued: App) {
    one_queued.queue.append("second task");
    one_queued.handle_key(key(KeyCode::Char('t')));
    let first = one_queued.hand.as_ref().expect("the first take left no hand").title.clone();
    one_queued.handle_key(key(KeyCode::Char('t')));
    assert_eq!(
        one_queued.hand.as_ref().map(|hand| hand.title.clone()),
        Some(first.clone()),
        "the second take overwrote the task already in the hand"
    );
    let notice = one_queued.notice.clone().unwrap_or_default();
    assert!(notice.contains("already in your hand"), "the refusal said nothing: {notice}");
    assert_eq!(one_queued.queue.len(), 1, "the second task left the queue anyway");
}

#[rstest]
fn given_a_task_in_hand_when_quit_is_pressed_then_the_surface_stays_and_says_why(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    one_queued.handle_key(key(KeyCode::Char('q')));
    assert!(!one_queued.quit, "quit threw the hand away without a word");
    assert!(one_queued.hand.is_some(), "the hand was lost on the way out");
    let notice = one_queued.notice.clone().unwrap_or_default();
    assert!(notice.contains("in your hand"), "the refusal said nothing: {notice}");
}

#[rstest]
fn given_a_task_in_hand_when_escape_arrives_then_the_task_is_dropped_and_said_so(mut one_queued: App) {
    one_queued.handle_key(key(KeyCode::Char('t')));
    assert!(one_queued.hand.is_some());
    one_queued.handle_key(key(KeyCode::Esc));
    assert!(one_queued.hand.is_none(), "Escape left the task in the hand");
    let notice = one_queued.notice.clone().unwrap_or_default();
    assert!(notice.contains("dropped"), "the drop said nothing: {notice}");
    assert_eq!(one_queued.queue.len(), 0);
}

#[rstest]
fn given_a_finished_task_when_cancel_is_pressed_then_the_row_leaves_the_list(one_queued: App) {
    let mut one_queued = one_queued;
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.queue.complete(id, "the answer").unwrap();
    one_queued.handle_key(key(KeyCode::Char('c')));
    assert_eq!(one_queued.queue.len(), 0, "the finished row stayed on the list");
}

#[rstest]
fn given_a_failed_task_when_retry_is_pressed_then_it_is_queued_again(one_queued: App) {
    let mut one_queued = one_queued;
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.queue.fail(id, "the provider refused").unwrap();
    one_queued.handle_key(key(KeyCode::Char('R')));
    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Queued);
}

#[rstest]
fn given_a_finished_task_when_the_files_key_is_pressed_then_the_loop_is_asked_what_it_touched(
    mut one_queued: App,
) {
    let id = one_queued.queue.view()[0].id;

    one_queued.handle_key(key(KeyCode::Char('f')));

    assert_eq!(one_queued.files_requested, Some(id));
    assert!(one_queued.notice.is_none(), "asking is not a refusal");
}

#[rstest]
fn given_a_task_waiting_on_a_question_when_s_is_pressed_then_the_line_says_answer_and_the_queue_hears_it(
    mut one_queued: App,
) {
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.queue.ask(id, "which crate?").unwrap();

    one_queued.handle_key(key(KeyCode::Char('s')));
    assert!(one_queued.prompt_line().starts_with("answer: "), "{}", one_queued.prompt_line());
    for c in "the tools one".chars() {
        one_queued.handle_key(key(KeyCode::Char(c)));
    }
    one_queued.handle_key(key(KeyCode::Enter));

    assert_eq!(one_queued.queue.get(id).unwrap().state, NodeState::Running);
    assert_eq!(one_queued.queue.get(id).unwrap().question, None);
    assert_eq!(one_queued.mode, Mode::Normal);
    assert!(one_queued.notice.is_none(), "{:?}", one_queued.notice);
}

#[rstest]
fn given_a_task_waiting_on_a_question_when_the_row_is_read_then_the_question_is_on_it(
    mut one_queued: App,
) {
    let id = one_queued.queue.view()[0].id;
    one_queued.queue.start(id).unwrap();
    one_queued.queue.ask(id, "which crate?").unwrap();

    let row = pearl_tui::screen(&one_queued, 120, 20);

    assert!(row.contains("? which crate?"), "{row}");
}

#[rstest]
fn given_a_task_that_stops_to_ask_when_the_surface_takes_its_turn_then_the_line_is_open_for_the_answer() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.ask(id, "which file?");
    let mut app = App::new(q);

    app.offer_question();

    assert!(app.is_prompting(), "the line did not open");
    assert!(app.prompt_line().starts_with("answer: "), "the line opened for something else: {}", app.prompt_line());
    // The question is where the eye is, not only on a row it has to find.
    assert_eq!(app.notice.as_deref(), Some("which file?"));
}

#[rstest]
fn given_a_draft_being_typed_when_a_task_asks_then_the_draft_is_not_taken_away() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    let mut app = App::new(q);
    app.handle_key(key(KeyCode::Char('a')));
    for c in "half a sentence".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }

    app.queue.ask(id, "which file?");
    app.offer_question();

    assert!(app.prompt_line().starts_with("append: "), "the question took the words away: {}", app.prompt_line());
    assert!(app.prompt_line().contains("half a sentence"));
}

#[rstest]
fn given_a_question_put_down_when_the_frame_is_drawn_again_then_the_line_stays_put_down() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.ask(id, "which file?");
    let mut app = App::new(q);
    app.offer_question();
    app.handle_key(key(KeyCode::Esc));

    app.offer_question();

    // Putting it down is not answering it: the words stay, and the line does
    // not take the caret back from someone who just moved away from it.
    assert!(app.prompt_line().contains("answer: "), "the line lost its question");
    app.handle_key(key(KeyCode::Tab));
    assert!(app.is_prompting());
}

#[rstest]
fn given_a_question_dropped_when_the_frame_is_drawn_again_then_it_does_not_come_back() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.ask(id, "which file?");
    let mut app = App::new(q);
    app.offer_question();
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal, "two Escapes did not drop the line");

    app.offer_question();

    assert_eq!(app.mode, Mode::Normal, "a question refused once came back by itself");
    app.handle_key(key(KeyCode::Char('s')));
    assert!(app.prompt_line().starts_with("answer: "), "the answer key stopped working: {}", app.prompt_line());
}

#[rstest]
fn given_two_tasks_waiting_when_the_first_is_answered_then_the_second_is_offered() {
    let mut q = Queue::new();
    let (first, _) = q.append("write the parser");
    let (second, _) = q.append("write the printer");
    for id in [first, second] {
        q.start(id).unwrap();
        q.ask(id, format!("{id}?"));
    }
    let mut app = App::new(q);

    app.offer_question();
    for c in "one".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    app.offer_question();

    assert!(app.prompt_line().starts_with("answer: "), "the second question was never offered: {}", app.prompt_line());
    assert_eq!(app.notice.as_deref(), Some(format!("{second}?").as_str()));
}

#[rstest]
fn given_a_task_answered_once_when_it_asks_again_then_the_line_opens_itself_again() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.ask(id, "which file?");
    let mut app = App::new(q);
    // Put the first question down hard, then answer the task's next one.
    app.offer_question();
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('s')));
    for c in "src/lib.rs".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));

    app.queue.ask(id, "and the module?");
    app.offer_question();

    assert!(app.prompt_line().starts_with("answer: "), "a second question never opened: {}", app.prompt_line());
    assert_eq!(app.notice.as_deref(), Some("and the module?"));
}

#[rstest]
fn given_a_finished_task_when_steer_is_pressed_then_it_says_why_without_asking_for_words() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "done").unwrap();
    let mut app = App::new(q);

    app.handle_key(key(KeyCode::Char('s')));

    assert_eq!(app.mode, Mode::Normal, "the line asked for words the queue would refuse");
    let notice = app.notice.clone().unwrap_or_default();
    assert!(notice.starts_with("steer:"), "the refusal does not name the key: {notice}");
    assert!(notice.contains("done"), "the refusal does not say what the task is: {notice}");
}

#[rstest]
fn given_a_finished_task_when_rewrite_is_pressed_then_it_says_why_without_asking_for_words() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "done").unwrap();
    let mut app = App::new(q);

    app.handle_key(key(KeyCode::Char('r')));

    assert_eq!(app.mode, Mode::Normal, "the line opened over a settled task");
    assert!(app.notice.clone().unwrap_or_default().starts_with("rewrite:"));
}

#[rstest]
fn given_a_task_running_when_it_finishes_before_the_note_is_entered_then_the_words_are_kept() {
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    let mut app = App::new(q);
    app.handle_key(key(KeyCode::Char('s')));
    for c in "prefer the other file".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }

    // The runner finishes the task while the human is still typing.
    app.queue.complete(id, "done").unwrap();
    app.handle_key(key(KeyCode::Enter));

    let notice = app.notice.clone().unwrap_or_default();
    assert!(notice.starts_with("steer:"), "no refusal was said: {notice}");
    assert!(app.prompt_line().contains("prefer the other file"), "the sentence was eaten");
    app.handle_key(key(KeyCode::Tab));
    assert!(app.is_prompting(), "the kept words cannot be reached");
}

#[rstest]
fn given_empty_surface_when_a_capital_a_is_typed_then_a_topic_heads_the_list(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('A')));
    assert!(matches!(surface.mode, Mode::Input(_)), "A opened nothing");
    for c in "parser".chars() {
        surface.handle_key(key(KeyCode::Char(c)));
    }
    surface.handle_key(key(KeyCode::Enter));

    assert_eq!(surface.queue.view()[0].title, "parser");
    assert_eq!(surface.queue.view()[0].form, pearl_core::Form::Topic);
}

#[rstest]
fn given_a_task_in_hand_and_a_topic_under_the_cursor_when_p_arrives_then_it_goes_inside(
    one_queued: App,
) {
    let mut surface = one_queued;
    surface.queue.topic("parser").unwrap();
    // Take the task, then put the cursor on the heading.
    surface.handle_key(key(KeyCode::Char('t')));
    let topic = surface
        .queue
        .view()
        .iter()
        .find(|n| n.form == pearl_core::Form::Topic)
        .map(|n| n.id)
        .unwrap();
    surface.queue.focus_on(topic).unwrap();

    surface.handle_key(key(KeyCode::Char('p')));

    let inside = surface.queue.get(topic).unwrap().children.clone();
    assert_eq!(inside.len(), 1, "the hand landed beside the topic, not in it");
}

#[rstest]
fn given_a_topic_with_unrun_parts_when_c_arrives_then_the_heading_goes_and_they_stay(
    surface: App,
) {
    let mut surface = surface;
    let (topic, _) = surface.queue.topic("parser").unwrap();
    surface.queue.add_child(topic, "lexer").unwrap();
    surface.queue.focus_on(topic).unwrap();

    surface.handle_key(key(KeyCode::Char('c')));

    assert!(surface.queue.get(topic).is_none(), "the heading stayed");
    assert_eq!(surface.queue.view()[0].title, "lexer");
}
