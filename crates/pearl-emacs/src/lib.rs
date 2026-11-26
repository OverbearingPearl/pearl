//! emacs --- the keys a person writing prose reaches for without looking,
//! as a state machine.
//!
//! It is not the whole of Emacs and does not pretend to be: motions, kills,
//! the mark, and one undo. What it is, is complete about the difference
//! between an editor and a text field: `C-k` kills into a ring the same
//! editor puts back with `C-y`, `C-Space` plants a mark the next kill takes,
//! and `C-/` walks the undo stack the buffer keeps.
//!
//! The key table is the host's to rewrite. `pearl_emacs::map()` is a default,
//! not a law: bind `⏎` to [`Command::Submit`] and the editor hands that key
//! back to the host instead of editing.

use pearl_text::{Buffer, Code, Command, Encoding, Key, KeyMap, Outcome, Unreachable};

/// Emacs's editing state that a buffer does not hold: what was killed, so
/// `C-y` can put it back.
#[derive(Debug, Clone, Default)]
pub struct Emacs {
    killed: String,
    /// Whether the last command took words away. Two kills in a row add to the
    /// same text, which is how `C-k C-k C-y` pastes two lines: the ring is
    /// replaced by any command that is not a kill.
    killing: bool,
}

impl Emacs {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the last kill took, for a host that wants to show it.
    pub fn killed(&self) -> &str {
        &self.killed
    }

    /// Answer one key. `map` is consulted first, so a host's rebinding wins
    /// over the default; a printable character the table says nothing about
    /// is text, which is what makes `C-a` a motion and `a` a letter.
    pub fn handle(&mut self, key: Key, buffer: &mut Buffer, map: &KeyMap) -> Outcome {
        if let Some(command) = map.on(key) {
            return self.run(command, buffer);
        }
        match key.code {
            Code::Char(c) if !key.ctrl && !key.alt => {
                buffer.insert(&c.to_string());
                Outcome::Handled
            }
            _ => Outcome::Ignored,
        }
    }

    fn run(&mut self, command: Command, buffer: &mut Buffer) -> Outcome {
        match command {
            Command::Submit | Command::Leave | Command::Cancel => Outcome::Host(command),
            Command::Yank => {
                self.killing = false;
                if self.killed.is_empty() {
                    Outcome::Ignored
                } else {
                    buffer.insert(&self.killed.clone());
                    Outcome::Handled
                }
            }
            other => {
                let was_killing = self.killing;
                let is_kill = is_kill(other);
                let before = if is_kill && was_killing {
                    self.killed.clone()
                } else {
                    String::new()
                };
                let outcome = pearl_text::execute(other, buffer, &mut self.killed)
                    .unwrap_or(Outcome::Ignored);
                if is_kill && was_killing && !before.is_empty() {
                    self.killed.insert_str(0, &before);
                }
                self.killing = is_kill;
                outcome
            }
        }
    }
}

/// Emacs's default table.
pub fn map() -> KeyMap {
    let mut map = KeyMap::new();
    for (key, command) in [
        (Key::new(Code::Backspace), Command::Backspace),
        (Key::new(Code::Delete), Command::Delete),
        (Key::new(Code::Left), Command::Left),
        (Key::new(Code::Right), Command::Right),
        (Key::new(Code::Up), Command::Up),
        (Key::new(Code::Down), Command::Down),
        (Key::new(Code::Home), Command::LineStart),
        (Key::new(Code::End), Command::LineEnd),
        (Key::ch('a').ctrl(), Command::LineStart),
        (Key::ch('e').ctrl(), Command::LineEnd),
        (Key::ch('b').ctrl(), Command::Left),
        (Key::ch('f').ctrl(), Command::Right),
        (Key::ch('p').ctrl(), Command::Up),
        (Key::ch('n').ctrl(), Command::Down),
        (Key::ch('b').alt(), Command::WordLeft),
        (Key::ch('f').alt(), Command::WordRight),
        (Key::ch('d').ctrl(), Command::Delete),
        (Key::ch('k').ctrl(), Command::KillToLineEnd),
        (Key::ch('u').ctrl(), Command::KillToLineStart),
        (Key::ch('w').ctrl(), Command::KillSelection),
        (Key::ch('w').alt(), Command::CopySelection),
        (Key::ch('y').ctrl(), Command::Yank),
        (Key::ch('d').alt(), Command::KillWordForward),
        (Key::new(Code::Backspace).alt(), Command::KillWordBackward),
        (Key::ch('t').ctrl(), Command::Transpose),
        (Key::ch('o').ctrl(), Command::OpenLine),
        (Key::ch('<').alt(), Command::Start),
        (Key::ch('>').alt(), Command::End),
        (Key::ch(' ').ctrl(), Command::Mark),
        (Key::ch('/').ctrl(), Command::Undo),
        (Key::ch('_').ctrl(), Command::Undo),
        (Key::escape(), Command::Cancel),
        (Key::ch('g').ctrl(), Command::Cancel),
    ] {
        map = map.bind(key, command);
    }
    map
}

/// Whether a command puts words in the ring. Two of them in a row add to it,
/// which is the difference between killing a line and killing a paragraph.
fn is_kill(command: Command) -> bool {
    matches!(
        command,
        Command::KillToLineEnd
            | Command::KillToLineStart
            | Command::KillSelection
            | Command::KillWordForward
            | Command::KillWordBackward
            | Command::KillLine
    )
}

/// The bindings this editor's table carries that the terminal cannot deliver
/// as written, with the reason for each. A host prints its hints through this:
/// a chord the terminal folds into another key is not a key.
pub fn unreachable(encoding: Encoding) -> Vec<Unreachable> {
    map().unreachable(encoding)
}

/// The keys this editor answers, in the order a hand meets them. They are the
/// editor's own document: a host decides whether to print them, and where.
pub fn help() -> Vec<(&'static str, &'static str)> {
    vec![
        ("C-a|C-e", "the start and the end of the line"),
        (
            "C-f|C-b|C-n|C-p",
            "one character forward and back, one line down and up",
        ),
        ("M-f|M-b", "one word forward and back"),
        (
            "C-k|C-y",
            "kill to the end of the line, and put what was killed back",
        ),
        ("C-w|M-w", "cut and copy the marked words"),
        ("M-d|M-\u{232b}", "kill the next word and the word behind"),
        ("C-u", "cut back to the start of the line"),
        ("C-d", "delete the character under the caret"),
        ("C-t", "swap the two characters at the caret"),
        ("C-o", "open a line above, the caret staying where it is"),
        ("M-<|M->", "the start and the end of the draft"),
        ("C-/", "undo"),
        ("C-Space", "plant the mark"),
        (
            "C-j",
            "break the line, where the terminal tells it from C-m",
        ),
        ("Tab|Esc", "leave the words, and put them down"),
    ]
}
