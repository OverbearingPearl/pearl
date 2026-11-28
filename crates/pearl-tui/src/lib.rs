//! The flow surface: a single-focus flat view over the task tree, made live.
//!
//! This crate owns presentation and input; the log owns none of it. The
//! queue is the subject. The surface renders one flat list at the current level
//! with a focus cursor and status badges, shows the request in the hand in the
//! footer, and maps keys to queue edits. The rendering is a pure function of
//! the queue, so a test backend can assert on it without a terminal.

mod app;
pub mod clipboard;
pub mod draft;
mod fit;
mod marks;
mod style;
mod view;

pub use app::{
    App, HelpRow, Input, InputKind, Lang, Mode, Panes, Pick, key, offered_keys, steering, wordmark,
};
pub use draft::Placed;
pub use view::{badge, list_lines, list_offset, pane_rows, render, report_lines, report_offset};

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyModifiers,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};

use ratatui::Terminal;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// The address stage's two examples, whole: a remote house and this machine.
/// The scheme and the path are drawn as furniture and the base between them
/// carries weight, because the base is the whole of what the human has to
/// produce -- the rest is what PEARL writes around it.
pub fn endpoint_example_lines(
    examples: &[(&'static str, &'static str, &'static str); 2],
) -> Vec<Line<'static>> {
    examples
        .iter()
        .enumerate()
        .map(|(i, (scheme, base, path))| {
            let which = if i == 0 { "remote" } else { "local" };
            Line::from(vec![
                Span::styled(format!(" {which:<7}"), style::dim()),
                Span::styled((*scheme).to_string(), style::dim()),
                Span::styled((*base).to_string(), style::typed()),
                Span::styled((*path).to_string(), style::dim()),
            ])
        })
        .collect()
}

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

/// The style of the first letter drawn in one row, so a story can say that
/// what the human is typing is not furniture. Weight and dimness are the
/// surface's only vocabulary (`style`), and a terminal's palette is not
/// something a story here can or should know.
pub fn row_style(
    app: &App,
    width: u16,
    height: u16,
    row: u16,
) -> (ratatui::style::Color, ratatui::style::Modifier) {
    let mut terminal = test_terminal(width, height);
    terminal.draw(|frame| draw(app, frame)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    for x in 0..width {
        let cell = &buffer[(x, row)];
        if cell.symbol() != " " {
            return (cell.fg, cell.modifier);
        }
    }
    (
        ratatui::style::Color::Reset,
        ratatui::style::Modifier::empty(),
    )
}

/// Every cell's two colours on a drawn frame, so a story can prove the
/// surface owns no palette: a terminal's colours are the human's, and a fg or
/// bg that is not `Reset` is a guess about a theme we have never seen.
pub fn cell_colours(
    app: &App,
    width: u16,
    height: u16,
) -> Vec<(ratatui::style::Color, ratatui::style::Color)> {
    let mut terminal = test_terminal(width, height);
    terminal.draw(|frame| draw(app, frame)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let mut out = Vec::with_capacity(usize::from(width) * usize::from(height));
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            out.push((cell.fg, cell.bg));
        }
    }
    out
}

/// The text of a drawn frame, one string per row. A wide character is one
/// symbol and one empty cell, which is exactly what the terminal shows.
/// Where the terminal will draw the cursor, or `None` when the surface hides
/// it. The input method anchors its unconfirmed spelling here, which is why a
/// prompt places it and every other screen leaves it hidden.
pub fn caret(app: &App, width: u16, height: u16) -> Option<(u16, u16)> {
    let mut terminal = test_terminal(width, height);
    terminal.draw(|frame| draw(app, frame)).expect("test draw");
    if terminal.backend().cursor_visible() {
        let at = terminal.backend().cursor_position();
        Some((at.x, at.y))
    } else {
        None
    }
}

fn test_terminal(width: u16, height: u16) -> Terminal<ratatui::backend::TestBackend> {
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
    run_with_hooks(
        app,
        |_| Err(String::from("no answerer attached")),
        || Err(String::from("no workspace attached")),
    )
}

/// Run the live surface with an answerer for side questions. When an aside is
/// asking, the loop hands its question to `answer` and settles the aside with
/// the result. The answerer is the caller's bridge to the model; the surface
/// itself stays pure.
pub fn run_with(app: App, answer: impl FnMut(&str) -> Result<String, String>) -> io::Result<()> {
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
    run_with_save(app, answer, undo, || {
        Err(String::from("no config path attached"))
    })
}

/// Run the live surface with all three bridges: an answerer for side
/// questions, an undo for the last write, and a save for the knobs.
pub fn run_with_save(
    mut app: App,
    mut answer: impl FnMut(&str) -> Result<String, String>,
    mut undo: impl FnMut() -> Result<String, String>,
    mut save: impl FnMut() -> Result<String, String>,
) -> io::Result<()> {
    run_with_files(&mut app, &mut answer, &mut undo, &mut save, |_| {
        Err(String::from("no ledger attached"))
    })
}

/// The whole surface, with every bridge it can ask through: an answerer for
/// side questions, an undo, a save, and the ledger of what a task touched.
pub fn run_with_files(
    mut app: &mut App,
    mut answer: impl FnMut(&str) -> Result<String, String>,
    mut undo: impl FnMut() -> Result<String, String>,
    mut save: impl FnMut() -> Result<String, String>,
    mut files: impl FnMut(&str) -> Result<String, String>,
) -> io::Result<()> {
    let nothing = |_: &App| {};
    run_with_usage(
        &mut app,
        &mut answer,
        &mut undo,
        &mut save,
        &mut files,
        nothing,
    )
}

/// The same, with one more thing to say when the surface closes: what the
/// run pressed. Written here because this is the one place that still
/// holds the app after the loop has finished.
pub fn run_with_usage(
    mut app: &mut App,
    mut answer: impl FnMut(&str) -> Result<String, String>,
    mut undo: impl FnMut() -> Result<String, String>,
    mut save: impl FnMut() -> Result<String, String>,
    mut files: impl FnMut(&str) -> Result<String, String>,
    mut done: impl FnMut(&App),
) -> io::Result<()> {
    // Asked before raw mode, because the answer is a round trip to the terminal
    // and crossterm's own detector switches raw mode on and off around it.
    let mut stdout = io::stdout();
    let asked = ask_for_keys(&mut stdout, keys_capable());
    app.keys_capable = asked;
    restore_on_panic(asked);
    enable_raw_mode()?;
    execute!(stdout, EnableBracketedPaste)?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut log = event_log();
    let result = event_loop(
        &mut terminal,
        &mut app,
        &mut log,
        &mut answer,
        &mut undo,
        &mut save,
        &mut files,
    );
    disable_raw_mode()?;
    restore(terminal.backend_mut(), asked);
    terminal.show_cursor()?;
    done(&app);
    result
}

/// Give the terminal back even when a frame panics.
///
/// A flag stack left pushed is a terminal whose next program reads keys it
/// never asked for, and a panic is exactly the way out that skips the code
/// that would have popped it.
fn restore_on_panic(asked: bool) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut out = io::stdout();
        restore(&mut out, asked);
        let _ = disable_raw_mode();
        previous(info);
    }));
}

/// Whether this terminal speaks the enhanced keyboard encoding.
///
/// A legacy terminal sends `Ctrl+v` and `Ctrl+Shift+v` as the same byte, so a
/// key that needs Shift under Control cannot arrive at all; the kitty encoding
/// puts the modifiers in the sequence instead. The answer decides which keys
/// the surface can name, not just which bytes it can read: a legend that names
/// a key the terminal cannot send is a legend that lies.
pub fn keys_capable() -> bool {
    crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false)
}

/// Ask the terminal for the enhanced encoding, if it speaks it.
///
/// The answer is what the surface remembers about the terminal, so a terminal
/// that does not speak it is left exactly as it was found rather than sent a
/// sequence it does not know.
pub fn ask_for_keys(out: &mut impl io::Write, capable: bool) -> bool {
    if !capable {
        return false;
    }
    push_key_flags(out).is_ok()
}

/// Push the enhanced encoding onto the terminal's stack.
pub fn push_key_flags(out: &mut impl io::Write) -> io::Result<()> {
    // The Windows console has no such request, and asking anyway fails the
    // surface on a terminal that was working: there is nothing to push.
    #[cfg(windows)]
    {
        let _ = out;
        return Ok(());
    }
    #[cfg(not(windows))]
    execute!(
        out,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
}

/// Pop the enhanced encoding the surface pushed.
pub fn pop_key_flags(out: &mut impl io::Write) -> io::Result<()> {
    #[cfg(windows)]
    {
        let _ = out;
        return Ok(());
    }
    #[cfg(not(windows))]
    execute!(out, PopKeyboardEnhancementFlags)
}

/// Give the terminal back: the flags that were pushed, paste that was
/// bracketed, and the screen that was borrowed.
///
/// One function for both ways out, because a terminal left holding a flag
/// stack is a terminal whose next program reads keys it never asked for.
pub fn restore(out: &mut impl io::Write, asked: bool) {
    if asked {
        pop_key_flags(out).ok();
    }
    execute!(out, DisableBracketedPaste, LeaveAlternateScreen).ok();
}

/// One frame of the surface: the queue and its report, then a line for each
/// thing the human is owed -- a side answer, a refused key, and the keys.
fn draw(app: &App, frame: &mut ratatui::Frame) {
    draw_screen(app, frame);
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
    if app.read {
        draw_read(app, frame, area);
        return;
    }
    let aside = app.aside_line();
    // Two rows of furniture, one at each edge: the header says what and where,
    // the footer says how. Everything between them is the work.
    // The draft takes the rows it needs before the panes see any: what the
    // human is writing is the thing that cannot be recovered if it is cut.
    let prompt_rows = if app.is_prompting() {
        app.draft_placed(width.saturating_sub(2), app.draft_cap(area.height as usize))
            .map_or(1, |placed| placed.lines.len().max(1))
    } else {
        1
    };
    let mut constraints = vec![Constraint::Length(1), Constraint::Min(2)];
    if aside.is_some() {
        constraints.push(Constraint::Length(1));
    }
    if app.notice.is_some() {
        constraints.push(Constraint::Length(1));
    }
    // The address stage is the one place a human has to produce a URL from
    // memory, so it shows two whole ones and which part of each is theirs.
    if app.endpoint_examples().is_some() {
        constraints.push(Constraint::Length(2));
    }
    constraints.push(Constraint::Length(prompt_rows as u16));
    let chunks = Layout::vertical(constraints).split(area);
    // The path yields from the left, because the tail of a path is what
    // identifies a directory. Only the ground's own item: a context reading
    // also contains a slash and is not a path.
    let money = app.money();
    let mut s = app.settings_items();
    if money.is_some() {
        s.pop();
    }
    let mut place = app.place_items();
    // The money is counted here and kept out of the row's own list: it closes
    // the row whether or not anything else fits, because a night's cost is not
    // something a long directory name gets to hide.
    let money_width = money
        .as_ref()
        .map(|m| m.chars().count() + 3)
        .unwrap_or(0);
    let others: usize = s.iter().map(|i| i.chars().count() + 3).sum::<usize>()
        + place
            .iter()
            .skip(1)
            .map(|i| i.chars().count() + 3)
            .sum::<usize>()
        + money_width
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
    // The settings stand together, and the ground comes after them: on a row
    // with no column to spare, `w/W` is what a person reaches for while the
    // window is on their mind, and the directory under it is read once a day.
    header.extend(s.iter().take(4).cloned());
    header.extend(place);
    header.extend(s.iter().skip(4).cloned());
    // One column of margin before the furniture, and -- counted in `inner` --
    // the last column of every row left empty: a character written into it
    // arms the terminal's pending wrap, and the next thing written, often the
    // input method's composition, takes the wrap and scrolls the screen.
    let inner = width.saturating_sub(2);
    dim(frame, format!(" {}", header_row(&header, money.as_deref(), inner)), chunks[0]);
    render(app, frame, chunks[1]);
    let mut row = 2;
    if let Some(line) = aside {
        let widget = Paragraph::new(line);
        frame.render_widget(widget, chunks[row]);
        row += 1;
    }
    if let Some(line) = &app.notice {
        let widget = Paragraph::new(line.clone()).style(style::strong());
        frame.render_widget(widget, chunks[row]);
        row += 1;
    }
    if let Some(examples) = app.endpoint_examples() {
        for (i, line) in endpoint_example_lines(&examples).into_iter().enumerate() {
            let at = Rect {
                y: chunks[row].y + i as u16,
                height: 1,
                ..chunks[row]
            };
            frame.render_widget(Paragraph::new(line), at);
        }
        row += 1;
    }
    // Help and quit survive every width: a window that hides how to leave is
    // a window the human has to kill. While the human is typing, the bottom row
    // is the prompt and it is furniture no longer: a legend that covers what
    // you are writing is a legend that ate your input, and dimming the words
    // you are choosing is dimming the thing you are looking at.
    if app.is_prompting() {
        match app.draft_placed(inner, prompt_rows) {
            Some(placed) => {
                let focused = matches!(&app.mode, Mode::Input(input) if input.focused);
                for (i, line) in placed.lines.iter().enumerate() {
                    let at = Rect {
                        y: chunks[row].y + i as u16,
                        height: 1,
                        ..chunks[row]
                    };
                    if focused {
                        lit(frame, line.clone(), at);
                    } else {
                        dim(frame, line.clone(), at);
                    }
                }
                if focused {
                    // `caret` is (row within the draft, column of cells): the
                    // row is added to the chunk the draft sits in, the column
                    // is the one the terminal is left in.
                    let (at_row, at_col) = placed.caret;
                    let at_col = at_col.min(width.saturating_sub(1)) as u16;
                    frame.set_cursor_position((
                        chunks[row].x + at_col,
                        chunks[row].y + at_row as u16,
                    ));
                }
            }
            None => {
                let (label, body) = app.prompt_parts();
                let line = format!(" {}", fit::label_keeping(&label, &body, inner));
                let caret = fit::width_of(&line).min(width.saturating_sub(1)) as u16;
                frame.set_cursor_position((caret, chunks[row].y));
                lit(frame, line, chunks[row]);
            }
        }
    } else {
        // The last column of the last row stays empty here too: a character
        // written into it arms the terminal's pending wrap, and the next thing
        // to be written -- an input method's composition, which the surface
        // neither sees nor can hold back -- takes the wrap and scrolls the
        // whole screen by one line.
        dim(
            frame,
            format!(
                " {}",
                fit::fit_keeping(
                    &app.key_items(),
                    inner,
                    " · ",
                    &format!(" · {}", app.tail_items().join(" · ")),
                )
            ),
            chunks[row],
        );
    }
}

/// A row the human is working on, in the colour of the work.
fn lit(frame: &mut ratatui::Frame, text: String, at: Rect) {
    let widget = Paragraph::new(text);
    frame.render_widget(widget, at);
}

/// The wordmark's own columns, which the rest of the header yields to.
/// The header row: the settings and the ground, then the money last.
///
/// The money survives a row that has no room for it, because a night's cost is
/// not something a long directory name gets to hide -- but not at the price of
/// the readings that carry a key. So the money gives up its time before the row
/// gives up the window, and gives up the row before the window's own name does.
/// Which of those happened is read off the drawn text rather than counted in
/// columns beforehand: a row that keeps `ctx` and `plain` is a row that still
/// says everything a hand needs, and one that does not is not.
fn header_row(header: &[String], money: Option<&str>, width: usize) -> String {
    let folded = header
        .iter()
        .find(|item| item.starts_with("folded "))
        .cloned();
    // The count rides with the money: both are what the row is paying for,
    // and a tail kept for the bill is a tail that can keep the count. The
    // footprint is not in that tail -- nobody acts on `mem`.
    let head: Vec<String> = header
        .iter()
        .filter(|item| !item.starts_with("folded "))
        .cloned()
        .collect();
    let tail = |bill: &str| match &folded {
        Some(count) => format!("{bill} | {count}"),
        None => bill.to_string(),
    };
    let Some(money) = money else {
        return match &folded {
            Some(count) => fit::fit_keeping(&head, width, " | ", &format!(" | {count}")),
            None => fit::fit(&head, width, " | "),
        };
    };
    // The two items whose key a person reaches for: the window and the way out
    // of whatever mode they are in. Their own text is the test, so no count of
    // separators can drift from what `fit` actually drew.
    let named = |row: &str| head.len() > 4 && row.contains(&head[3]) && row.contains(&head[4]);
    let with = |tail_text: &str| fit::fit_keeping(&head, width, " | ", tail_text);
    let full = with(&tail(&format!(" | {money}")));
    if named(&full) {
        return full;
    }
    // The time is the first part of a bill a tight row can do without: it is
    // the one figure a person can get back by looking at the clock.
    if let Some((shot, _time)) = money.rsplit_once(' ') {
        let short = with(&tail(&format!(" | {shot}")));
        if named(&short) {
            return short;
        }
    }
    // Neither bill can be named beside the keys, so the count rides alone: it
    // is the only sign that the window gave history up, and it costs a dozen
    // columns where the bill costs thirty. The keys still win if even that will
    // not sit beside them.
    if folded.is_some() {
        let alone = with(&tail(""));
        if named(&alone) {
            return alone;
        }
    }
    fit::fit(&head, width, " | ")
}

fn pearl_tui_wordmark_len() -> usize {
    wordmark().chars().count() + 3
}

/// One dim row of furniture.
fn dim(frame: &mut ratatui::Frame, text: String, at: Rect) {
    let widget = Paragraph::new(text).style(style::dim());
    frame.render_widget(widget, at);
}

/// The models to choose between, on a screen of their own. The row in force
/// is marked, the highlighted row is reversed, and a row whose key cannot be
/// found says so -- choosing it is allowed, being surprised by it is not.
fn draw_picker(app: &App, at: usize, frame: &mut ratatui::Frame, area: Rect) {
    let rows = app.picker_rows();
    let motion = app.list_motion();
    let topic = match app.pick_kind {
        Pick::Model => format!("models  ({motion} move  ⏎ choose  a add  e edit  d drop  ␛ close)"),
        Pick::Effort => format!(
            "effort  ({motion} move  ⏎ choose  ␛ close)  {}",
            app.settings.dialect().describe()
        ),
        Pick::Window => {
            format!("window  ({motion} move  ⏎ choose  ␛ close)  tokens of prompt before a fold")
        }
        // The list is opened from a row, and that row is behind the list: the
        // topic is the only thing that says which row is being sent
        // somewhere new. Adding has no row yet, so the topic says what it
        // will be.
        Pick::Endpoint(row) => match row.and_then(|n| app.settings.candidates().get(n).cloned()) {
            Some(c) => format!("address for {}  ({motion} move  ⏎ choose  ␛ close)", c.label()),
            None => format!("address for a new row  ({motion} move  ⏎ choose  ␛ close)"),
        },
    };
    let mut lines = vec![Line::styled(topic, style::strong())];
    // The mark and the opening cursor are the same answer, so they are read
    // from the same place: a row marked one thing and opened on another makes
    // the first press a jump.
    let at_now = app.picker_start();
    let count = rows.len().to_string().len();
    for (i, row) in rows.into_iter().enumerate() {
        let mark = if i == at_now { "*" } else { " " };
        let n = i + 1;
        let text = format!("{mark} {n:>count$}  {row}");
        lines.push(if i == at {
            Line::styled(text, style::marked())
        } else {
            Line::raw(text)
        });
    }
    // What the last press did. The list owns the screen, so the sentence
    // that says a row was dropped has to be drawn here or nowhere.
    if let Some(note) = &app.notice {
        lines.push(Line::styled(note.clone(), style::strong()));
    }
    let room = area.height as usize;
    if lines.len() > room && room > 1 {
        let hidden = lines.len() - (room - 1);
        lines.truncate(room - 1);
        lines.push(Line::styled(
            format!(" +{hidden} more -- enlarge the window"),
            style::dim(),
        ));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// The request in front of the cursor, wrapped, on a screen of its own. A
/// queue row shortens a title where the glass ends and says so; this is where
/// the rest of that sentence lives, so a cut is a shortening and not a loss.
fn draw_read(app: &App, frame: &mut ratatui::Frame, area: Rect) {
    let width = area.width as usize;
    let title = if app.read_text.is_some() {
        " files"
    } else {
        " request"
    };
    let mut lines = vec![Line::styled(
        format!(
            " {title}  ({}|PgUp|PgDn scroll, any other key closes)",
            app.list_motion()
        ),
        style::strong(),
    )];
    let text = match &app.read_text {
        Some(text) => text.clone(),
        None => app.request_text(),
    };
    let body: Vec<Line> = crate::view::wrap(&text, width.saturating_sub(2))
        .into_iter()
        .map(|line| Line::raw(format!(" {line}")))
        .collect();
    let room = area.height as usize;
    if body.len() + 2 > room && room > 2 {
        let last = body.len().saturating_sub(room - 2);
        let at = app.read_at.min(last);
        lines.extend(body.iter().skip(at).take(room - 2).cloned());
        lines.push(Line::styled(
            format!(" {} of {} lines", at + room - 2, body.len()),
            style::dim(),
        ));
    } else {
        lines.extend(body);
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Everything the rows had to drop, on a screen of its own. A narrow window
/// may hide a key; it may not be the only place that key was ever written.
fn draw_help(app: &App, frame: &mut ratatui::Frame, area: Rect) {
    let lang = app.help_lang;
    let mut lines = vec![Line::styled(
        app.help_lang.header(app.settings.keys()),
        style::strong(),
    )];
    // The same groups in the same order as the row, so help is the row with
    // room rather than a second list to learn.
    for (en, zh, entries) in app.help_groups() {
        let name = match lang {
            Lang::En => en,
            Lang::Zh => zh,
        };
        lines.push(Line::styled(format!(" {name}"), style::strong()));
        for row in entries {
            lines.push(Line::raw(format!("   {:<10} {}", row.keys, row.text(lang))));
        }
    }
    // Everything here is a key with its sentence, so the screen is taller
    // than a terminal. It scrolls, and says where it is, rather than cutting
    // the last groups off without a word. The header already carries the
    // readings and the keys beside them; repeating them here would be a
    // second list to keep in step with the first.
    // What the last press did. The list owns the screen, so the sentence
    // that says a row was dropped has to be drawn here or nowhere.
    if let Some(note) = &app.notice {
        lines.push(Line::styled(note.clone(), style::strong()));
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
            format!(" {}/{}  ⇧⇩ more", at + room - 2, body.len()),
            style::dim(),
        ));
        lines = shown;
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// One line for one event, for the reports that cannot be answered from the
/// code: whether the terminal sends pinyin spellings, a Return per keystroke,
/// or nothing at all is not something the surface can infer. `PEARL_EVENT_LOG`
/// names a file and the loop writes this line for every event it reads.
pub fn describe_event(event: &Event) -> String {
    match event {
        Event::Key(key) => {
            let held: Vec<&str> = [
                (KeyModifiers::CONTROL, "CONTROL"),
                (KeyModifiers::ALT, "ALT"),
                (KeyModifiers::SHIFT, "SHIFT"),
            ]
            .iter()
            .filter(|(bit, _)| key.modifiers.contains(*bit))
            .map(|(_, name)| *name)
            .collect();
            let held = if held.is_empty() {
                String::from("NONE")
            } else {
                held.join("|")
            };
            format!("key {:?} {held}", key.code)
        }
        Event::Paste(text) => format!("paste {} chars", text.chars().count()),
        Event::Mouse(mouse) => format!("mouse {:?} {},{}", mouse.kind, mouse.column, mouse.row),
        Event::Resize(columns, rows) => format!("resize {columns}x{rows}"),
        Event::FocusGained => String::from("focus gained"),
        Event::FocusLost => String::from("focus lost"),
    }
}

/// Open the event log named by `PEARL_EVENT_LOG`, if it is named at all.
fn event_log() -> Option<std::fs::File> {
    let path = std::env::var("PEARL_EVENT_LOG").ok()?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

/// Draw one frame, for the stories that need a real frame rather than text.
pub fn draw_for_test(app: &App, frame: &mut ratatui::Frame) {
    draw(app, frame);
}

/// Park the terminal's cursor at the left of the bottom row, hidden, so the
/// input method's unconfirmed spelling is drawn where it has room.
pub fn park_cursor<B>(terminal: &mut Terminal<B>)
where
    B: ratatui::backend::Backend,
{
    let rows = terminal.size().map(|size| size.height).unwrap_or(1);
    let _ = terminal
        .backend_mut()
        .set_cursor_position(ratatui::layout::Position::new(0, rows.saturating_sub(1)));
    let _ = terminal.hide_cursor();
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    log: &mut Option<std::fs::File>,
    answer: &mut impl FnMut(&str) -> Result<String, String>,
    undo: &mut impl FnMut() -> Result<String, String>,
    save: &mut impl FnMut() -> Result<String, String>,
    files: &mut impl FnMut(&str) -> Result<String, String>,
) -> io::Result<()> {
    let mut last_git = std::time::Instant::now() - Duration::from_secs(3);
    while !app.quit {
        // The kernel's own figure, refreshed each frame: an estimate on a
        // status line invites trust in a number nobody measured.
        app.footprint = pearl_tools::footprint::describe();
        // A page is the window's own height, and the window measures itself
        // as it draws: a second count of the furniture here disagreed with the
        // rows the panes were actually given, so a page was not a screen.
        // The branch is a file read; the dirty count is a subprocess, so it is
        // asked for now and then rather than every frame.
        let ask_git = app.place.is_none() || last_git.elapsed() >= Duration::from_secs(2);
        if ask_git {
            last_git = std::time::Instant::now();
            // Read only when the answer is kept: the branch is a file read and
            // the dirty count is a subprocess, and a frame that throws both
            // away is ten reads a second nobody asked for.
            let place = pearl_tools::place::Place::read(&app.workspace, app.policy, true);
            app.place = Some(place.describe());
        }
        // A task the runner finished has to hand the cursor on before the
        // frame is drawn, or this frame describes work that is over.
        app.follow();
        // A question the model asked is the one event that stops the run and
        // waits for the human: the line opens itself rather than waiting to be
        // found by a key nobody has reason to press yet.
        app.offer_question();
        terminal.draw(|frame| draw(app, frame))?;
        // A hidden cursor still has a position, and a terminal anchors the
        // input method's unconfirmed spelling there. Left where the last cell
        // was written, that is the corner of the last row: the composition
        // starts in the last column, its second character wraps, and the row
        // that wraps is the bottom one, so the alt screen scrolls up under the
        // human's eyes with no keystroke in the event log to blame. Parked at
        // the left of the bottom row there is a whole line's room for it.
        if !app.is_prompting() {
            park_cursor(terminal);
        }
        let wait = 100;
        if event::poll(Duration::from_millis(wait))? {
            let event = event::read()?;
            if let Some(log) = log.as_mut() {
                use std::io::Write;
                writeln!(log, "{}", describe_event(&event)).ok();
            }
            match event {
                Event::Key(key) => app.handle_key(key),
                // A paste is text. Handled as keys it is a Return per line,
                // and one paragraph becomes a task plus a prompt nobody asked
                // for -- the same shape an IME's unconfirmed spelling takes.
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
        }
        // Hand a copied document to the terminal, which owns the clipboard,
        // and say what happened: a copy that did nothing and said nothing is
        // worse than no copy at all.
        if let Some(text) = app.pending_copy.take() {
            let delivery = clipboard::deliver(&text);
            app.notice = Some(delivery.words(text.chars().count()));
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
        // Ask the caller what this task touched, and show its words on the
        // read screen: the ledger lives with the files, not with the surface.
        if let Some(id) = app.files_requested.take() {
            app.read_text = Some(match files(&id.to_string()) {
                Ok(text) => text,
                Err(why) => why,
            });
            app.read = true;
            app.read_at = 0;
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
