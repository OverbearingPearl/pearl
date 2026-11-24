//! select --- the mouse, and the text a drag takes.
//!
//! Left to the terminal, a drag has no idea where the window is: start inside
//! pearl, drag to the top of the screen, and the shell's scrollback comes
//! with it. So pearl claims the left button and selects its own screen, and
//! the terminal keeps Shift+drag for its own selection.
//!
//! The unit is the line. A human dragging over a report is choosing lines they
//! want in a message, not a rectangle of glyphs; the column they released at
//! says nothing about which half of a line they meant.

use std::fmt::Write as _;

/// A drag with the left button held: two corners of the screen, in the order
/// they were visited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub from: (u16, u16),
    pub to: (u16, u16),
}

impl Drag {
    /// The rows the drag covers, first to last, whichever way it was made.
    pub fn rows(&self) -> (usize, usize) {
        let a = self.from.1 as usize;
        let b = self.to.1 as usize;
        (a.min(b), a.max(b))
    }

    /// Whether the pointer moved across a line. A press and release on one
    /// line is a click -- it names a place, not a piece of text -- and a
    /// click that copied an invisible line would be a surprise.
    pub fn covers_a_line(&self) -> bool {
        self.from.1 != self.to.1
    }
}

/// The text a drag took: whole lines, in screen order, with the padding the
/// frame uses to fill the window taken off the end.
pub fn copied(lines: &[String], drag: Drag) -> Option<String> {
    if !drag.covers_a_line() {
        return None;
    }
    let (first, last) = drag.rows();
    let taken: Vec<&str> =
        lines.iter().skip(first).take(last - first + 1).map(|l| l.trim_end()).collect();
    if taken.is_empty() {
        return None;
    }
    Some(taken.join("\n"))
}

/// The escape that hands text to the terminal's clipboard. OSC 52 is the only
/// route out that needs no dependency and no display server, and it is what
/// every terminal with a clipboard understands.
pub fn clipboard_escape(text: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()))
}

/// Base64, the encoding OSC 52 asks for. Six bytes of alphabet, because the
/// one thing worth writing by hand is the thing with no second use.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            let index = (n >> (18 - 6 * i)) & 0x3f;
            let _ = out.write_char(ALPHABET[index as usize] as char);
        }
        // The tail is padded, not truncated: a decoder that reads fewer than
        // a multiple of four characters has to guess.
        for _ in 0..(3 - chunk.len()) {
            out.pop();
            out.push('=');
        }
    }
    out
}
