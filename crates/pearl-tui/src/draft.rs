//! draft --- where a draft is placed on the glass.
//!
//! The words, the caret and the editing keys live in `pearl-text`,
//! `pearl-emacs` and `pearl-vim`; what is left here is the part that is the
//! surface's own: how a buffer becomes rows, where the label goes, which row
//! is windowed into view, and where the caret lands among them.
//!
//! Positions are counted in characters, never bytes (see `pearl-text`); this
//! module converts the caret's character column into the cells a terminal
//! draws, which is why it needs `unicode-width`.

use crate::fit;
use pearl_text::Buffer;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A draft placed on the glass: the lines to draw, the label already on the
/// first of them, and where the caret landed among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub lines: Vec<String>,
    pub caret: (usize, usize),
}

/// Lay a buffer out in `width` cells: wrapped at the glass, windowed so the
/// caret is always one of the rows, with the label kept on the first visible
/// row -- a prompt that hides which act is being typed has to be left to be
/// identified -- and the hint pushed to the right end of the last row when it
/// fits at all. A hint that does not fit is dropped entire: a half hint is a
/// word the human will read wrong.
pub fn place(
    buffer: &Buffer,
    label: &str,
    hint: Option<&str>,
    width: usize,
    max_rows: usize,
) -> Placed {
    let label_width = UnicodeWidthStr::width(label);
    let width = width.max(1);
    // The first row of the text carries the label, so it has that much less
    // room; every row after it keeps the one column of margin the surface
    // gives its furniture, and no more -- a hanging indent as wide as the
    // label would spend the glass on white.
    let first_room = width.saturating_sub(label_width).max(1);
    let row_room = width.saturating_sub(1).max(1);
    let chars: Vec<char> = buffer.text().chars().collect();
    let mut rows: Vec<(String, usize)> = vec![(String::new(), 0)];
    let mut caret_row = 0;
    let mut caret_used = 0;
    for (i, &c) in chars.iter().enumerate() {
        if i == buffer.caret() {
            caret_row = rows.len() - 1;
            caret_used = rows.last().expect("a row is always there").1;
        }
        if c == '\n' {
            rows.push((String::new(), 0));
            continue;
        }
        let cell = UnicodeWidthChar::width(c).unwrap_or(0);
        let room = if rows.len() == 1 { first_room } else { row_room };
        if rows.last().expect("a row is always there").1 + cell > room {
            rows.push((String::new(), 0));
        }
        let row = rows.last_mut().expect("a row is always there");
        row.0.push(c);
        row.1 += cell;
    }
    if buffer.caret() >= chars.len() {
        caret_row = rows.len() - 1;
        caret_used = rows.last().expect("a row is always there").1;
    }

    let max_rows = max_rows.max(1);
    let start = (caret_row + 1).saturating_sub(max_rows).min(rows.len() - 1);
    let end = (start + max_rows).min(rows.len());
    let mut caret = (caret_row - start, caret_used);
    let mut lines: Vec<String> = rows[start..end]
        .iter()
        .enumerate()
        .map(|(i, (t, _))| if i == 0 { t.clone() } else { format!(" {t}") })
        .collect();
    // A label that the text does not know about takes its cells out of the
    // first visible row, and the row is cut where the glass ends.
    let kept = fit::head(&lines[0], width.saturating_sub(label_width));
    lines[0] = format!("{label}{kept}");
    if caret.0 == 0 {
        caret.1 = (caret.1 + label_width).min(width.saturating_sub(1));
    }

    if let Some(hint) = hint {
        let hint_width = UnicodeWidthStr::width(hint);
        let last = lines.len() - 1;
        let taken = UnicodeWidthStr::width(lines[last].as_str());
        if taken + 1 + hint_width <= width {
            let pad = width - taken - hint_width;
            lines[last].push_str(&" ".repeat(pad));
            lines[last].push_str(hint);
            if caret == (last, taken) {
                caret.1 = caret.1.min(taken);
            }
        }
    }
    Placed { lines, caret }
}
