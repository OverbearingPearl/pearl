//! Stories for where the screen's space goes.
//!
//! Two panes in boxes spend four columns and four rows on furniture, and a
//! list that grows past its pane simply loses its tail -- including the task
//! the cursor is on. The space belongs to what the human is looking at.

use pearl_core::Queue;
use crossterm::event::KeyCode;
use pearl_tui::{key, screen, App, Panes};
use rstest::rstest;

fn app_with(tasks: usize) -> App {
    let queue = Queue::new();
    for i in 0..tasks {
        let _ = queue.append(format!("task number {i}"));
    }
    let mut app = App::new(queue);
    for _ in 1..tasks {
        app.handle_key(key(KeyCode::Char('j')));
    }
    app
}

#[rstest]
fn given_any_surface_when_it_is_drawn_then_no_vertical_rules_steal_columns() {
    let app = app_with(3);
    let screen = screen(&app, 60, 20);
    assert!(!screen.contains('│'), "the panes are still boxed:\n{screen}");
    assert!(!screen.contains('┌'), "the panes are still boxed:\n{screen}");
}

#[rstest]
fn given_more_tasks_than_rows_when_the_cursor_is_on_the_last_then_it_is_on_screen() {
    let app = app_with(40);
    let focused = app.queue.focus().and_then(|id| app.queue.get(id)).unwrap();
    let screen = screen(&app, 60, 20);
    assert!(screen.contains(&focused.title), "the focused task scrolled out of sight:\n{screen}");
}

#[rstest]
fn given_more_tasks_than_rows_when_the_list_is_shown_then_it_says_how_many_there_are() {
    let app = app_with(40);
    let screen = screen(&app, 60, 20);
    assert!(screen.contains("40"), "the list hid how much work there is:\n{screen}");
}

#[rstest]
fn given_a_long_report_when_it_is_shown_then_the_newest_lines_are_in_front() {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    let body: String = (0..200).map(|i| format!("line {i}\n")).collect();
    queue.complete(id, body).unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 60, 20);
    assert!(screen.contains("line 199"), "the newest line was not in front:\n{screen}");
}

#[rstest]
fn given_a_long_report_when_the_human_scrolls_back_then_earlier_lines_appear() {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    let body: String = (0..200).map(|i| format!("line {i}\n")).collect();
    queue.complete(id, body).unwrap();
    let mut app = App::new(queue);
    for _ in 0..40 {
        app.scroll_report(-1);
    }
    let screen = screen(&app, 60, 20);
    assert!(!screen.contains("line 199"), "scrolling back did not move the report:\n{screen}");
    assert!(screen.contains("line 1"), "the earlier lines never came into view:\n{screen}");
}

#[rstest]
fn given_a_split_surface_when_the_report_is_expanded_then_it_gets_the_rows() {
    let mut app = app_with(40);
    let split = screen(&app, 60, 20);
    app.panes = Panes::Report;
    let full = screen(&app, 60, 20);
    let count = |s: &str| s.lines().filter(|l| l.contains("task number")).count();
    assert!(count(&full) < count(&split), "the report did not take the queue's rows");
}

#[rstest]
fn given_a_split_surface_when_the_queue_is_expanded_then_more_tasks_are_visible() {
    let mut app = app_with(40);
    let split = screen(&app, 60, 20);
    app.panes = Panes::Queue;
    let full = screen(&app, 60, 20);
    let count = |s: &str| s.lines().filter(|l| l.contains("task number")).count();
    assert!(count(&full) > count(&split), "the queue did not take the report's rows");
}

#[rstest]
fn given_a_split_surface_when_the_pane_key_is_pressed_then_it_cycles_through_the_three() {
    let mut app = app_with(3);
    assert_eq!(app.panes, Panes::Split);
    app.cycle_panes();
    assert_eq!(app.panes, Panes::Report);
    app.cycle_panes();
    assert_eq!(app.panes, Panes::Queue);
    app.cycle_panes();
    assert_eq!(app.panes, Panes::Split);
}

#[rstest]
fn given_the_keys_the_surface_answers_when_the_footer_is_read_then_each_is_named() {
    let app = app_with(2);
    let screen = screen(&app, 130, 22);
    for named in ["z undo", "Tab pane", "Alt+Up/Dn", ": set", "b btw", "n nest", "S save", "M/E/W", "T think"] {
        assert!(screen.contains(named), "a key nobody can find is a key nobody presses: {named}");
    }
}

#[rstest]
fn given_a_focused_task_when_a_part_is_nested_then_it_becomes_its_child() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
    let mut app = app_with(1);
    let parent = app.queue.focus().unwrap();
    app.handle_key(key(KeyCode::Char('n')));
    for c in "inspect the parser".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let node = app.queue.get(parent).unwrap();
    assert_eq!(node.children.len(), 1, "nesting did nothing: {:?}", app.notice);
    assert_eq!(app.queue.next_queued(), Some(node.children[0]), "the part would never run");
}
