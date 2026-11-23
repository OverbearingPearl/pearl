//! view --- single-focus flat list: badges for awareness, one cursor for focus.
//!
//! The tree executes in parallel underneath, but the human sees one flat list
//! at the current level. Status is a badge you scan; the cursor is the one spot
//! your attention rests. This is the flow surface, not an editor.

use pearl_core::{NodeState, Queue};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem};
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

/// Render the queue's current view into a frame.
pub fn render(queue: &Queue, frame: &mut Frame, area: Rect) {
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
