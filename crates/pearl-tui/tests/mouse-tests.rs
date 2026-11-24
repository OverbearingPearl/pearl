//! Stories for the mouse: a drag inside pearl's own window.
//!
//! The terminal offers two kinds of selection, and only one of them can see
//! where the window is. With the mouse left to the terminal, a drag that
//! starts inside pearl and ends at the top of the screen takes the shell's
//! scrollback with it -- the build log, the last command, the frame before
//! the window opened. So pearl claims the mouse and does its own selection,
//! and the terminal keeps Shift+drag for its own.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use pearl_core::Queue;
use pearl_core::Settings;
use ratatui::layout::Rect;
use pearl_tui::{clipboard_escape, copied, App, Drag, Pane};
use rstest::rstest;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn frame() -> Vec<String> {
    vec![
        String::from(" header "),
        String::from(" queue ───── "),
        String::from(" first task      "),
        String::from(" second task     "),
        String::from(" footer          "),
    ]
}

fn drag(from_row: usize, to_row: usize) -> Drag {
    Drag::start(Pane::Queue, from_row, 0).over(Pane::Queue, to_row)
}

/// A screen of a known size, with the panes drawn where the draw puts them.
fn screen(app: &mut App) {
    app.area.set(Rect::new(0, 1, 80, 22));
}

#[rstest]
fn given_a_drag_down_the_window_when_the_button_is_released_then_the_lines_it_covered_are_taken() {
    let text = copied(&frame(), drag(2, 3)).expect("a two-line drag copied nothing");
    assert_eq!(text, " first task\n second task", "the drag took the wrong lines");
}

#[rstest]
fn given_a_drag_made_upwards_when_the_button_is_released_then_the_same_lines_are_taken() {
    // A drag is two corners, not a direction: taking the pointer back over a
    // line it already crossed is how a human changes their mind.
    let down = copied(&frame(), drag(2, 3)).expect("the downward drag copied nothing");
    let up = copied(&frame(), drag(3, 2)).expect("the upward drag copied nothing");
    assert_eq!(down, up);
}

#[rstest]
fn given_a_click_that_never_moved_when_the_button_is_released_then_nothing_is_taken() {
    assert_eq!(copied(&frame(), drag(2, 2)), None);
}

#[rstest]
fn given_a_frame_padded_to_the_window_width_when_a_line_is_taken_then_the_padding_is_not() {
    // Every line is written to the full width, so a copy that kept the
    // padding would paste a wall of spaces after every task.
    let text = copied(&frame(), drag(2, 3)).expect("nothing was taken");
    assert_eq!(text.lines().next(), Some(" first task"), "the line kept its padding: {text:?}");
}

/// An app with two tasks whose report is `lines` long, drawn on a known
/// screen: the list takes the top of it and the report the rest.
fn long_app(lines: usize) -> App {
    let queue = Queue::new();
    let (first, _) = queue.append("task one");
    queue.append("task two");
    let report: String = (0..lines).map(|i| format!("line {i}\n")).collect();
    queue.start(first).expect("the task never started");
    queue.complete(first, report).expect("the task was never completed");
    queue.focus_on(first).expect("the report cannot be seen");
    let mut app = App::new(queue).with_settings(Settings::new("deepseek-chat"));
    app.area.set(Rect::new(0, 1, 80, 22));
    app
}

fn report_doc(app: &App) -> Vec<String> {
    pearl_tui::report_lines(app, 80).into_iter().map(|(l, _)| l).collect()
}

#[rstest]
fn given_the_button_held_down_when_it_is_released_then_the_lines_can_be_taken_from_the_app() {
    let mut app = long_app(40);
    // The report starts below the list: two task rows and a rule.
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 5);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 9, 7);
    let drag = app.drag.expect("the drag was forgotten while the button was down");
    app.handle_mouse(MouseEventKind::Up(MouseButton::Left), 9, 7);
    let doc = report_doc(&app);
    let text = copied(&doc, drag).expect("the release copied nothing");
    assert_eq!(text.lines().count(), 3, "the drag took the wrong number of lines: {text:?}");
    assert_eq!(copied(&doc, app.drag.unwrap()), copied(&doc, drag), "the mark moved on release");
}

#[rstest]
fn given_lines_taken_when_the_next_key_is_pressed_then_the_mark_is_gone() {
    let mut app = long_app(40);
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 5);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 9, 7);
    app.handle_mouse(MouseEventKind::Up(MouseButton::Left), 9, 7);
    assert!(app.drag.is_some(), "there is no mark to clear");
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.drag, None, "the mark outlived the next key");
}

#[rstest]
fn given_a_report_taller_than_its_pane_when_the_pointer_is_held_past_the_top_edge_then_the_take_reaches_off_screen() {
    // The reason the selection lives in document rows: the window moves under
    // the hand, and a selection counted in screen rows would slide with it.
    let mut app = long_app(60);
    let height = 22 - 3 - 1; // the report's inner height on this screen
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 20);
    let anchor = app.drag.expect("no drag started").anchor;
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 3, 2);
    assert_eq!(app.auto, Some(-1), "the pointer past the top edge was not noticed");
    for _ in 0..height {
        app.auto_scroll();
    }
    let drag = app.drag.expect("the drag was lost");
    assert!(drag.cursor < anchor - height, "the window never scrolled: {} -> {}", anchor, drag.cursor);
    let text = copied(&report_doc(&app), drag).expect("the release copied nothing");
    assert!(text.lines().count() > height, "the take is not longer than the pane: {} lines", text.lines().count());
}

#[rstest]
fn given_a_drag_held_past_the_report_edge_when_the_window_scrolls_then_the_queue_stays_where_it_was() {
    let mut app = long_app(60);
    let focus = app.queue.focus();
    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 20);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 3, 2);
    for _ in 0..5 {
        app.auto_scroll();
    }
    assert_eq!(app.queue_shift, 0, "the queue moved for a drag in the report");
    assert_eq!(app.queue.focus(), focus, "the queue's cursor moved for a drag in the report");
    assert!(app.report_back > 0, "the report did not scroll at all");
}

#[rstest]
fn given_a_queue_longer_than_its_pane_when_the_drag_passes_the_bottom_then_only_the_queue_window_moves() {
    // Twenty tasks on a screen whose list can show two rows: the window has to
    // travel for the selection to reach the next task.
    let queue = Queue::new();
    for i in 0..20 {
        queue.append(format!("task {i}"));
    }
    let mut app = App::new(queue).with_settings(Settings::new("deepseek-chat"));
    app.area.set(Rect::new(0, 1, 80, 22));

    app.handle_mouse(MouseEventKind::Down(MouseButton::Left), 3, 1);
    app.handle_mouse(MouseEventKind::Drag(MouseButton::Left), 3, 30);
    assert_eq!(app.auto, Some(1), "the pointer past the bottom edge was not noticed");
    let before = app.drag.expect("no drag started").cursor;
    app.auto_scroll();
    assert!(app.queue_shift > 0, "the queue's window never moved");
    assert_eq!(app.report_back, 0, "the report scrolled for a drag in the queue");
    let after = app.drag.expect("the drag was lost").cursor;
    assert!(after > before, "the loose corner did not follow the window: {before} -> {after}");
}

#[rstest]
fn given_text_when_it_is_handed_to_the_terminal_clipboard_then_the_escape_carries_the_bytes() {
    // OSC 52 is what every terminal with a clipboard understands, and it is
    // the only route out that needs no dependency and no display server.
    assert_eq!(clipboard_escape("hello"), "\u{1b}]52;c;aGVsbG8=\u{7}");
    assert_eq!(clipboard_escape("轮"), "\u{1b}]52;c;6L2u\u{7}");
    assert_eq!(clipboard_escape(""), "\u{1b}]52;c;\u{7}");
}

// --- the clipboard has two routes, and Apple's terminal answers only one ---

use std::os::unix::fs::PermissionsExt;

fn helper_script(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\ncat > \"$0.out\"\n").expect("write the helper");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[rstest]
fn given_a_program_that_reads_its_standard_input_when_the_selection_is_handed_to_it_then_it_holds_the_lines() {
    let dir = std::env::temp_dir().join(format!("pearl-copy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = helper_script(&dir, "fakecopy");
    let taken = text();
    let carried = pearl_tui::hand_to(pearl_tui::Helper { program: path.to_str().expect("path"), args: &[] }, &taken);
    assert!(carried, "the helper was never handed the text");
    let kept = std::fs::read_to_string(path.with_extension("out")).expect("the helper wrote nothing");
    assert_eq!(kept, taken);
    let _ = std::fs::remove_dir_all(&dir);
}

#[rstest]
fn given_the_routes_pearl_tries_when_they_are_read_then_every_platform_clipboard_is_there() {
    let programs: Vec<&str> = pearl_tui::routes().iter().map(|r| r.program).collect();
    assert_eq!(programs, vec!["pbcopy", "wl-copy", "xclip", "clip.exe"], "a platform lost its route");
}

#[rstest]
fn given_a_path_where_one_route_exists_when_the_helper_is_chosen_then_that_one_is_chosen() {
    // The routes are all in the table; only the one this machine has is used.
    let dir = std::env::temp_dir().join(format!("pearl-route-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    helper_script(&dir, "xclip");
    let found = pearl_tui::helper_on(dir.to_str().expect("path"));
    assert_eq!(found.map(|h| h.program), Some("xclip"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[rstest]
fn given_lines_a_helper_carried_when_the_remark_is_written_then_it_says_they_were_taken() {
    assert_eq!(pearl_tui::copy_remark(3, true), "copied 3 lines");
    assert_eq!(pearl_tui::copy_remark(1, true), "copied 1 line");
}

#[rstest]
fn given_lines_only_the_terminal_was_asked_for_when_the_remark_is_written_then_it_says_so() {
    // Saying "copied" when nothing holds the text is the one lie a copy can
    // tell, and the human only finds out by pasting.
    let remark = pearl_tui::copy_remark(2, false);
    assert!(remark.contains("Shift+drag"), "the remark does not say how to get the text: {remark}");
}

fn text() -> String {
    String::from(" first task\n second task")
}
