//! Stories for reading the work: the report is on the screen, not in a file.
//!
//! The queue is the object of attention while the agent generates, and what it
//! produced is what the human reads while shaping the next task. A badge alone
//! keeps the first half of that promise and drops the second.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::Queue;
use pearl_tui::{App, render_to_string, screen};
use ratatui::style::Color;
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
    assert!(
        seen.contains("结尾在这里"),
        "the end of the report never reached the eye:\n{screen}"
    );
}

#[rstest]
fn given_a_turn_whose_work_is_longer_than_the_window_when_the_report_is_read_then_the_bill_survives()
 {
    let queue = Queue::new();
    let (id, _) = queue.append("run the tests");
    queue.start(id).unwrap();
    queue.charge(
        id,
        pearl_core::Bill::doing(
            pearl_core::Cost::round(26_700, 89, 1_300),
            "bash cargo test -p pearl-core --test queue-tests 2>&1 | grep -A12",
        ),
    );
    queue.complete(id, "done").unwrap();
    let screen = render_to_string(&queue, 40, 24);
    let row = screen
        .lines()
        .find(|line| line.trim_start().starts_with("round 1"))
        .expect("no round line");
    assert!(
        row.contains("↑26.7k+0 ↓89"),
        "the bill of a round was clipped away with its work: {row:?}"
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
    // A settled task rests at its answer, so the newest line is asked for by
    // name: the key for the end still has to reach it.
    let mut app = App::new(queue);
    app.handle_key(alt(KeyCode::End));
    let screen = screen(&app, 80, 24);
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
fn given_a_report_longer_than_the_pane_when_the_human_pages_back_then_every_page_is_full_and_continues_the_last()
 {
    let app = App::new(finished(&numbered(200)));
    let full = lines_on(&screen(&app, 80, 24)).len();
    assert!(
        full > 2,
        "a pane this short cannot show a page:\n{}",
        screen(&app, 80, 24)
    );
    let mut app = app;
    // The story is about paging, not about the resting place: start at the end.
    app.handle_key(alt(KeyCode::End));
    let mut rows = lines_on(&screen(&app, 80, 24));
    for page in 0..12 {
        app.handle_key(alt(KeyCode::PageUp));
        let drawn = screen(&app, 80, 24);
        let next = lines_on(&drawn);
        assert_eq!(
            next.len(),
            full,
            "page {page} is not a full screen:\n{drawn}"
        );
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
            assert_eq!(
                rows[0], 0,
                "page {page} stayed put before the first screen:\n{drawn}"
            );
        }
        rows = next;
    }
    assert_eq!(
        rows[0], 0,
        "twelve pages back never reached the oldest line"
    );
}

#[rstest]
fn given_a_report_longer_than_the_pane_when_the_oldest_line_is_asked_for_then_the_first_screen_shows_and_a_page_forward_follows_it()
 {
    let mut app = App::new(finished(&numbered(200)));
    let full = lines_on(&screen(&app, 80, 24)).len();
    app.handle_key(alt(KeyCode::Home));
    let top = lines_on(&screen(&app, 80, 24));
    assert_eq!(
        top.first(),
        Some(&0),
        "the oldest line is not on the screen"
    );
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

/// A round of a running task, with the round's number in its note so a row on
/// the glass can be named: the work a note describes is what a bill is for.
fn round(i: usize) -> pearl_core::Bill {
    pearl_core::Bill::doing(
        pearl_core::Cost::round(1_200, 40, 300),
        format!("bash cargo test --test round{i:04}"),
    )
}

/// The rounds the pane shows, in order. A running task's report is its bills,
/// and the note names the round that paid for it.
fn rounds_on(screen: &str) -> Vec<usize> {
    screen
        .lines()
        .filter_map(|row| row.split_once("round "))
        .filter_map(|(_, rest)| rest.split_once("  "))
        .filter_map(|(n, _)| n.parse().ok())
        .collect()
}

// A page is the reader's, not the work's. The window follows the newest line
// while the newest line is what the eye is on -- and the moment the reader
// walks back, the rows in front of the eye belong to nobody but them. Pinned to
// the end of a document that is still being written, a page back through a
// running task is a page that leaves at the speed of the work.

#[rstest]
fn given_a_running_task_read_back_when_a_round_lands_then_the_page_keeps_the_rows_it_showed() {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    for i in 0..40 {
        queue.charge(id, round(i));
    }
    let mut app = App::new(queue.clone());
    let newest = screen(&app, 80, 24);
    app.handle_key(alt(KeyCode::PageUp));
    let before = rounds_on(&screen(&app, 80, 24));
    assert!(
        before.len() > 2,
        "a pane this short cannot show a page:\n{newest}"
    );
    assert!(
        before.last().copied() < Some(40),
        "the page back already reached the newest round:\n{}",
        screen(&app, 80, 24)
    );
    // The work goes on while the human reads: this is the surface, the agent
    // runs and the human reads what it did.
    queue.charge(id, round(40));
    queue.note(id, "asking the model").ok();
    let drawn = screen(&app, 80, 24);
    let after = rounds_on(&drawn);
    assert_eq!(
        after, before,
        "the page moved out from under the eye when a round landed:\n{drawn}"
    );
    assert!(
        drawn.contains("(+"),
        "the title no longer says how much newer work waits below:\n{drawn}"
    );
}

#[rstest]
fn given_a_running_task_at_the_newest_line_when_a_round_lands_then_the_new_round_is_on_the_screen()
{
    // Holding the page must not cost the tail: at the newest line the window
    // follows the work, because the newest line is what the eye is on.
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    for i in 0..40 {
        queue.charge(id, round(i));
    }
    let app = App::new(queue.clone());
    let drawn = screen(&app, 80, 24);
    assert!(
        rounds_on(&drawn).last().copied() == Some(40),
        "the newest round is not on the screen at rest:\n{drawn}"
    );
    queue.charge(id, round(40));
    let drawn = screen(&app, 80, 24);
    assert!(
        rounds_on(&drawn).last().copied() == Some(41),
        "the window stopped following the work:\n{drawn}"
    );
}

#[rstest]
fn given_a_report_read_back_when_a_motion_has_nowhere_to_go_then_the_page_stays_where_it_is() {
    // One task, one row: the cursor has nowhere to go. A key that moves
    // nothing is not a request for another task's report, and it used to hand
    // the reader back to the newest line of the one they were reading.
    let mut app = App::new(finished(&numbered(200)));
    app.handle_key(alt(KeyCode::PageUp));
    let before = lines_on(&screen(&app, 80, 24));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let after = lines_on(&screen(&app, 80, 24));
    assert_eq!(
        after,
        before,
        "a motion with nowhere to go threw the page away:\n{}",
        screen(&app, 80, 24)
    );
}

#[rstest]
fn given_a_report_read_back_when_the_cursor_reaches_another_task_then_the_page_is_that_tasks_answer()
 {
    // The other half of the same promise: when the cursor really does land on
    // another task, the report is that task's, read from its answer.
    let queue = Queue::new();
    for title in ["write the parser", "write the lexer"] {
        let (id, _) = queue.append(title);
        queue.start(id).unwrap();
        queue.complete(id, &numbered(200)).unwrap();
    }
    let mut app = App::new(queue);
    let _ = screen(&app, 80, 24);
    app.handle_key(alt(KeyCode::PageUp));
    let back = lines_on(&screen(&app, 80, 24));
    assert!(
        back.last().copied() < Some(199),
        "the page did not go back:\n{}",
        screen(&app, 80, 24)
    );
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let next = lines_on(&screen(&app, 80, 24));
    assert_eq!(
        next.first(),
        Some(&0),
        "the next task's report did not open at its answer:\n{}",
        screen(&app, 80, 24)
    );
}

#[rstest]
fn given_a_report_read_back_when_the_level_key_carries_the_cursor_in_then_the_child_opens_at_its_answer()
 {
    // The page belongs to a task, not to the window: a distance kept across a
    // change of task is a distance nobody asked for in the report that arrives.
    let queue = Queue::new();
    let (parent, _) = queue.append("write the parser");
    queue.start(parent).unwrap();
    queue.complete(parent, &numbered(200)).unwrap();
    let (child, _) = queue.add_child(parent, "read the grammar").unwrap();
    queue.start(child).unwrap();
    queue.complete(child, &numbered(120)).unwrap();
    let mut app = App::new(queue);
    let _ = screen(&app, 80, 24);
    app.handle_key(alt(KeyCode::PageUp));
    let page = lines_on(&screen(&app, 80, 24));
    assert!(
        page.last().copied() < Some(199),
        "the page did not go back:\n{}",
        screen(&app, 80, 24)
    );
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let inside = screen(&app, 80, 24);
    let rows = lines_on(&inside);
    assert_eq!(
        rows.first(),
        Some(&0),
        "the task the level key opened did not start at its answer:\n{inside}"
    );
}

#[rstest]
fn given_a_running_task_read_back_when_it_settles_and_the_cursor_moves_then_the_next_task_opens_at_its_newest()
 {
    // A task that settles under the cursor hands the cursor on without a
    // keystroke. The reader was in the one that finished; the report below is
    // now the next task's, and it opens where a reader starts: the newest line.
    let queue = Queue::new();
    let (first, _) = queue.append("write the parser");
    let (second, _) = queue.append("write the lexer");
    queue.start(first).unwrap();
    queue.start(second).unwrap();
    for i in 0..40 {
        queue.charge(first, round(i));
        queue.charge(second, round(i));
    }
    let mut app = App::new(queue.clone());
    // A frame as the loop draws it: the settle below is the second one, and
    // what the surface saw a moment ago is what tells it the task finished.
    app.follow();
    let _ = screen(&app, 80, 24);
    app.handle_key(alt(KeyCode::PageUp));
    let page = rounds_on(&screen(&app, 80, 24));
    assert!(
        page.last().copied() < Some(40),
        "the page did not go back:\n{}",
        screen(&app, 80, 24)
    );
    queue.complete(first, &numbered(20)).unwrap();
    app.follow();
    let drawn = screen(&app, 80, 24);
    assert_eq!(
        queue.focus(),
        Some(second),
        "the cursor was not carried on:\n{drawn}"
    );
    let rows = rounds_on(&drawn);
    assert_eq!(
        rows.last().copied(),
        Some(40),
        "the task the cursor was carried to did not open at its newest round:\n{drawn}"
    );
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
    assert!(
        screen.contains("The parser"),
        "the topic's words never reached the eye:\n{screen}"
    );
    assert!(
        !screen.contains("# The parser"),
        "the topic was printed as its mark:\n{screen}"
    );
    assert!(
        screen.contains("• reads a trailing comma"),
        "the bullet is not a bullet:\n{screen}"
    );
    assert!(
        !screen.contains("- reads a trailing comma"),
        "the bullet was printed as its mark:\n{screen}"
    );
    assert!(
        screen.contains("let x = 1;"),
        "the code never reached the eye:\n{screen}"
    );
    assert!(
        !screen.contains("```"),
        "a fence was printed as its mark:\n{screen}"
    );
    assert!(
        screen.contains("rust"),
        "the code block forgot which language it is:\n{screen}"
    );
    assert!(
        !screen.contains("**done**"),
        "the strong mark was printed as its mark:\n{screen}"
    );
}

#[rstest]
fn given_a_code_line_longer_than_the_pane_when_it_is_read_then_it_is_cut_at_the_edge() {
    // Code is not wrapped: a command broken across two rows is not the command
    // that ran. The edge cuts, and the cut says it cut.
    let queue =
        finished("```sh\ncargo test -p pearl-core --test report-tests -- --nocapture\n```\n");
    let screen = render_to_string(&queue, 40, 24);
    let row = screen
        .lines()
        .find(|line| line.contains("cargo test"))
        .expect("no code row");
    assert!(
        row.contains('…'),
        "a code row ran into the edge with no mark: {row:?}"
    );
    assert!(
        !screen.contains("--nocapture"),
        "the tail of a code line was wrapped, not cut:\n{screen}"
    );
}

#[rstest]
fn given_a_topic_when_it_is_read_then_it_is_the_answers_ink_made_bold() {
    // `Color::White` is the palette's brightest white, and on a theme whose
    // brightest white is cream it lands as grey -- a topic dimmer than the
    // body it heads. Weight, not a colour, is what makes it a topic.
    use ratatui::style::Modifier;
    let queue = finished("# The parser\n\nit reads a comma\n");
    let app = App::new(queue);
    let lines = pearl_tui::report_lines(&app, 80);
    let span = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .find(|span| span.content.contains("The parser"))
        .expect("the topic's words were not drawn");
    assert_eq!(
        span.style.fg, None,
        "a topic was painted in a colour of its own: {:?}",
        span.style.fg
    );
    assert!(
        span.style.add_modifier.contains(Modifier::BOLD),
        "a topic carries no weight: {:?}",
        span.style.add_modifier
    );
}

#[rstest]
fn given_an_answer_with_a_table_when_it_is_read_then_the_columns_line_up() {
    // A table is a shape, and the shape is the whole of what it says: a row
    // whose second cell is not under the header's second cell is not a row.
    let queue = finished("| alpha | beta |\n|---|---|\n| aaaa | bbbb |\n| cc | d |\n");
    let screen = render_to_string(&queue, 60, 24);
    assert!(
        !screen.contains("|---|"),
        "the table's dashes were printed as their mark:\n{screen}"
    );
    let head = screen
        .lines()
        .find(|line| line.contains("alpha"))
        .expect("no header row");
    let row = screen
        .lines()
        .find(|line| line.contains("aaaa"))
        .expect("no body row");
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
    // A grid squeezed by half a column puts a number under the wrong topic,
    // so a table that cannot fit is handed on as the text it is, pipes and all.
    let queue =
        finished("| a very long name indeed | another long name |\n|---|---|\n| one | two |\n");
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
    let one = screen
        .lines()
        .find(|line| line.trim_start().starts_with("a "))
        .expect("no row a");
    let many = screen
        .lines()
        .find(|line| line.trim_start().starts_with("b "))
        .expect("no row b");
    assert_eq!(
        one.find('1'),
        many.rfind('0'),
        "a right-aligned column did not line its numbers up:\n{screen}"
    );
}

#[rstest]
fn given_a_quote_that_holds_a_list_when_it_is_read_then_the_list_is_read_inside_the_quote() {
    // A quote is a column of the page, not a way of writing a dash: the line
    // inside it is a list item, and it is drawn as one.
    let queue = finished("> - alpha\n> - beta\n");
    let screen = render_to_string(&queue, 60, 24);
    let row = screen
        .lines()
        .find(|line| line.contains("alpha"))
        .expect("no quoted item");
    assert!(
        row.contains("• alpha"),
        "a quoted list drew its dash as text: {row:?}"
    );
    assert!(
        row.trim_start().starts_with('│'),
        "the quote lost its edge: {row:?}"
    );
}

#[rstest]
fn given_a_quote_that_holds_a_code_block_when_it_is_read_then_the_fences_are_gone() {
    // Read one line at a time, a quoted fence opens and closes inside a single
    // row and the code is drawn as prose. The block is read as a block.
    let queue = finished("> ```sh\n> cargo run --release\n> ```\n");
    let screen = render_to_string(&queue, 60, 24);
    assert!(
        screen.contains("cargo run --release"),
        "the quoted code never reached the eye:\n{screen}"
    );
    assert!(
        !screen.contains("```"),
        "a quoted fence was printed as its mark:\n{screen}"
    );
    // The quote's edge inside the code's edge: two edges means the block was
    // read as a block, and one means its lines were read one at a time.
    assert!(
        screen
            .lines()
            .any(|line| line.trim_start().starts_with("│ │ sh")),
        "the fence inside the quote was never opened:\n{screen}"
    );
}

#[rstest]
fn given_a_list_inside_a_list_when_it_is_read_then_the_inner_one_is_indented() {
    let queue = finished("- alpha\n  - beta\n");
    let screen = render_to_string(&queue, 60, 24);
    let outer = screen
        .lines()
        .find(|line| line.contains("• alpha"))
        .expect("no outer item");
    let inner = screen
        .lines()
        .find(|line| line.contains("• beta"))
        .expect("no inner item");
    assert!(
        inner.find('•').unwrap() > outer.find('•').unwrap(),
        "a nested list sits where its parent sits:\n{screen}"
    );
}

#[rstest]
fn given_a_task_the_sandbox_refused_when_the_report_is_read_then_the_line_is_on_the_screen() {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    queue
        .notice(id, "sandbox: a write was refused; writes land under ~/proj")
        .unwrap();
    let screen = render_to_string(&queue, 100, 24);
    assert!(
        screen.contains("sandbox: a write was refused"),
        "the report kept the fact to itself:\n{screen}"
    );
}

#[rstest]
fn given_a_task_whose_history_was_folded_when_the_report_is_read_then_it_says_so() {
    let queue = Queue::new();
    let (id, _) = queue.append("write the parser");
    queue.start(id).unwrap();
    queue
        .notice(id, "history: folded 12 older entries to fit the window")
        .unwrap();
    let screen = render_to_string(&queue, 100, 24);
    assert!(
        screen.contains("history: folded 12 older entries"),
        "the fold left no trace on the screen:\n{screen}"
    );
}

#[rstest]
fn given_a_finished_answer_when_the_report_is_read_then_it_is_the_terminals_own_ink() {
    // The answer is what the human came to read. Drawing it in `Gray` made it
    // dimmer than the chrome around it, on a dark terminal it was the least
    // legible thing on the screen.
    let queue = finished("the answer, in plain words");
    let app = App::new(queue.clone());
    let lines = pearl_tui::report_lines(&app, 60);
    let answer: Vec<_> = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .filter(|span| span.content.contains("plain words"))
        .collect();
    assert!(!answer.is_empty(), "the answer was not drawn at all");
    for span in answer {
        assert_eq!(
            span.style.fg, None,
            "the answer was drawn in a colour of its own: {:?}",
            span.style.fg
        );
    }
    assert!(
        !lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.style.fg == Some(Color::Gray))),
        "the report still paints something in the answer's old grey"
    );
}
