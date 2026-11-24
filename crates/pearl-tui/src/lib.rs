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

pub use app::{key, steering, App, Input, InputKind, Mode, Panes};
pub use view::{badge, render};

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, &mut app, &mut answer, &mut undo, &mut save);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// One frame of the surface: the queue and its report, then a line for each
/// thing the human is owed -- a side answer, a refused key, and the keys.
fn draw(app: &App, frame: &mut ratatui::Frame) {
    let area = frame.area();
    let width = area.width as usize;
    if app.help {
        draw_help(app, frame, area);
        return;
    }
    let aside = app.aside_line();
    // Furniture yields before content does. A window too short for the ground
    // row loses the ground row, not the answer the human is waiting for.
    let spare = area.height.saturating_sub(4);
    let show_place = app.place.is_some() && spare >= 5;
    let show_status = spare >= 3;
    let show_motion = spare >= 4;
    let mut constraints = vec![Constraint::Min(2)];
    if aside.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if app.notice.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if show_place {
        constraints.push(Constraint::Length(1));
    }
    if show_status {
        constraints.push(Constraint::Length(1));
    }
    if show_motion {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    let chunks = Layout::vertical(constraints).split(area);
    render(app, frame, chunks[0]);
    let mut row = 1;
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
    let mut dim = |text: String, at: usize| {
        let widget = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(widget, chunks[at]);
    };
    if show_place {
        let mut items = app.place_items();
        // The tail of a path is what identifies it; a prefix of ~/Projects
        // identifies nothing. Give the path what the other items leave.
        let rest: usize = items.iter().skip(1).map(|i| i.chars().count() + 2).sum();
        if let Some(first) = items.first_mut() {
            *first = fit::tail_path(first, width.saturating_sub(rest).max(8));
        }
        dim(fit::fit(&items, width, "  "), row);
        row += 1;
    }
    if show_status {
        dim(fit::fit(&app.settings_items(), width, " | "), row);
        row += 1;
    }
    if show_motion {
        dim(fit::fit(&app.motion_items(), width, "  "), row);
        row += 1;
    }
    // Help and quit survive every width: a window that hides how to leave is
    // a window the human has to kill.
    dim(fit::fit_keeping(&app.action_items(), width, "  ", "  ? keys  q quit"), row);
}

/// Everything the rows had to drop, on a screen of its own. A narrow window
/// may hide a key; it may not be the only place that key was ever written.
fn draw_help(app: &App, frame: &mut ratatui::Frame, area: Rect) {
    let mut lines = vec![Line::styled("keys  (any key closes)", Style::default().fg(Color::Yellow))];
    for group in [app.motion_items(), app.action_items(), app.settings_items(), app.place_items()] {
        for item in group {
            lines.push(Line::raw(format!("  {item}")));
        }
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled("  ? keys  q quit", Style::default().fg(Color::DarkGray)));
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
        terminal.draw(|frame| draw(app, frame))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
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
