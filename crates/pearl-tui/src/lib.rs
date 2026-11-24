//! The flow surface: a single-focus flat view over the task tree, made live.
//!
//! This crate owns presentation and input; the session owns none of it. The
//! queue is the subject. The surface renders one flat list at the current level
//! with a focus cursor and status badges, shows the request in the hand in the
//! footer, and turns keys into queue edits. The rendering is a pure function of
//! the queue, so a test backend can assert on it without a terminal.

mod app;
mod fit;
mod view;

pub mod select;
pub use select::{clipboard_escape, copied, Drag};
pub use app::{
    key, steering, wordmark, App, HelpRow, Input, InputKind, Lang, Mode, Panes, Pick,
};
pub use view::{badge, pane_rows, render};

use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

/// Render the queue into a string at a fixed size, for tests and snapshots.
pub fn render_to_string(queue: &pearl_core::Queue, width: u16, height: u16) -> String {
    let app = App::new(queue.clone());
    let mut terminal = test_terminal(width, height);
    terminal
        .draw(|frame| render(&app, frame, frame.area()))
        .expect("draw");
    buffer_text(&terminal, width, height)
}

/// The whole surface as the human sees it -- queue, report, notice, prompt --
/// so a story can assert what reached the eye and not only what the data holds.
pub fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = test_terminal(width, height);
    terminal.draw(|frame| draw(app, frame)).expect("draw");
    buffer_text(&terminal, width, height)
}

/// The text of a drawn frame, one string per row. A wide character is one
/// symbol and one empty cell, which is exactly what the terminal shows.
fn frame_text(frame: &mut ratatui::Frame) -> Vec<String> {
    let buffer = frame.buffer_mut();
    let area = buffer.area;
    (0..area.height)
        .map(|y| (0..area.width).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect()
}

fn test_terminal(
    width: u16,
    height: u16,
) -> Terminal<ratatui::backend::TestBackend> {
    Terminal::new(ratatui::backend::TestBackend::new(width, height)).expect("test backend")
}

fn buffer_text(
    terminal: &Terminal<ratatui::backend::TestBackend>,
    width: u16,
    height: u16,
) -> String {
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// Run the live surface until the human quits. Owns the terminal lifecycle:
/// raw mode and the alternate screen on entry, restored on exit even on error.
pub fn run(app: App) -> io::Result<()> {
    run_with_hooks(app, |_| Err(String::from("no answerer attached")), || {
        Err(String::from("no workspace attached"))
    })
}

/// Run the live surface with an answerer for side questions. When an aside is
/// asking, the loop hands its question to `answer` and settles the aside with
/// the result. The answerer is the caller's bridge to the model; the surface
/// itself stays pure.
pub fn run_with(
    app: App,
    answer: impl FnMut(&str) -> Result<String, String>,
) -> io::Result<()> {
    run_with_hooks(app, answer, || Err(String::from("no workspace attached")))
}

/// Run the live surface with both bridges: an answerer for side questions and
/// an undo for the last write. The surface asks; the caller, who owns the model
/// and the workspace, acts and reports back what happened.
pub fn run_with_hooks(
    app: App,
    answer: impl FnMut(&str) -> Result<String, String>,
    undo: impl FnMut() -> Result<String, String>,
) -> io::Result<()> {
    run_with_save(app, answer, undo, || Err(String::from("no config path attached")))
}

/// Run the live surface with all three bridges: an answerer for side
/// questions, an undo for the last write, and a save for the knobs.
pub fn run_with_save(
    mut app: App,
    mut answer: impl FnMut(&str) -> Result<String, String>,
    mut undo: impl FnMut() -> Result<String, String>,
    mut save: impl FnMut() -> Result<String, String>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // The mouse is taken from the terminal here, and for one reason: a drag
    // the terminal handles cannot see where this window is, so it runs on into
    // whatever the screen held before pearl started. Shift+drag is still the
    // terminal's own selection, which is how a human copies a line out of the
    // scrollback after quitting.
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, &mut app, &mut answer, &mut undo, &mut save);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    result
}

/// One frame of the surface: the queue and its report, then a line for each
/// thing the human is owed -- a side answer, a refused key, and the keys.
fn draw(app: &App, frame: &mut ratatui::Frame) {
    draw_screen(app, frame);
    mark(app, frame);
}

/// The mark the mouse is making: the lines a drag has covered, drawn in
/// reverse so the human sees what a release will take.
fn mark(app: &App, frame: &mut ratatui::Frame) {
    let Some(drag) = app.drag else { return };
    if !drag.covers_a_line() {
        return;
    }
    let (first, last) = drag.rows();
    let area = frame.area();
    let top = (first as u16).min(area.bottom().saturating_sub(1));
    let bottom = (last as u16).min(area.bottom().saturating_sub(1));
    if bottom < top {
        return;
    }
    let rows = Rect::new(area.left(), top, area.width, bottom - top + 1);
    frame.buffer_mut().set_style(rows, Style::default().add_modifier(Modifier::REVERSED));
}

fn draw_screen(app: &App, frame: &mut ratatui::Frame) {
    let area = frame.area();
    let width = area.width as usize;
    if let Some(at) = app.picker {
        draw_picker(app, at, frame, area);
        return;
    }
    if app.help {
        draw_help(app, frame, area);
        return;
    }
    let aside = app.aside_line();
    // Two rows of furniture, one at each edge: the header says what and where,
    // the footer says how. Everything between them is the work.
    let mut constraints = vec![Constraint::Length(1), Constraint::Min(2)];
    if aside.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if app.notice.is_some() {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    let chunks = Layout::vertical(constraints).split(area);
    // The path yields from the left, because the tail of a path is what
    // identifies a directory. Only the ground's own item: a context reading
    // also contains a slash and is not a path.
    let s = app.settings_items();
    let mut place = app.place_items();
    let others: usize = s.iter().map(|i| i.chars().count() + 3).sum::<usize>()
        + place.iter().skip(1).map(|i| i.chars().count() + 3).sum::<usize>()
        + pearl_tui_wordmark_len();
    if let Some(path) = place.first_mut() {
        let room = width.saturating_sub(others).max(10);
        // Keep the policy's word: `write:` is what makes the directory
        // a boundary rather than just a location.
        if let Some(rest) = path.strip_prefix("write:").map(String::from) {
            let short = fit::tail_path(&rest, room.saturating_sub(6));
            *path = format!("write:{short}");
        } else {
            *path = fit::tail_path(path, room);
        }
    }
    let mut header = vec![wordmark().to_string()];
    header.extend(s.iter().take(3).cloned());
    header.extend(place);
    header.extend(s.iter().skip(3).cloned());
    dim(frame, fit::fit(&header, width, " | "), chunks[0]);
    render(app, frame, chunks[1]);
    let mut row = 2;
    if let Some(line) = aside {
        let widget = Paragraph::new(line).style(Style::default().fg(Color::Cyan));
        frame.render_widget(widget, chunks[row]);
        row += 1;
    }
    if let Some(line) = &app.notice {
        let widget = Paragraph::new(line.clone()).style(Style::default().fg(Color::Yellow));
        frame.render_widget(widget, chunks[row]);
        row += 1;
    }
    // Help and quit survive every width: a window that hides how to leave is
    // a window the human has to kill.
    // While the human is typing, the bottom row is the prompt. A legend that
    // covers what you are writing is a legend that ate your input.
    let bottom = if app.is_prompting() {
        app.prompt_line()
    } else {
        fit::fit_keeping(&app.key_items(), width, "  ", "  ? keys  q quit")
    };
    dim(frame, bottom, chunks[row]);
}

/// The wordmark's own columns, which the rest of the header yields to.
fn pearl_tui_wordmark_len() -> usize {
    wordmark().chars().count() + 3
}

/// One dim row of furniture.
fn dim(frame: &mut ratatui::Frame, text: String, at: Rect) {
    let widget = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(widget, at);
}

/// The models to choose between, on a screen of their own. The row in force
/// is marked, the highlighted row is reversed, and a row whose key cannot be
/// found says so -- choosing it is allowed, being surprised by it is not.
fn draw_picker(app: &App, at: usize, frame: &mut ratatui::Frame, area: Rect) {
    let rows = app.picker_rows();
    let heading = match app.pick_kind {
        Pick::Model => String::from("models  (RET choose  a add  e edit  d drop  Esc close)"),
        Pick::Effort => format!("effort  (RET choose  Esc close)  {}", app.settings.dialect().describe()),
        Pick::Window => String::from("window  (RET choose  Esc close)  tokens of prompt before a fold"),
        Pick::Endpoint(_) => String::from(
            "endpoint  (RET choose  Esc close)  where this row's requests are sent",
        ),
    };
    let mut lines = vec![Line::styled(heading, Style::default().fg(Color::Yellow))];
    let chosen = app.settings.chosen();
    let at_now = match app.pick_kind {
        Pick::Model => rows
            .iter()
            .position(|r| r.starts_with(&chosen.model) || r.contains(&chosen.model))
            .unwrap_or(0),
        Pick::Effort => {
            let word = app.settings.effort().map(|e| e.as_str()).unwrap_or("auto");
            rows.iter().position(|r| r == word).unwrap_or(0)
        }
        Pick::Window => {
            let now = app.settings.window();
            pearl_core::Settings::windows().iter().position(|w| *w == now).unwrap_or(0)
        }
        Pick::Endpoint(row) => {
            let now = row
                .and_then(|n| app.settings.candidates().get(n).map(|c| c.endpoint.clone()))
                .unwrap_or_default();
            rows.iter()
                .position(|r| *r == if now.is_empty() { String::from("-  the launch endpoint") } else { now.clone() })
                .unwrap_or(0)
        }
    };
    for (i, row) in rows.into_iter().enumerate() {
        let mark = if i == at_now { "*" } else { " " };
        let text = format!("{mark} {} {row}", i + 1);
        lines.push(if i == at {
            Line::styled(text, Style::default().bg(Color::DarkGray).fg(Color::White))
        } else {
            Line::raw(text)
        });
    }
    // What the last press did. The list owns the screen, so the sentence
    // that says a row was dropped has to be drawn here or nowhere.
    if let Some(note) = &app.notice {
        lines.push(Line::styled(note.clone(), Style::default().fg(Color::Yellow)));
    }
    let room = area.height as usize;
    if lines.len() > room && room > 1 {
        let hidden = lines.len() - (room - 1);
        lines.truncate(room - 1);
        lines.push(Line::styled(
            format!(" +{hidden} more -- enlarge the window"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Everything the rows had to drop, on a screen of its own. A narrow window
/// may hide a key; it may not be the only place that key was ever written.
fn draw_help(app: &App, frame: &mut ratatui::Frame, area: Rect) {
    let lang = app.help_lang;
    let mut lines = vec![Line::styled(app.help_lang.header(), Style::default().fg(Color::Yellow))];
    // The same groups in the same order as the row, so help is the row with
    // room rather than a second list to learn.
    for (en, zh, entries) in app.help_groups() {
        let name = match lang {
            Lang::En => en,
            Lang::Zh => zh,
        };
        lines.push(Line::styled(format!(" {name}"), Style::default().fg(Color::Cyan)));
        for row in entries {
            lines.push(Line::raw(format!("   {:<10} {}", row.keys, row.text(lang))));
        }
    }
    lines.push(Line::styled(
        format!(" {}", lang.readings()),
        Style::default().fg(Color::Cyan),
    ));
    for item in app.settings_items().into_iter().chain(app.place_items()) {
        lines.push(Line::raw(format!("   {item}")));
    }
    lines.push(Line::styled("   ? keys  q quit", Style::default().fg(Color::DarkGray)));
    // Every key carries its sentence, so the screen is taller than a
    // terminal. It scrolls, and says where it is, rather than cutting the
    // last groups off without a word.
    // What the last press did. The list owns the screen, so the sentence
    // that says a row was dropped has to be drawn here or nowhere.
    if let Some(note) = &app.notice {
        lines.push(Line::styled(note.clone(), Style::default().fg(Color::Yellow)));
    }
    let room = area.height as usize;
    if lines.len() > room && room > 1 {
        let head = lines.remove(0);
        let body = lines;
        let last = body.len().saturating_sub(room - 2);
        let at = app.help_at.min(last);
        let mut shown = vec![head];
        shown.extend(body.iter().skip(at).take(room - 2).cloned());
        shown.push(Line::styled(
            format!(" {}/{}  ↑↓ more", at + room - 2, body.len()),
            Style::default().fg(Color::DarkGray),
        ));
        lines = shown;
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    answer: &mut impl FnMut(&str) -> Result<String, String>,
    undo: &mut impl FnMut() -> Result<String, String>,
    save: &mut impl FnMut() -> Result<String, String>,
) -> io::Result<()> {
    let mut last_git = std::time::Instant::now() - Duration::from_secs(3);
    while !app.quit {
        // The kernel's own figure, refreshed each frame: an estimate on a
        // status line invites trust in a number nobody measured.
        app.footprint = pearl_tools::footprint::describe();
        // A page is the window's own height, so the key and the pane agree.
        let size = terminal.size()?;
        let spare = size.height.saturating_sub(4);
        let furniture = 1 + u16::from(spare >= 3) + u16::from(spare >= 4)
            + u16::from(app.place.is_some() && spare >= 5);
        let (q, r) = pane_rows(app, size.height.saturating_sub(furniture));
        app.queue_page = q.saturating_sub(1).max(1) as usize;
        app.report_page = r.saturating_sub(1).max(1) as usize;
        // The branch is a file read; the dirty count is a subprocess, so it is
        // asked for now and then rather than every frame.
        let ask_git = last_git.elapsed() >= Duration::from_secs(2);
        if ask_git {
            last_git = std::time::Instant::now();
        }
        let place = pearl_tools::place::Place::read(&app.workspace, app.policy, ask_git);
        if ask_git || app.place.is_none() {
            app.place = Some(place.describe());
        }
        // The frame's own text is what a drag takes, so it is read out at the
        // moment it is drawn rather than guessed at from the queue.
        let mut lines: Vec<String> = Vec::new();
        terminal.draw(|frame| {
            draw(app, frame);
            lines = frame_text(frame);
        })?;
        if app.copy_pending {
            app.copy_pending = false;
            if let Some(drag) = app.drag {
                if !drag.covers_a_line() {
                    // A click names a place; it takes nothing.
                    app.drag = None;
                } else if let Some(text) = copied(&lines, drag) {
                    let taken = text.lines().count();
                    let out = terminal.backend_mut();
                    write!(out, "{}", clipboard_escape(&text))?;
                    out.flush()?;
                    app.notice = Some(format!(
                        "copied {taken} line{}",
                        if taken == 1 { "" } else { "s" }
                    ));
                }
            }
        }
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse.kind, mouse.column, mouse.row),
                _ => {}
            }
        }
        // Keep the knobs through the caller, who owns the config file.
        if app.save_requested {
            let outcome = save();
            app.settle_save(outcome);
        }
        // Take the last write back through the caller, who owns the files.
        if app.undo_requested {
            let outcome = undo();
            app.settle_undo(outcome);
        }
        // Settle an asking aside through the answerer; the surface shows it.
        if let Some(aside) = &mut app.aside {
            if aside.state == pearl_core::AsideState::Asking {
                let question = aside.question.clone();
                match answer(&question) {
                    Ok(text) => {
                        let _ = aside.answer(text);
                    }
                    Err(why) => {
                        let _ = aside.fail(why);
                    }
                }
            }
        }
    }
    Ok(())
}
