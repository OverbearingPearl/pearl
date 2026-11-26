//! The three styles PEARL owns, and the reason there are only three.
//!
//! The surface runs in the human's terminal, whose palette PEARL cannot see and
//! must not assume. A colour of our own is a guess about that palette: `Cyan`
//! on a theme that uses cyan for its background is a title the same shade as
//! the page, and a red that reads as a warning on one theme reads as a link on
//! the next. An attribute is not a guess -- the terminal derives dim, bold and
//! reverse from whatever foreground and background it was configured with, so
//! they are legible in a palette we have never seen, including a monochrome
//! one.
//!
//! So: weight for what matters, dimness for furniture, reverse for what the
//! cursor is on. Nothing else. A meaning that cannot be carried by one of these
//! three is carried by a glyph or by words, which is where the badges and the
//! notice line already live.

use ratatui::style::{Modifier, Style};

/// Furniture: rules, hints, what the eye may skip and the hand may not.
pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// Emphasis: a topic, a notice, the thing the eye should land on first.
pub fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// The cursor's own row, in the terminal's own two colours swapped.
pub fn marked() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}
