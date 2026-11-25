//! vim --- Normal, Insert and Visual, and the verbs a paragraph is written
//! with, as a state machine.
//!
//! Not the whole of Vim: no text objects, no `.`, no registers beyond one,
//! no macros. What is here is the shape -- a mode decides what a key means,
//! an operator takes a target, `u` takes back the last line -- because that
//! shape is what a person's hands remember.
//!
//! The tables are the host's to rewrite (`pearl_vim::insert()` and
//! `normal()` are defaults). The keys that *begin* a sequence (`g`, `d`, `c`,
//! `y`) are the editor's: a two-key verb cannot be rebound one half at a
//! time, and pretending otherwise would leave a table that lies.

use pearl_text::{Buffer, Code, Command, Key, KeyMap, Outcome};

/// Which set of meanings the keys have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
}

/// Vim's state that a buffer does not hold: the mode, the sequence being
/// typed, and the one register.
#[derive(Debug, Clone)]
pub struct Vim {
    mode: Mode,
    pending: Option<char>,
    register: String,
}

impl Default for Vim {
    fn default() -> Self {
        Self::typing()
    }
}

impl Vim {
    /// A draft that starts in Insert, because the human came here to write a
    /// sentence and not to move around one.
    pub fn typing() -> Self {
        Self { mode: Mode::Insert, pending: None, register: String::new() }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Start in a mode, for a host that keeps a draft between screens and
    /// comes back to it where it was left.
    pub fn starting_in(mode: Mode) -> Self {
        Self { mode, pending: None, register: String::new() }
    }

    /// Whether a two-key verb is waiting for its second key.
    pub fn pending(&self) -> Option<char> {
        self.pending
    }

    pub fn register(&self) -> &str {
        &self.register
    }

    /// The table the current mode reads from.
    pub fn map(&self) -> KeyMap {
        match self.mode {
            Mode::Insert => insert(),
            Mode::Normal | Mode::Visual => normal(),
        }
    }

    pub fn handle(&mut self, key: Key, buffer: &mut Buffer, map: &KeyMap) -> Outcome {
        if let Some(prefix) = self.pending.take() {
            return self.finish(prefix, key, buffer);
        }
        if self.mode == Mode::Insert {
            if key.code == Code::Escape {
                self.mode = Mode::Normal;
                return Outcome::Handled;
            }
            if let Some(command) = map.on(key) {
                return self.run(command, buffer);
            }
            return match key.code {
                Code::Char(c) if !key.ctrl && !key.alt => {
                    buffer.insert(&c.to_string());
                    Outcome::Handled
                }
                _ => Outcome::Ignored,
            };
        }
        // Normal and Visual: a sequence first, then the table, then nothing.
        if let Code::Char(prefix @ ('g' | 'd' | 'c' | 'y')) = key.code
            && !key.ctrl
            && !key.alt
        {
            if self.mode == Mode::Visual && prefix != 'g' {
                return self.take_line(prefix, buffer);
            }
            self.pending = Some(prefix);
            return Outcome::Handled;
        }
        if let Some(command) = map.on(key) {
            return self.run(command, buffer);
        }
        Outcome::Ignored
    }

    /// The second key of a sequence.
    fn finish(&mut self, prefix: char, key: Key, buffer: &mut Buffer) -> Outcome {
        match (prefix, key.code) {
            ('g', Code::Char('g')) => {
                buffer.first();
                Outcome::Handled
            }
            (prefix @ ('d' | 'c' | 'y'), Code::Char(c)) if c == prefix => {
                self.take_line(prefix, buffer)
            }
            (verb @ ('d' | 'c' | 'y'), _) if self.mode == Mode::Visual => {
                self.take_line(verb, buffer)
            }
            _ => Outcome::Ignored,
        }
    }

    /// `dd`, `cc`, `yy`: the marked words when one is marked, the line the
    /// caret is on otherwise. One operator, one target, in that order.
    fn take_line(&mut self, verb: char, buffer: &mut Buffer) -> Outcome {
        let range = buffer.selection().unwrap_or_else(|| {
            let (from, to) = buffer.line_range(buffer.caret());
            let to = if to > from { to } else { (from + 1).min(buffer.len_chars()) };
            (from, to)
        });
        let (from, to) = range;
        if from == to {
            return Outcome::Ignored;
        }
        match verb {
            'y' => {
                self.register = buffer.text().chars().skip(from).take(to - from).collect();
                buffer.clear_mark();
            }
            'd' | 'c' => {
                self.register = buffer.cut(from, to);
                if verb == 'c' {
                    self.mode = Mode::Insert;
                }
            }
            _ => return Outcome::Ignored,
        }
        self.mode = if verb == 'c' { Mode::Insert } else { Mode::Normal };
        Outcome::Handled
    }

    fn run(&mut self, command: Command, buffer: &mut Buffer) -> Outcome {
        match command {
            Command::Submit | Command::Leave => Outcome::Host(command),
            Command::Cancel => {
                buffer.clear_mark();
                Outcome::Host(Command::Cancel)
            }
            Command::Normal => {
                self.mode = Mode::Normal;
                buffer.clear_mark();
                Outcome::Handled
            }
            Command::InsertMode => {
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::Append => {
                buffer.right();
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::InsertLineStart => {
                buffer.line_start();
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::AppendLineEnd => {
                buffer.line_end();
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::OpenBelow => {
                buffer.line_end();
                buffer.break_line();
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::OpenAbove => {
                buffer.line_start();
                buffer.break_line();
                buffer.up();
                self.mode = Mode::Insert;
                Outcome::Handled
            }
            Command::Visual => {
                buffer.toggle_mark();
                self.mode = if self.mode == Mode::Visual { Mode::Normal } else { Mode::Visual };
                Outcome::Handled
            }
            Command::Yank => {
                if self.register.is_empty() {
                    Outcome::Ignored
                } else {
                    buffer.insert(&self.register.clone());
                    Outcome::Handled
                }
            }
            Command::KillLine => self.take_line('d', buffer),
            Command::CopyLine => self.take_line('y', buffer),
            Command::ChangeLine => self.take_line('c', buffer),
            other => {
                pearl_text::execute(other, buffer, &mut self.register).unwrap_or(Outcome::Ignored)
            }
        }
    }
}

/// Insert mode: the keys a text field answers, and the way back out.
pub fn insert() -> KeyMap {
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
        (Key::escape(), Command::Normal),
    ] {
        map = map.bind(key, command);
    }
    map
}

/// Normal mode: the motions, the operators, the modes and one undo.
pub fn normal() -> KeyMap {
    let mut map = KeyMap::new();
    for (key, command) in [
        (Key::ch('h'), Command::Left),
        (Key::ch('l'), Command::Right),
        (Key::ch('j'), Command::Down),
        (Key::ch('k'), Command::Up),
        (Key::ch('w'), Command::WordRight),
        (Key::ch('b'), Command::WordLeft),
        (Key::ch('0'), Command::LineStart),
        (Key::ch('$'), Command::LineEnd),
        (Key::ch('G'), Command::End),
        (Key::ch('v'), Command::Visual),
        (Key::ch('x'), Command::Delete),
        (Key::ch('u'), Command::Undo),
        (Key::ch('p'), Command::Yank),
        (Key::ch('i'), Command::InsertMode),
        (Key::ch('a'), Command::Append),
        (Key::ch('I'), Command::InsertLineStart),
        (Key::ch('A'), Command::AppendLineEnd),
        (Key::ch('o'), Command::OpenBelow),
        (Key::ch('O'), Command::OpenAbove),
        (Key::ch('X'), Command::Backspace),
        (Key::escape(), Command::Cancel),
    ] {
        map = map.bind(key, command);
    }
    map
}
