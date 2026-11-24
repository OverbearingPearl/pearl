//! view --- single-focus flat list: badges for awareness, one cursor for focus.
//!
//! The tree executes in parallel underneath, but the human sees one flat list
//! at the current level. Status is a badge you scan; the cursor is the one spot
//! your attention rests, and the pane below it is what that task reported.

use pearl_core::{NodeState, Queue};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
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

/// The last `rows` display lines of `text`, hard-wrapped at `cols`, so the
/// newest thing the agent said is the thing in front of the eye.
fn tail(text: &str, rows: usize, cols: usize) -> Vec<Line<'static>> {
    let cols = cols.max(1);
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut width = 0;
        for c in line.chars() {
            if width == cols {
                lines.push(std::mem::take(&mut current));
                width = 0;
            }
            current.push(c);
            width += 1;
        }
        lines.push(current);
    }
    let skip = lines.len().saturating_sub(rows);
    lines
        .into_iter()
        .skip(skip)
        .map(Line::from)
        .collect()
}

/// Render the queue's current view into a frame: the list above, the report of
/// the focused task below. Reading is not a mode you enter.
pub fn render(queue: &Queue, frame: &mut Frame, area: Rect) {
    // The list takes the rows its tasks need, up to half the surface; the
    // report takes the rest, because reading is what the space is for.
    let needed = u16::try_from(queue.view().len()).unwrap_or(u16::MAX).saturating_add(2);
    let rows = needed.clamp(3, (area.height / 2).max(3));
    let parts = Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).split(area);
    render_list(queue, frame, parts[0]);
    render_report(queue, frame, parts[1]);
}

fn render_report(queue: &Queue, frame: &mut Frame, area: Rect) {
    let node = queue.focus().and_then(|id| queue.get(id));
    let block = Block::bordered().title(" report ");
    let inner = block.inner(area);
    let body = match &node {
        Some(n) => n
            .result
            .clone()
            .or_else(|| n.error.clone())
            .unwrap_or_else(|| String::from("no report yet")),
        None => String::from("no report yet"),
    };
    let lines = tail(&body, inner.height as usize, inner.width as usize);
    let failed = node.is_some_and(|n| n.state == NodeState::Failed);
    let colour = if failed { Color::Red } else { Color::Gray };
    frame.render_widget(Paragraph::new(lines).style(Style::default().fg(colour)).block(block), area);
}

fn render_list(queue: &Queue, frame: &mut Frame, area: Rect) {
    let focus = queue.focus();
    let items: Vec<ListItem> = queue
        .view()
        .iter()
        .map(|node| {
            let cursor = if Some(node.id) == focus { "▶ " } else { "  " };
            let mut spans = vec![
                Span::raw(cursor),
                Span::styled(badge(node.state), Style::default().fg(badge_color(node.state))),
                Span::raw(" "),
                Span::raw(node.title.clone()),
            ];
            if !node.children.is_empty() {
                spans.push(Span::styled(
                    format!("  [{}]", node.children.len()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            let style = if Some(node.id) == focus {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();
    let title = match queue.scope() {
        Some(_) => " queue (narrowed) ",
        None => " queue ",
    };
    let list = List::new(items).block(Block::bordered().title(title));
    frame.render_widget(list, area);
}
