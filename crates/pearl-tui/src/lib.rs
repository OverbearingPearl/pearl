//! pearl-tui --- the flow surface over the queue.
//!
//! A single-focus flat view: the tree runs in parallel underneath, the human
//! shapes one list at the current level. Built on ratatui and crossterm so the
//! rendering is differential and the terminal does not flicker.

pub mod view;

pub use view::{badge, render};

use pearl_core::Queue;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Render the queue to a string of the given size, for tests and snapshots.
pub fn render_to_string(queue: &Queue, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal
        .draw(|frame| render(queue, frame, frame.area()))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
