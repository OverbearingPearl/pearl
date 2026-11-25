//! view --- single-focus flat list: badges for awareness, one cursor for focus.
//!
//! The tree executes in parallel underneath, but the human sees one flat list
//! at the current level. Status is a badge you scan; the cursor is the one spot
//! your attention rests, and the pane below it is what that task reported.

use crate::app::{App, Panes};
use pearl_core::{NodeState, Queue, Turn};
use unicode_width::UnicodeWidthChar;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

/// A status badge: awareness at a glance, no attention demanded.
pub fn badge(state: NodeState) -> &'static str {
    match state {
        NodeState::Queued => "·",
        NodeState::Running => "▶",
        NodeState::AwaitingInput => "?",
        NodeState::Paused => "‖",
        NodeState::Done => "✓",
        NodeState::Failed => "✗",
        NodeState::Cancelled => "⊘",
    }
}

fn badge_color(state: NodeState) -> Color {
    match state {
        NodeState::Running => Color::Green,
        NodeState::AwaitingInput => Color::Yellow,
        NodeState::Failed => Color::Red,
        NodeState::Done => Color::DarkGray,
        NodeState::Paused => Color::Magenta,
        _ => Color::Gray,
    }
}

/// Hard-wrap `text` at `cols` display columns, keeping every line.
///
/// Columns, not characters: a title written in Chinese draws two cells per
/// character, so a line wrapped by counting characters runs past the glass and
/// is clipped by the terminal without a mark -- the same mistake ADR 21 names
/// for `fit`, and the one that leaves half a wide character and a neighbour's
/// numbers on the frame, because the console's model of the screen stops
/// agreeing with the cells that are actually lit.
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let glass = cols.max(1);
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut width = 0;
        for c in line.chars() {
            let cell = UnicodeWidthChar::width(c).unwrap_or(0);
            if width + cell > glass && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                width = 0;
            }
            current.push(c);
            width += cell;
        }
        lines.push(current);
    }
    lines
}

/// One turn's work and its bill, fitted to the window.
///
/// The work yields first -- `…` says it did -- because the bill is the reason
/// the line is printed at all, and a turn whose note ate the numbers says a
/// round was expensive without saying how.
fn turn_line(number: usize, turn: &Turn, width: usize) -> String {
    let lead = format!("turn {number}  ");
    let bill = turn.cost.describe();
    let room = width.saturating_sub(crate::fit::width_of(&lead) + crate::fit::width_of(&bill) + 2);
    let note = crate::fit::head(&turn.note, room);
    if note.is_empty() {
        format!("{lead}{bill}")
    } else {
        format!("{lead}{note}  {bill}")
    }
}

/// Render the queue's current view into a frame: the list above, the report of
/// the focused task below. Reading is not a mode you enter.
/// How many rows each window gets. One function, because a page key that
/// jumps a different number of rows than the window shows is a page key that
/// lies about what a page is.
pub fn pane_rows(app: &App, height: u16) -> (u16, u16) {
    let needed = u16::try_from(app.queue.view().len()).unwrap_or(u16::MAX).saturating_add(1);
    let half = (height / 2).max(2);
    let rows = match app.panes {
        Panes::Split => needed.clamp(2, half),
        Panes::Queue => height.saturating_sub(2).max(2),
        Panes::Report => 2,
    };
    (rows, height.saturating_sub(rows))
}

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    // A rule under a title, not a box: the sides of the screen are for text.
    // The panes give up the last column, as the header and the row do, because
    // a character written there arms the pending wrap. The left edge keeps its
    // column: a pane's title and its rows already begin one cell in, so the
    // rule's name lines up with the rows it heads.
    let area = Rect { width: area.width.saturating_sub(1), ..area };
    let (rows, _) = pane_rows(app, area.height);
    let parts = Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).split(area);
    render_list(app, frame, parts[0]);
    render_report(app, frame, parts[1]);
}

/// A pane's heading: a title and a rule beneath it, spanning the full width.
fn heading(title: String) -> Block<'static> {
    headed(title, None)
}

/// A heading whose rule also carries the keys that move the window it heads.
///
/// The hint sits at the rule's right end, opposite the name of what the window
/// shows, so the two read as one line: what this is on the left, what moves it
/// on the right. A window too narrow for both keeps the name and drops the
/// hint whole -- a caption cut mid-key teaches half a key, and `?` still has
/// the sentence. The row paid for these keys out of the width its operations
/// needed; the rule has the width anyway, because it is drawn either way.
///
/// Both ends breathe: a space between the name and its rule, two cells between
/// the rule and the hint, and two characters of rule left at the far corner,
/// because a hint that runs into the edge reads as a line that was severed
/// rather than one that ends. The gap is two cells rather than one: a rule's
/// own glyph bleeds into the cell beside it in most monospace fonts, and a
/// single space between `─` and a key is a space nobody can see.
fn headed(title: String, hint: Option<String>) -> Block<'static> {
    let mut block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
        .title_bottom(Line::from(title).style(Style::default().fg(Color::DarkGray)));
    if let Some(hint) = hint {
        block = block.title_bottom(
            Line::from(hint).right_aligned().style(Style::default().fg(Color::DarkGray)),
        );
    }
    block
}

/// The hint, dressed for the rule: padded by a space at each end and closed by
/// a short stretch of rule so the line ends rather than runs out. Kept only if
/// the rule can hold it beside its name with a stretch of rule between them.
/// Whole or nothing: half a hint is a key nobody presses.
fn hint_that_fits(area: Rect, title: &str, hint: String) -> Option<String> {
    let dressed = format!("  {hint} ──");
    let need = crate::fit::width_of(title) + crate::fit::width_of(&dressed) + 3;
    (need <= area.width as usize).then_some(dressed)
}

/// The report as the pane draws it: the whole document, in order, whether or
/// not it fits. The pane shows the window its offset names, and a story can
/// read what is beyond the glass without a terminal to scroll.
pub fn report_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let queue = &app.queue;
    let node = queue.focus().and_then(|id| queue.get(id));
    let thought = node.as_ref().and_then(|n| n.reasoning.clone());
    let answer = match &node {
        Some(n) => n
            .result
            .clone()
            .or_else(|| n.error.clone())
            .or_else(|| n.activity.clone())
            .unwrap_or_else(|| String::from("no report yet")),
        None => String::from("no report yet"),
    };
    // Thinking reads before the answer and dimmer than it: it is how the
    // answer was reached, not the answer, and the eye lands on the answer.
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Each turn's own bill, before the answer it paid for. A task's total tells
    // you what it cost; the turns tell you which round was the expensive one.
    if let Some(n) = &node {
        for (i, turn) in n.turns.iter().enumerate() {
            lines.push(Line::styled(turn_line(i + 1, turn, width), dim));
        }
        if !n.turns.is_empty() {
            lines.push(Line::default());
        }
    }
    if let (Some(t), true) = (&thought, app.show_thinking) {
        lines.push(Line::styled(String::from("thinking"), dim));
        // Thinking is drawn plainly: it is how the answer was reached, and the
        // marks in it are the model thinking aloud rather than a document.
        lines.extend(wrap(t, width).into_iter().map(|l| Line::styled(l, dim)));
        lines.push(Line::default());
        lines.push(Line::styled(String::from("answer"), dim));
    }
    let failed = node.as_ref().is_some_and(|n| n.state == NodeState::Failed);
    let colour = if failed { Color::Red } else { Color::Gray };
    // The answer is drawn through its own marks: a heading, a list and a code
    // block arrive as the model wrote them, and the human reads a document
    // rather than the syntax of one. Every row still comes back no wider than
    // the pane, because a line the terminal clips at a cell it did not choose
    // is how a wide character loses its second half.
    lines.extend(crate::marks::lines(&answer, width, Style::default().fg(colour)));
    lines
}

/// Where the report's window starts, in document rows. One function, because
/// a window that starts anywhere but here is a window the selection and the
/// drawing would disagree about.
pub fn report_offset(app: &App, height: usize, len: usize) -> usize {
    // A window is exactly `height` rows tall and slides over the document, so
    // it can only sit where a whole screen still fits. Past the first screen
    // there is no place further back: clamped as a distance from the end of
    // the text, the window shrank as it went and finished in blank pages.
    let room = len.saturating_sub(height);
    len.saturating_sub(app.report_back.min(room)).saturating_sub(height)
}

fn render_report(app: &App, frame: &mut Frame, area: Rect) {
    let node = app.queue.focus().and_then(|id| app.queue.get(id));
    let thought = node.as_ref().and_then(|n| n.reasoning.clone());
    // The heading's rule is the same whatever the title says, so the width the
    // text wraps at is known before the title is: the count of lines decides
    // how far back "the oldest line" is, and only the drawing can count them.
    let inner = heading(String::from(" report ")).inner(area);
    let lines = report_lines(app, inner.width as usize);
    app.report_len.set(lines.len());
    // A page is the pane's own height, so a page key moves the window by
    // exactly one screenful. Measured here because the drawing is the only
    // place that knows how many rows the pane got.
    let page = (inner.height as usize).max(1);
    app.report_page.set(page);
    let start = report_offset(app, page, lines.len());
    let end = (start + page).min(lines.len());
    let back = lines.len().saturating_sub(end);
    let mut title = String::from(" report ");
    if let Some(t) = &thought {
        title = if app.show_thinking {
            String::from(" report (thinking open) ")
        } else {
            format!(" report (+thinking, {} lines) ", t.lines().count())
        };
    }
    if back > 0 {
        title = format!("{title}(+{back} newer) ");
    }
    let (_, motions) = app.motion_hints();
    let block = headed(title.clone(), hint_that_fits(area, &title, motions));
    let shown: Vec<Line> = lines[start..end].to_vec();
    frame.render_widget(Paragraph::new(shown).block(block), area);
}

/// The queue as the pane draws it: one row per task, in order, whether or not
/// the pane is tall enough. The cursor glyph follows the focus, so a copy of
/// the rows reads the way the screen did.
pub fn list_lines(queue: &Queue, width: usize) -> Vec<String> {
    let focus = queue.focus();
    queue
        .view()
        .iter()
        .map(|node| {
            let cursor = if Some(node.id) == focus { "▶ " } else { "  " };
            // The bill and the child count are readings: a row that drops them
            // to make room for a title has hidden what the task cost. So the
            // title yields first, and yields with a mark that says it did.
            let room = width.saturating_sub(4);
            let mut tail = String::new();
            if !node.children.is_empty() {
                tail.push_str("  ");
                tail.push_str(&format!("[{}]", node.children.len()));
            }
            if !node.cost.is_empty() {
                tail.push_str("  ");
                tail.push_str(&node.cost.describe());
            }
            // A cut row says where the rest of the sentence went. The mark
            // alone admits the loss without offering the remedy, and a key
            // nobody is told about is a key nobody presses.
            let mut hint = "";
            if crate::fit::width_of(&node.title) > room.saturating_sub(crate::fit::width_of(&tail)) {
                const SAID: &str = "  (v shows it all)";
                // Only where a few columns of title survive it: a row that
                // spends its whole width explaining itself has not helped.
                if crate::fit::width_of(&tail) + crate::fit::width_of(SAID) + 8 <= room {
                    hint = SAID;
                }
            }
            let title_room =
                room.saturating_sub(crate::fit::width_of(&tail) + crate::fit::width_of(hint));
            let shown = format!("{}{hint}{tail}", crate::fit::head(&node.title, title_room));
            format!("{cursor}{} {shown}", badge(node.state))
        })
        .collect()
}

/// Where the queue's window starts, in document rows: the cursor's place,
/// held near the middle of the list it sits in.
pub fn list_offset(queue: &Queue, height: usize) -> usize {
    let nodes = queue.view();
    let rows = height;
    let at = nodes
        .iter()
        .position(|n| Some(n.id) == queue.focus())
        .unwrap_or(0);
    at.saturating_sub(rows / 2).min(nodes.len().saturating_sub(rows))
}

fn render_list(app: &App, frame: &mut Frame, area: Rect) {
    let queue = &app.queue;
    let focus = queue.focus();
    let block = heading(String::new());
    let inner = block.inner(area);
    let rows = inner.height as usize;
    // The page is the drawn height here too: a page key that moved by any
    // other number would not land a screen away from where the eye was.
    app.queue_page.set(rows.max(1));
    let lines = list_lines(queue, inner.width as usize);
    let start = list_offset(queue, rows);
    let items: Vec<ListItem> = lines
        .iter()
        .skip(start)
        .take(rows)
        .enumerate()
        .map(|(i, text)| {
            let node = queue.view().get(start + i).cloned();
            let style = match &node {
                Some(n) if Some(n.id) == focus => Style::default().add_modifier(Modifier::BOLD),
                _ => Style::default(),
            };
            // The badge keeps its colour: the row is one string so a copy of it
            // reads like the screen, and colour is drawn back on top of the
            // badge's one cell rather than argued into the string.
            let mut spans = vec![Span::raw(text.clone())];
            if let Some(n) = &node {
                spans = vec![
                    Span::raw(text.chars().take(2).collect::<String>()),
                    Span::styled(
                        text.chars().nth(2).map(String::from).unwrap_or_default(),
                        Style::default().fg(badge_color(n.state)),
                    ),
                    Span::raw(text.chars().skip(3).collect::<String>()),
                ];
            }
            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();
    // Where the window sits in the whole, so a long queue is not a mystery.
    let nodes = queue.view();
    let shown = if nodes.is_empty() {
        String::from(" queue ")
    } else {
        format!(" queue {}-{} of {} ", start + 1, (start + rows).min(lines.len()), lines.len())
    };
    // A level is a place, so the heading names it: "(narrowed)" says that you
    // moved without saying where to, and the way back is a key you must trust.
    let shown = match queue.scope().and_then(|id| queue.get(id)) {
        Some(parent) => format!("{shown}in \"{}\" ", parent.title),
        None => shown,
    };
    let (motions, _) = app.motion_hints();
    let block = headed(shown.clone(), hint_that_fits(area, &shown, motions));
    frame.render_widget(List::new(items).block(block), area);
}
