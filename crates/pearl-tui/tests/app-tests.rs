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
