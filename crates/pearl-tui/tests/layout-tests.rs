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
fn given_a_split_surface_when_a_layout_key_is_pressed_then_that_window_is_taken() {
    let mut app = app_with(3);
    assert_eq!(app.panes, Panes::Split);
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.panes, Panes::Report);
    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.panes, Panes::Queue);
    app.handle_key(key(KeyCode::Char('3')));
    assert_eq!(app.panes, Panes::Split);
}

#[rstest]
fn given_the_keys_the_surface_answers_when_help_is_opened_then_each_is_named() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = screen(&app, 60, 40);
    for named in ["z undo", "Alt+↑↓", ": set", "b btw", "n nest", "S save", "M/E/W", "T think", "1/2/3 queue/report/both"] {
        assert!(screen.contains(named), "a key nobody can find is a key nobody presses: {named}");
    }
}

#[rstest]
fn given_any_width_when_the_action_row_is_read_then_help_and_quit_survive() {
    let app = app_with(2);
    for width in [30u16, 40, 60, 100, 140] {
        let screen = screen(&app, width, 20);
        assert!(screen.contains("? keys"), "at {width} there was no way to find the rest");
        assert!(screen.contains("q quit"), "at {width} there was no way to leave");
    }
}

#[rstest]
fn given_a_narrow_window_when_a_row_is_read_then_it_drops_whole_items_and_says_so() {
    let mut app = app_with(2);
    app.footprint = Some(String::from("16M"));
    let screen = screen(&app, 46, 20);
    assert!(screen.contains('…'), "a shortened row claimed to be whole:\n{screen}");
    assert!(!screen.contains("mem 1"), "half a reading was shown:\n{screen}");
    assert!(!screen.contains("| ctx"), "a label without its value was shown:\n{screen}");
}

#[rstest]
fn given_a_short_window_when_the_surface_is_drawn_then_the_answer_keeps_its_rows() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.complete(id, "the answer").unwrap();
    let mut app = App::new(queue);
    app.place = Some(String::from("~/proj  sandbox:workspace-write  git:main"));
    let screen = screen(&app, 50, 7);
    assert!(screen.contains("the answer"), "the furniture ate the answer:\n{screen}");
}

#[rstest]
fn given_help_is_open_when_any_key_is_pressed_then_it_closes() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    assert!(app.help);
    app.handle_key(key(KeyCode::Char('j')));
    assert!(!app.help, "help swallowed the next key as well");
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

#[rstest]
fn given_a_narrow_queue_when_a_row_is_read_then_the_bill_is_dropped_whole() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1234, 340, 2400));
    let app = App::new(queue);
    let screen = screen(&app, 30, 12);
    let row = screen.lines().find(|l| l.contains("fix the parser")).unwrap_or_default();
    assert!(!row.contains("↓340 2"), "half a bill was on the row: {row}");
    assert!(row.contains("fix the parser"), "the title lost to the bill: {row}");
}

#[rstest]
fn given_a_narrowed_queue_when_the_heading_is_read_then_it_names_where_you_are() {
    let queue = Queue::new();
    let (parent, _) = queue.append("fix the parser");
    let _ = queue.add_child(parent, "read the grammar").unwrap();
    let _ = queue.add_child(parent, "write the test").unwrap();
    queue.narrow(parent).unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 90, 20);
    let head = screen.lines().find(|l| l.contains("queue ")).unwrap_or_default();
    assert!(head.contains("fix the parser"), "the heading did not say whose parts these are: {head}");
    assert!(head.contains('2'), "the heading did not say how many parts: {head}");
}

#[rstest]
fn given_a_narrowed_queue_when_the_level_key_is_pressed_then_it_comes_back_out() {
    let queue = Queue::new();
    let (parent, _) = queue.append("fix the parser");
    let _ = queue.add_child(parent, "read the grammar").unwrap();
    queue.narrow(parent).unwrap();
    let mut app = App::new(queue);
    app.handle_key(key(KeyCode::Left));
    assert!(app.queue.scope().is_none(), "the level key did not come back out");
}

#[rstest]
fn given_a_plain_legend_when_it_is_read_then_it_offers_no_vim_letters() {
    let app = app_with(2);
    let legend = app.motion_line();
    assert!(!legend.contains("j/k"), "vim keys in a plain legend: {legend}");
    assert!(legend.contains("PgUp/Dn"), "plain paging is not the page keys: {legend}");
}

#[rstest]
fn given_one_key_row_when_it_is_read_then_adding_moving_and_reading_are_all_named() {
    let app = app_with(2);
    let screen = screen(&app, 116, 12);
    let row = screen.lines().last().unwrap_or_default();
    for named in ["a add", "queue ", "report ", "1/2/3 queue/report/both"] {
        assert!(row.contains(named), "the row spent itself before {named}: {row}");
    }
}
