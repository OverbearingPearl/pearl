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

/// Which window a drag works in. The press names it: the pointer went down
/// over one pane, and the drag belongs to that pane until the button comes up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Queue,
    Report,
    /// A screen that is not a document: the model list, or the help. There is
    /// nothing behind it to scroll, so its rows are the lines, and a drag takes
    /// exactly what the eye sees.
    Screen,
}

/// A drag with the left button held, in the document's own rows rather than
/// the screen's. The difference is the whole point: a report taller than its
/// pane scrolls while the button is down, and a selection counted in screen
/// rows would slide out from under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub pane: Pane,
    /// The document row the press landed on, and the one the pointer is over
    /// now. They are the two corners of the selection.
    pub anchor: usize,
    pub cursor: usize,
    /// Where the press landed across the window. It is kept because a click
    /// has a place as well as a line, and because a drag made sideways and
    /// never up or down is still a click.
    pub column: u16,
}

impl Drag {
    /// Start a drag on one document row.
    pub fn start(pane: Pane, row: usize, column: u16) -> Self {
        Self { pane, anchor: row, cursor: row, column }
    }

    /// The same drag with its loose corner somewhere else, which is what a
    /// drag past the edge of a window does on every tick of the scroll.
    pub fn over(mut self, pane: Pane, row: usize) -> Self {
        self.pane = pane;
        self.cursor = row;
        self
    }

    /// The rows the drag covers, first to last, whichever way it was made.
    pub fn rows(&self) -> (usize, usize) {
        (self.anchor.min(self.cursor), self.anchor.max(self.cursor))
    }

    /// Whether the pointer moved across a line. A press and release on one
    /// line is a click -- it names a place, not a piece of text -- and a
    /// click that copied an invisible line would be a surprise.
    pub fn covers_a_line(&self) -> bool {
        self.anchor != self.cursor
    }
}

/// The text a drag took: whole rows of the document, in order, with the
/// padding the frame uses to fill the window taken off the end.
pub fn copied(document: &[String], drag: Drag) -> Option<String> {
    if !drag.covers_a_line() {
        return None;
    }
    let (first, last) = drag.rows();
    let taken: Vec<&str> = document
        .iter()
        .skip(first)
        .take(last - first + 1)
        .map(|l| l.trim_end())
        .collect();
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

/// One program that can put text on this machine's clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Helper<'a> {
    pub program: &'a str,
    pub args: &'a [&'a str],
}

/// The programs a clipboard might be behind, in the order worth trying. The
/// list is every desktop pearl is likely to meet, not the one it was built on.
pub fn routes() -> &'static [Helper<'static>] {
    &[
        Helper { program: "pbcopy", args: &[] },
        Helper { program: "wl-copy", args: &[] },
        Helper { program: "xclip", args: &["-selection", "clipboard"] },
        Helper { program: "clip.exe", args: &[] },
    ]
}

/// The first route this machine has, looked for where a shell would look.
pub fn helper() -> Option<Helper<'static>> {
    helper_on_paths(&std::env::var_os("PATH")?)
}

/// The first route inside one directory, for a caller that knows where to look
/// better than `PATH` does.
pub fn helper_on(dir: &str) -> Option<Helper<'static>> {
    helper_on_paths(std::ffi::OsStr::new(dir))
}

fn helper_on_paths(paths: &std::ffi::OsStr) -> Option<Helper<'static>> {
    routes().iter().copied().find(|route| {
        std::env::split_paths(paths).any(|dir| dir.join(route.program).is_file())
    })
}

/// Hand text to one route, and say whether it took it. A program that exits
/// non-zero, or is not there any more, has not carried anything: the answer is
/// what the next line of the report will rest on.
pub fn hand_to(helper: Helper<'_>, text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let spawned = Command::new(helper.program)
        .args(helper.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        return false;
    };
    if let Some(stdin) = child.stdin.as_mut()
        && stdin.write_all(text.as_bytes()).is_err()
    {
        return false;
    }
    drop(child.stdin.take());
    child.wait().map(|status| status.success()).unwrap_or(false)
}

/// What the surface says after a copy. Claiming a copy that never happened is
/// the one lie a copy can tell, and the human only catches it by pasting; so
/// when no route carried the text, the remark says which route was asked and
/// how to get the text anyway.
pub fn copy_remark(taken: usize, carried: bool) -> String {
    let lines = if taken == 1 { "line" } else { "lines" };
    if carried {
        format!("copied {taken} {lines}")
    } else {
        format!("{taken} {lines} sent as OSC 52; if that pastes nothing, Shift+drag selects for the terminal")
    }
}
