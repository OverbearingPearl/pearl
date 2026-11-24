//! Stories for where the screen's space goes.
//!
//! Two panes in boxes spend four columns and four rows on furniture, and a
//! list that grows past its pane simply loses its tail -- including the task
//! the cursor is on. The space belongs to what the human is looking at.

use pearl_core::Queue;
use crossterm::event::KeyCode;
use pearl_tui::{key, screen, App, Panes};
use rstest::rstest;

/// The rendered text without whitespace: a wide character takes two cells and
/// leaves the second blank, which is invisible but breaks every substring.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

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
    // Wide enough that a long sentence is not wrapped through the middle of
    // the key it names.
    let screen = whole_help(&mut app, 140, 40);
    for named in [
        "undo the last file change",
        "↑ / ↓ one task",
        "Alt+↑/↓ one line",
        "name a setting directly",
        "beside the work",
        "split the focused task",
        "save these settings",
        "reasoning",
        "1 / 2 / 3",
        "(m/M)",
        "(e/E)",
        "(w/W)",
    ] {
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
fn given_a_narrow_queue_when_a_row_is_read_then_the_bill_stays_and_the_title_admits_the_cut() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1234, 340, 2400));
    let app = App::new(queue);
    let screen = screen(&app, 30, 12);
    let row = screen.lines().find(|l| l.contains('▶')).unwrap_or_default();
    // The bill is the reading; the title is the thing with a rest of it, and a
    // rest the human can ask for again is worth less than what the task cost.
    assert!(row.contains("↑1.2k"), "the bill was dropped from a narrow row: {row}");
    assert!(row.contains('…'), "the title was cut without saying so: {row}");
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
    // 117 rather than 116: the row keeps its last column empty so a composition
    // has somewhere to land that is not past the edge.
    let app = app_with(2);
    let screen = screen(&app, 117, 12);
    let row = screen.lines().last().unwrap_or_default();
    for named in ["a add", "queue ", "report ", "1/2/3 queue/report/both"] {
        assert!(row.contains(named), "the row spent itself before {named}: {row}");
    }
}

#[rstest]
fn given_an_open_prompt_when_the_bottom_row_is_read_then_it_is_the_prompt_not_the_legend() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('a')));
    let screen = screen(&app, 70, 20);
    assert!(screen.contains("append:"), "the legend ate the input:\n{screen}");
    assert!(!screen.contains("? keys"), "the legend is still competing with the prompt");
}

/// Every line the key screen can show, gathered by scrolling it. Help is
/// taller than a terminal now that each key carries its sentence, so a story
/// about what it names has to read all of it rather than the first page.
fn whole_help(app: &mut App, w: u16, h: u16) -> String {
    let mut all = String::new();
    let mut last = usize::MAX;
    while app.help_at != last {
        last = app.help_at;
        all.push_str(&screen(app, w, h));
        app.handle_key(key(KeyCode::PageDown));
    }
    all.push_str(&screen(app, w, h));
    all
}

#[rstest]
fn given_the_look_group_when_help_is_read_then_a_style_costs_three_lines() {
    let app = app_with(2);
    let look = app
        .help_groups()
        .into_iter()
        .find(|(en, _, _)| *en == "look")
        .map(|(_, _, rows)| rows)
        .expect("help has no look group");
    // Three facts, three lines: the queue, the report, the level. A list per
    // motion kind said "page" twice and spent the screen on headings.
    assert_eq!(look.len(), 3, "the look group is {} lines, not three", look.len());
}

#[rstest]
fn given_help_in_english_when_tab_is_pressed_then_the_screen_speaks_chinese() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let en = screen(&app, 100, 40);
    assert!(en.contains("Tab Eng/中"), "the header does not name the toggle:\n{en}");
    app.handle_key(key(KeyCode::Tab));
    // A wide character occupies two cells and the buffer keeps the second
    // empty, so the rendered text reads with gaps; the words are what matter.
    let zh = squash(&screen(&app, 100, 60));
    assert!(app.help, "Tab closed the screen instead of translating it");
    assert!(zh.contains("按键"), "the header stayed English:\n{zh}");
    assert!(zh.contains("队列"), "the rows stayed English:\n{zh}");
    assert!(!zh.contains("add a task to the end"), "an English sentence survived the switch:\n{zh}");
    app.handle_key(key(KeyCode::Tab));
    assert!(screen(&app, 100, 40).contains("add a task to the end"), "Tab did not translate back");
}

#[rstest]
fn given_the_help_table_when_it_is_read_then_no_row_exists_in_one_language_only() {
    let app = app_with(2);
    let cjk = |s: &str| s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    for (en, zh, rows) in app.help_groups() {
        assert!(!en.is_empty() && !zh.is_empty(), "a group has no name in one language");
        assert!(cjk(zh), "the group {en} is not translated: {zh}");
        for row in rows {
            assert!(
                !row.en.is_empty() && !row.zh.is_empty(),
                "{} is not said in both languages",
                row.keys
            );
            // A Chinese row that is the English row with the letters left in
            // is a row nobody wrote; the reader is getting a caption, not an
            // explanation.
            assert!(cjk(&row.zh), "{} is not translated: {}", row.keys, row.zh);
            assert_ne!(row.en, row.zh, "{} says the same thing twice", row.keys);
        }
    }
}

#[rstest]
fn given_help_when_it_is_read_then_the_keys_are_grouped_in_the_rows_order() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = whole_help(&mut app, 60, 40);
    for group in ["start", "look", "work", "safety", "ask", "set", "readings"] {
        assert!(screen.contains(group), "help is an unlabelled pile: missing {group}");
    }
    let row = app.key_items();
    let start = screen.find("add a task to the end").unwrap_or(usize::MAX);
    let look = screen.find("↑ / ↓ one task").unwrap_or(0);
    let work = screen.find("run a task right now").unwrap_or(0);
    assert!(start < look && look < work, "help orders the keys differently from the row");
    assert_eq!(row.first().map(String::as_str), Some("a add"));
}

#[rstest]
fn given_the_header_names_the_knobs_when_the_key_row_is_read_then_they_are_not_repeated() {
    let app = app_with(2);
    assert!(
        !app.key_items().iter().any(|i| i.contains("M/E/W")),
        "the row repeats what the header already says beside each reading"
    );
}

#[rstest]
fn given_a_window_too_short_for_help_when_it_is_opened_then_it_says_how_much_it_hides() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = screen(&app, 46, 12);
    assert!(screen.contains("↑↓ more"), "help cut groups off silently:\n{screen}");
    // And the rest is reachable: a screen that hides work it will not give
    // back is worse than one that never offered it.
    let rest = whole_help(&mut app, 46, 12);
    assert!(rest.contains("readings"), "the hidden groups cannot be scrolled to:\n{rest}");
}

#[rstest]
fn given_the_help_screen_when_it_is_read_then_the_main_legends_tail_is_not_drawn_on_it() {
    // The help screen has a header that says a key closes it and a footer that
    // says which slice of the table is showing. `? keys  q quit` is the main
    // screen's tail, and carried here it is a third voice saying the second
    // one's sentence on a line the table needed.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    // Tall enough that the whole table is on screen: on a short window the
    // table scrolls and the tail is cut off, which would pass this story
    // without the line ever being absent.
    let drawn = screen(&app, 100, 60);
    assert!(drawn.contains("keys"), "the help screen did not open:\n{drawn}");
    assert!(
        !drawn.contains("? keys  q quit"),
        "the main screen's legend is in help:\n{drawn}"
    );
}
