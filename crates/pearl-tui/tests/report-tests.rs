//! Stories for reading the work: the report is on the screen, not in a file.
//!
//! The queue is the object of attention while the agent generates, and what it
//! produced is what the human reads while shaping the next task. A badge alone
//! keeps the first half of that promise and drops the second.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{render_to_string, screen, App};
use rstest::rstest;

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

/// A reply of `count` numbered lines, the number padded so that asking whether
/// the screen holds one of them cannot be answered by another: "line 0007" is
/// a line, and "line 7" is a prefix of "line 71".
fn numbered(count: usize) -> String {
    (0..count).map(|i| format!("line {i:04}\n")).collect()
}

/// The numbered report lines the screen shows, in order.
fn lines_on(screen: &str) -> Vec<usize> {
    screen
        .lines()
        .filter_map(|row| row.split("line ").nth(1))
        .filter_map(|rest| rest.get(..4))
        .filter_map(|n| n.parse().ok())
        .collect()
}

/// A queue holding one task that has finished with `reply` as its report.
fn finished(reply: &str) -> Queue {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    queue.complete(id, reply).unwrap();
    queue
}

#[rstest]
fn given_a_finished_task_when_the_surface_is_drawn_then_its_reply_is_on_the_screen() {
    let queue = finished("the parser now accepts a trailing comma");
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("the parser now accepts a trailing comma"),
        "the report never reached the eye:\n{screen}"
    );
}

#[rstest]
fn given_a_report_written_in_chinese_when_the_pane_is_narrow_then_the_end_of_it_is_on_the_screen() {
    // A Chinese character is two cells on the glass, so a line that fits by
    // counting characters is twice as wide as the pane and the tail of it is
    // clipped by the terminal without a mark. The last words are the test
    // because they are what a half-wide line loses.
    let queue = finished("总结一下这个工程的整体结构，说明每个模块的职责，结尾在这里");
    let screen = render_to_string(&queue, 40, 24);
    // The drawing puts a space after a cell a wide character left over, so the
    // words are read from the screen with the spacing taken out.
    let seen: String = screen.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(seen.contains("结尾在这里"), "the end of the report never reached the eye:\n{screen}");
}

#[rstest]
fn given_a_turn_whose_work_is_longer_than_the_window_when_the_report_is_read_then_the_bill_survives() {
    let queue = Queue::new();
    let (id, _) = queue.append("run the tests");
    queue.start(id).unwrap();
    queue.charge(
        id,
        pearl_core::Turn::doing(
            pearl_core::Cost::turn(26_700, 89, 1_300),
            "bash cargo test -p pearl-core --test queue-tests 2>&1 | grep -A12",
        ),
    );
    queue.complete(id, "done").unwrap();
    let screen = render_to_string(&queue, 40, 24);
    let row = screen.lines().find(|line| line.starts_with("turn 1")).expect("no turn line");
    assert!(
        row.contains("↑26.7k ↓89"),
        "the bill of a turn was clipped away with its work: {row:?}"
    );
}

#[rstest]
fn given_a_failed_task_when_the_surface_is_drawn_then_the_reason_is_on_the_screen() {
    let queue = Queue::new();
    let (id, _) = queue.append("call the model");
    queue.start(id).unwrap();
    queue.fail(id, "provider: connection refused").unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("connection refused"),
        "a failure with no reason on screen:\n{screen}"
    );
}

#[rstest]
fn given_a_reply_longer_than_the_pane_when_the_surface_is_drawn_then_its_newest_line_is_visible() {
    let mut reply = String::new();
    for n in 0..40 {
        reply.push_str(&format!("line {n}\n"));
    }
    reply.push_str("the last thing it said");
    let queue = finished(&reply);
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("the last thing it said"),
        "the newest line scrolled out of reach:\n{screen}"
    );
}

#[rstest]
fn given_a_task_that_has_not_run_when_the_surface_is_drawn_then_the_pane_says_so() {
    let queue = Queue::new();
    queue.append("not yet");
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("no report yet"),
        "an empty pane with nothing said:\n{screen}"
    );
}

#[rstest]
fn given_two_finished_tasks_when_the_focus_moves_then_the_pane_shows_the_focused_report() {
    let queue = Queue::new();
    let (first, _) = queue.append("first");
    queue.start(first).unwrap();
    queue.complete(first, "the first answer").unwrap();
    let (second, _) = queue.append("second");
    queue.start(second).unwrap();
    queue.complete(second, "the second answer").unwrap();
    queue.focus_on(second).unwrap();

    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("the second answer"), "{screen}");
    assert!(!screen.contains("the first answer"), "{screen}");
}

#[rstest]
fn given_a_running_task_when_the_surface_is_drawn_then_what_it_is_doing_is_on_the_screen() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.note(id, "edit src/parser.rs").unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("edit src/parser.rs"),
        "a running task showed nothing of its work:\n{screen}"
    );
}

#[rstest]
fn given_a_task_that_finished_after_working_when_drawn_then_the_reply_replaces_the_act() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.note(id, "edit src/parser.rs").unwrap();
    queue.complete(id, "the parser is fixed").unwrap();
    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("the parser is fixed"), "{screen}");
    assert!(!screen.contains("edit src/parser.rs"), "{screen}");
}

// The pane is a window on the document, and a page is the window's own height:
// back is not a distance from the end of the text, it is a place a whole screen
// still fits. Measured as a distance, the window shrinks past the first screen
// until it is empty, and a scroll measured any other way does not land a page.

#[rstest]
fn given_a_report_longer_than_the_pane_when_the_human_pages_back_then_every_page_is_full_and_continues_the_last(
) {
    let app = App::new(finished(&numbered(200)));
    let full = lines_on(&screen(&app, 80, 24)).len();
    assert!(full > 2, "a pane this short cannot show a page:\n{}", screen(&app, 80, 24));
    let mut app = app;
    let mut rows = lines_on(&screen(&app, 80, 24));
    for page in 0..12 {
        app.handle_key(alt(KeyCode::PageUp));
        let drawn = screen(&app, 80, 24);
        let next = lines_on(&drawn);
        assert_eq!(next.len(), full, "page {page} is not a full screen:\n{drawn}");
        if next != rows {
            // Pages join, except the first screen, which is whole and so may
            // show again the few lines the page before it already showed: a
            // page that stopped short of its own height would be the blank
            // space this story is about.
            assert!(
                next.last().unwrap() + 1 == rows[0] || next[0] == 0,
                "page {page} skips lines:\n{drawn}"
            );
        } else {
            // A page that does not move is the first screen holding: the
            // window stops where a whole screen still fits, and stops there
            // rather than shrinking into it.
            assert_eq!(rows[0], 0, "page {page} stayed put before the first screen:\n{drawn}");
        }
        rows = next;
    }
    assert_eq!(rows[0], 0, "twelve pages back never reached the oldest line");
}

#[rstest]
fn given_a_report_longer_than_the_pane_when_the_oldest_line_is_asked_for_then_the_first_screen_shows_and_a_page_forward_follows_it(
) {
    let mut app = App::new(finished(&numbered(200)));
    let full = lines_on(&screen(&app, 80, 24)).len();
    app.handle_key(alt(KeyCode::Home));
    let top = lines_on(&screen(&app, 80, 24));
    assert_eq!(top.first(), Some(&0), "the oldest line is not on the screen");
    assert_eq!(top.len(), full, "the first screen is not full");
    app.handle_key(alt(KeyCode::PageDown));
    let next = lines_on(&screen(&app, 80, 24));
    assert_eq!(
        next.first(),
        Some(&full),
        "a page forward from the oldest line did not follow the first screen"
    );
    assert_eq!(next.len(), full, "the page forward is not full");
}

#[rstest]
fn given_an_answer_written_in_markdown_when_it_is_read_then_the_marks_are_drawn_and_not_printed() {
    // A model answers in markdown, and the report used to hand the syntax to
    // the human: `### 一句话` as prose, a list as dashes in the margin, and a
    // code block as three backticks with a hole under them.
    let queue = finished(
        "# The parser\n\n- reads a trailing comma\n- keeps one entry\n\n```rust\nlet x = 1;\n```\n\nIt is **done** now.\n",
    );
    let screen = render_to_string(&queue, 80, 24);
    assert!(screen.contains("The parser"), "the heading's words never reached the eye:\n{screen}");
    assert!(!screen.contains("# The parser"), "the heading was printed as its mark:\n{screen}");
    assert!(screen.contains("• reads a trailing comma"), "the bullet is not a bullet:\n{screen}");
    assert!(!screen.contains("- reads a trailing comma"), "the bullet was printed as its mark:\n{screen}");
    assert!(screen.contains("let x = 1;"), "the code never reached the eye:\n{screen}");
    assert!(!screen.contains("```"), "a fence was printed as its mark:\n{screen}");
    assert!(screen.contains("rust"), "the code block forgot which language it is:\n{screen}");
    assert!(!screen.contains("**done**"), "the strong mark was printed as its mark:\n{screen}");
}

#[rstest]
fn given_a_code_line_longer_than_the_pane_when_it_is_read_then_it_is_cut_at_the_edge() {
    // Code is not wrapped: a command broken across two rows is not the command
    // that ran. The edge cuts, and the cut says it cut.
    let queue = finished("```sh\ncargo test -p pearl-core --test report-tests -- --nocapture\n```\n");
    let screen = render_to_string(&queue, 40, 24);
    let row = screen.lines().find(|line| line.contains("cargo test")).expect("no code row");
    assert!(row.contains('…'), "a code row ran into the edge with no mark: {row:?}");
    assert!(!screen.contains("--nocapture"), "the tail of a code line was wrapped, not cut:\n{screen}");
}

#[rstest]
fn given_a_heading_when_it_is_read_then_its_words_are_not_the_colour_of_the_prose() {
    use ratatui::style::Color;
    let queue = finished("# The parser\n\nit reads a comma\n");
    let app = App::new(queue);
    let drawn = screen(&app, 80, 24);
    let row = drawn
        .lines()
        .position(|line| line.contains("The parser"))
        .expect("the heading is not on the screen") as u16;
    assert_eq!(
        pearl_tui::row_fg(&app, 80, 24, row),
        Color::White,
        "a heading is drawn in the prose's own colour:\n{drawn}"
    );
}

#[rstest]
fn given_an_answer_with_a_table_when_it_is_read_then_the_columns_line_up() {
    // A table is a shape, and the shape is the whole of what it says: a row
    // whose second cell is not under the header's second cell is not a row.
    let queue = finished("| alpha | beta |\n|---|---|\n| aaaa | bbbb |\n| cc | d |\n");
    let screen = render_to_string(&queue, 60, 24);
    assert!(!screen.contains("|---|"), "the table's dashes were printed as their mark:\n{screen}");
    let head = screen.lines().find(|line| line.contains("alpha")).expect("no header row");
    let row = screen.lines().find(|line| line.contains("aaaa")).expect("no body row");
    assert!(head.contains("beta"), "the header lost a column:\n{screen}");
    assert_eq!(
        head.find("alpha"),
        row.find("aaaa"),
        "the body's first cell is not under the header's:\n{screen}"
    );
    assert_eq!(
        head.find("beta"),
        row.find("bbbb"),
        "the body's second cell is not under the header's:\n{screen}"
    );
}

#[rstest]
fn given_a_table_wider_than_the_pane_when_it_is_read_then_it_is_drawn_as_the_text_it_is() {
    // A grid squeezed by half a column puts a number under the wrong heading,
    // so a table that cannot fit is handed on as the text it is, pipes and all.
    let queue = finished("| a very long name indeed | another long name |\n|---|---|\n| one | two |\n");
    let screen = render_to_string(&queue, 24, 24);
    assert!(
        screen.contains('|'),
        "a table too wide for the pane was squeezed instead of printed:\n{screen}"
    );
}

#[rstest]
fn given_a_right_aligned_column_when_it_is_read_then_its_numbers_end_in_one_column() {
    let queue = finished("| name | size |\n|------|-----:|\n| a | 1 |\n| b | 100 |\n");
    let screen = render_to_string(&queue, 40, 24);
    let one = screen.lines().find(|line| line.trim_start().starts_with("a ")).expect("no row a");
    let many = screen.lines().find(|line| line.trim_start().starts_with("b ")).expect("no row b");
    assert_eq!(
        one.find('1'),
        many.rfind('0'),
        "a right-aligned column did not line its numbers up:\n{screen}"
    );
}
