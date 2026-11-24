//! view --- single-focus flat list: badges for awareness, one cursor for focus.
//!
//! The tree executes in parallel underneath, but the human sees one flat list
//! at the current level. Status is a badge you scan; the cursor is the one spot
//! your attention rests, and the pane below it is what that task reported.

use crate::app::{App, Panes};
use pearl_core::{NodeState, Queue};
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
fn wrap(text: &str, cols: usize) -> Vec<String> {
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
    lines
}

/// Render the queue's current view into a frame: the list above, the report of
/// the focused task below. Reading is not a mode you enter.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let queue = &app.queue;
    // A rule under a title, not a box: the sides of the screen are for text.
    let needed = u16::try_from(queue.view().len()).unwrap_or(u16::MAX).saturating_add(1);
    let half = (area.height / 2).max(2);
    let rows = match app.panes {
        Panes::Split => needed.clamp(2, half),
        Panes::Queue => area.height.saturating_sub(2).max(2),
        Panes::Report => 2,
    };
    let parts = Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).split(area);
    render_list(queue, frame, parts[0]);
    render_report(app, frame, parts[1]);
}

/// A pane's heading: a title and a rule beneath it, spanning the full width.
fn heading(title: String) -> Block<'static> {
    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
        .title_bottom(Line::from(title).style(Style::default().fg(Color::DarkGray)))
}

fn render_report(app: &App, frame: &mut Frame, area: Rect) {
    let queue = &app.queue;
    let node = queue.focus().and_then(|id| queue.get(id));
    let back = app.report_back;
    let thought = node.as_ref().and_then(|n| n.reasoning.clone());
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
    let block = heading(title);
    let inner = block.inner(area);
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
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Each turn's own bill, before the answer it paid for. A task's total tells
    // you what it cost; the turns tell you which round was the expensive one.
    if let Some(n) = &node {
        for (i, turn) in n.turns.iter().enumerate() {
            lines.push(Line::styled(
                format!("turn {}  {}", i + 1, turn.describe()),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if !n.turns.is_empty() {
            lines.push(Line::raw(""));
        }
    }
    if let (Some(t), true) = (&thought, app.show_thinking) {
        lines.push(Line::styled("thinking", Style::default().fg(Color::DarkGray)));
        lines.extend(
            wrap(t, inner.width as usize)
                .into_iter()
                .map(|l| Line::styled(l, Style::default().fg(Color::DarkGray))),
        );
        lines.push(Line::raw(""));
        lines.push(Line::styled("answer", Style::default().fg(Color::DarkGray)));
    }
    let failed = node.is_some_and(|n| n.state == NodeState::Failed);
    let colour = if failed { Color::Red } else { Color::Gray };
    lines.extend(
        wrap(&answer, inner.width as usize)
            .into_iter()
            .map(|l| Line::styled(l, Style::default().fg(colour))),
    );
    let end = lines.len().saturating_sub(back);
    let start = end.saturating_sub(inner.height as usize);
    frame.render_widget(Paragraph::new(lines[start..end].to_vec()).block(block), area);
}

fn render_list(queue: &Queue, frame: &mut Frame, area: Rect) {
    let focus = queue.focus();
    let nodes = queue.view();
    let block = heading(String::new());
    let inner = block.inner(area);
    let rows = inner.height as usize;
    // The cursor is the one place attention rests, so the window follows it:
    // a list that grows past its pane must never hide the task being worked.
    let at = nodes.iter().position(|n| Some(n.id) == focus).unwrap_or(0);
    let start = at.saturating_sub(rows / 2).min(nodes.len().saturating_sub(rows));
    let items: Vec<ListItem> = nodes
        .iter()
        .skip(start)
        .take(rows)
        .map(|node| {
            let cursor = if Some(node.id) == focus { "▶ " } else { "  " };
            let mut spans = vec![
                Span::raw(cursor),
                Span::styled(badge(node.state), Style::default().fg(badge_color(node.state))),
                Span::raw(" "),
                Span::raw(node.title.clone()),
            ];
            if !node.cost.is_empty() {
                spans.push(Span::styled(
                    format!("  {}", node.cost.describe()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
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
    // Where the window sits in the whole, so a long queue is not a mystery.
    let shown = if nodes.is_empty() {
        String::from(" queue ")
    } else {
        format!(" queue {}-{} of {} ", start + 1, (start + rows).min(nodes.len()), nodes.len())
    };
    let shown = match queue.scope() {
        Some(_) => format!("{shown}(narrowed) "),
        None => shown,
    };
    let block = heading(shown);
    frame.render_widget(List::new(items).block(block), area);
}
