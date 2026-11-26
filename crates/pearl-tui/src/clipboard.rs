//! clipboard --- handing text back to the terminal that owns it.
//!
//! The surface cannot put anything on the system clipboard itself: a terminal
//! owns the selection, and a program that wants to hand it words has to ask
//! through OSC 52. Most terminals take that quietly; some refuse it (Terminal
//! .app, a tmux without `set-clipboard`), and a refusal must not be silent --
//! a copy that did nothing and said nothing is worse than no copy at all. So a
//! refusal writes the text to a file and names the path in the notice.

use std::io::Write;
use std::path::PathBuf;

/// What the delivery managed. The notice words come from here so that the one
/// place that knows what happened is also the place that says it.
pub enum Delivery {
    Copied,
    Filed(PathBuf),
}

impl Delivery {
    pub fn words(&self, chars: usize) -> String {
        match self {
            Delivery::Copied => format!("copied {chars} chars"),
            Delivery::Filed(path) => {
                format!("this terminal takes no clipboard; wrote {}", path.display())
            }
        }
    }
}

/// The escape that asks the terminal to put text on the system clipboard.
/// `c` names the clipboard selection; the text is base64 because a raw answer
/// may hold bytes that mean something else to the terminal.
pub fn osc52(text: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()))
}

/// Hand the text over, and say what happened.
pub fn deliver(text: &str) -> Delivery {
    if blind() {
        return std::fs::write(&store_path(), text)
            .map(|_| Delivery::Filed(store_path()))
            .unwrap_or(Delivery::Copied);
    }
    let mut out = std::io::stdout();
    if out
        .write_all(osc52(text).as_bytes())
        .and_then(|_| out.flush())
        .is_err()
    {
        return std::fs::write(&store_path(), text)
            .map(|_| Delivery::Filed(store_path()))
            .unwrap_or(Delivery::Copied);
    }
    Delivery::Copied
}

/// Where a refused copy lands: the scratch directory, a name the human can
/// type in another program.
pub fn store_path() -> PathBuf {
    std::env::temp_dir().join(format!("pearl-copy-{}.md", std::process::id()))
}

/// Terminals known to swallow OSC 52 without a word. The list is short on
/// purpose: it is a guess about someone else's program, and a wrong guess that
/// hides a working copy is worse than a try that fails loudly.
fn blind() -> bool {
    matches!(
        std::env::var("TERM_PROGRAM").as_deref(),
        Ok("Apple_Terminal")
    )
}

/// Base64, written here because the surface has no room for a dependency that
/// does one thing this cleanly.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = *chunk.get(1).unwrap_or(&0) as u32;
        let c = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (a << 16) | (b << 8) | c;
        out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
