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

use pearl_text::{Buffer, Code, Command, Key, KeyMap, Outcome};

/// Emacs's editing state that a buffer does not hold: what was killed, so
/// `C-y` can put it back.
#[derive(Debug, Clone, Default)]
pub struct Emacs {
    killed: String,
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
                if self.killed.is_empty() {
                    Outcome::Ignored
                } else {
                    buffer.insert(&self.killed.clone());
                    Outcome::Handled
                }
            }
            other => {
                pearl_text::execute(other, buffer, &mut self.killed).unwrap_or(Outcome::Ignored)
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
