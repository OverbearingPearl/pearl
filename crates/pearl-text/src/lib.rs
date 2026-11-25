//! text --- the words a human is writing, the caret in them, and the
//! vocabulary an editor uses to talk about both.
//!
//! This crate knows nothing about terminals. It has no rendering, no events,
//! no `crossterm` and no clock: [`Buffer`] holds characters and a caret,
//! [`Key`] says what was pressed, [`Command`] says what it means, and
//! [`KeyMap`] is the table that decides the second from the first. That split
//! is what lets `pearl-emacs` and `pearl-vim` be libraries a host drives with
//! its own event loop, and what lets the host rewrite a binding without
//! forking an editor.
//!
//! Positions are counted in characters, never bytes. A title in Chinese is
//! one character per cell, and a caret counted in bytes lands in the middle
//! of one.

/// How many edits a buffer remembers. A draft is a paragraph, not a file: a
/// hand that has taken back one line is not about to walk back thirty.
const UNDO_LIMIT: usize = 100;

/// A key, as far as an editor cares.
///
/// Deliberately its own type rather than the terminal library's: a crate that
/// names `crossterm` in its public API is a crate whose users upgrade when
/// `crossterm` does. A host writes one `From` impl and owns the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: Code,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Which key it was, ignoring the modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Enter,
    Tab,
    Escape,
    /// A key no editor has an opinion about (a function key, a paste burst):
    /// it reaches the host unclaimed.
    Other,
}

impl Key {
    /// A key with no modifiers held.
    pub const fn new(code: Code) -> Self {
        Self { code, shift: false, ctrl: false, alt: false }
    }

    /// A character typed as it is.
    pub const fn ch(c: char) -> Self {
        Self::new(Code::Char(c))
    }

    pub const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// The key for a typed character, as an editor would receive it: a plain
    /// letter, or a letter held with a modifier.
    pub const fn typed(c: char, ctrl: bool, alt: bool) -> Self {
        Self { code: Code::Char(c), shift: false, ctrl, alt }
    }
}

impl Key {
    /// `⏎`, the key a form is handed in with.
    pub const fn enter() -> Self {
        Self::new(Code::Enter)
    }

    /// `⇧⏎`, a break in the words rather than the end of them.
    pub const fn shift_enter() -> Self {
        Self::enter().shift()
    }

    pub const fn tab() -> Self {
        Self::new(Code::Tab)
    }

    pub const fn escape() -> Self {
        Self::new(Code::Escape)
    }
}

/// What a key means, once a table has said so.
///
/// Every key an editor answers is one of these, including the ones only the
/// host can carry out ([`Command::Submit`], [`Command::Leave`],
/// [`Command::Cancel`]): a host that wants `⏎` to hand a task in, and not to
/// break a line, rebinds one entry and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    // typing
    Insert(char),
    Break,
    Backspace,
    Delete,
    // motion
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    Start,
    End,
    // taking words away and putting them back
    KillToLineEnd,
    KillToLineStart,
    KillSelection,
    KillWordForward,
    KillWordBackward,
    CopySelection,
    Yank,
    Mark,
    Undo,
    /// Swap the character before the caret with the one at it (`C-t`).
    Transpose,
    /// Break the line at the caret and leave it there (`C-o`).
    OpenLine,
    // whole lines (vim's `dd`, `cc`, `yy`)
    KillLine,
    CopyLine,
    ChangeLine,
    // vim's modes
    Normal,
    InsertMode,
    Append,
    InsertLineStart,
    AppendLineEnd,
    OpenBelow,
    OpenAbove,
    Visual,
    // the host's own verbs
    Submit,
    Leave,
    Cancel,
}

/// What an editor did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The words or the caret changed.
    Handled,
    /// The key meant nothing here. The host may still want it: this is how a
    /// global key reaches past an editor that is holding the keys.
    Ignored,
    /// The key means something the editor cannot do alone -- hand the form in,
    /// leave the field, put it down. The host owns those.
    Host(Command),
}

/// One key, one meaning. The host builds the table (or takes an editor's
/// default) and may rebind any entry, including the ones only it can carry
/// out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMap {
    table: Vec<(Key, Command)>,
}

/// What a terminal can tell apart. A terminal that says nothing speaks the
/// plain encoding, where Enter, Tab, Escape and a few control chords arrive as
/// one key; one that answers the keyboard protocol can send a distinct code for
/// each of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    Plain,
    Enhanced,
}

/// A binding the terminal will never deliver as written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unreachable {
    pub key: Key,
    /// What the terminal sends instead, in words a human can read.
    pub why: &'static str,
}

/// The chords a terminal folds into another key when it speaks the plain
/// encoding.
fn folded(character: char) -> Option<&'static str> {
    Some(match character {
        'i' => "arrives as Tab",
        'm' => "arrives as \u{23ce}",
        'j' => "arrives as \u{23ce}, like C-m",
        'h' => "arrives as \u{232b}",
        '[' => "arrives as \u{241b}",
        '@' => "arrives as NUL, like C-Space",
        ' ' => "arrives as NUL, like C-@",
        '\u{7f}' | '?' => "arrives as \u{232b}",
        _ => return None,
    })
}

impl KeyMap {
    pub fn new() -> Self {
        Self { table: Vec::new() }
    }

    /// Bind a key. A later binding of the same key wins, so a host can start
    /// from an editor's table and replace one entry without knowing the rest.
    pub fn bind(mut self, key: Key, command: Command) -> Self {
        self.table.retain(|(bound, _)| *bound != key);
        self.table.insert(0, (key, command));
        self
    }

    /// What this key means, or `None` when the table has nothing to say --
    /// which is how a printable character becomes text.
    /// The bindings this table carries that the terminal cannot deliver as
    /// written, with the reason. A host asks before it prints a key: a hint
    /// naming a chord the terminal folds is a promise the keys cannot keep.
    pub fn unreachable(&self, encoding: Encoding) -> Vec<Unreachable> {
        let mut found: Vec<Unreachable> = Vec::new();
        for (key, _) in &self.table {
            let why = match encoding {
                Encoding::Plain => {
                    if key.ctrl {
                        match key.code {
                            Code::Char(character) => folded(character),
                            _ => None,
                        }
                        .or_else(|| key.alt.then_some("a terminal cannot tell this from the same key without Alt"))
                    } else if key.shift && matches!(key.code, Code::Char(_)) {
                        Some("a terminal sends the character, not the shift")
                    } else {
                        None
                    }
                }
                Encoding::Enhanced => match (key.ctrl, key.code) {
                    (true, Code::Char('@')) => Some("arrives as NUL, like C-Space"),
                    _ => None,
                },
            };
            if let Some(why) = why {
                if !found.iter().any(|seen| seen.key == *key) {
                    found.push(Unreachable { key: *key, why });
                }
            }
        }
        found
    }

    pub fn on(&self, key: Key) -> Option<Command> {
        self.table.iter().find(|(bound, _)| *bound == key).map(|(_, command)| *command)
    }

    /// Every binding, in the order they are consulted. For a host that wants
    /// to print what an editor answers.
    pub fn entries(&self) -> impl Iterator<Item = (Key, Command)> + '_ {
        self.table.iter().copied()
    }
}

/// The text being written, and where the writing is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Buffer {
    text: String,
    caret: usize,
    /// Where a selection was planted, when one is live.
    mark: Option<usize>,
    /// States to come back to, oldest first.
    undo: Vec<(String, usize)>,
}

impl Buffer {
    /// A buffer that starts from what is already there, with the caret at the
    /// end: rewriting a task begins wherever the old words ended.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        Self { text, caret, mark: None, undo: Vec::new() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Put the caret at `at`, clamped to the end, and lift the selection. For
    /// a host restoring a caret it kept; motions keep the mark, because a
    /// planted mark is a region the next motion extends.
    pub fn set_caret(&mut self, at: usize) {
        self.caret = at.min(self.len_chars());
        self.mark = None;
    }

    /// Whether the words have been broken into more than one line.
    pub fn has_break(&self) -> bool {
        self.text.contains('\n')
    }

    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }

    fn byte_at(&self, caret: usize) -> usize {
        self.text.char_indices().nth(caret).map_or(self.text.len(), |(at, _)| at)
    }

    fn remember(&mut self) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push((self.text.clone(), self.caret));
    }

    /// Put text at the caret, replacing the selection when one is live.
    pub fn insert(&mut self, piece: &str) {
        self.remember();
        let from = self.selection_start();
        let to = self.selection_end();
        let (a, b) = (self.byte_at(from), self.byte_at(to));
        self.text.replace_range(a..b, piece);
        self.caret = from + piece.chars().count();
        self.mark = None;
    }

    /// One break, at the caret.
    pub fn break_line(&mut self) {
        self.insert("\n");
    }

    /// Take back one character, or the selection when one is live.
    pub fn backspace(&mut self) {
        if self.drop_selection() {
            return;
        }
        if self.caret == 0 {
            return;
        }
        self.cut(self.caret - 1, self.caret);
    }

    /// Take out the character under the caret (`C-d`, `x`).
    pub fn delete(&mut self) {
        if self.drop_selection() {
            return;
        }
        if self.caret < self.len_chars() {
            self.cut(self.caret, self.caret + 1);
        }
    }

    /// Whether a live selection was cut. One answer, so every verb that
    /// replaces rather than edits asks the same question.
    fn drop_selection(&mut self) -> bool {
        match self.selection() {
            Some((from, to)) => {
                self.cut(from, to);
                true
            }
            None => false,
        }
    }

    /// Remove `from..to` and leave the caret at `from`.
    pub fn cut(&mut self, from: usize, to: usize) -> String {
        let (from, to) = (from.min(self.len_chars()), to.min(self.len_chars()));
        if from >= to {
            return String::new();
        }
        self.remember();
        let (a, b) = (self.byte_at(from), self.byte_at(to));
        let taken: String = self.text[a..b].to_string();
        self.text.replace_range(a..b, "");
        self.caret = from;
        self.mark = None;
        taken
    }

    /// The marked range, when one is live and not empty.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.mark?;
        let (from, to) = (self.selection_start(), self.selection_end());
        (from != to).then_some((from, to))
    }

    fn selection_start(&self) -> usize {
        self.mark.map_or(self.caret, |m| m.min(self.caret))
    }

    fn selection_end(&self) -> usize {
        self.mark.map_or(self.caret, |m| m.max(self.caret))
    }

    /// Plant or lift the selection anchor (`C-Space`, vim's `v`).
    pub fn toggle_mark(&mut self) {
        self.mark = if self.mark.is_some() { None } else { Some(self.caret) };
    }

    pub fn clear_mark(&mut self) {
        self.mark = None;
    }

    pub fn has_mark(&self) -> bool {
        self.mark.is_some()
    }

    pub fn undo(&mut self) {
        if let Some((text, caret)) = self.undo.pop() {
            self.text = text;
            self.caret = caret.min(self.len_chars());
            self.mark = None;
        }
    }

    pub fn left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.caret = (self.caret + 1).min(self.len_chars());
    }

    /// The line the caret is on, as `from..to` with `to` past its break.
    pub fn line_range(&self, at: usize) -> (usize, usize) {
        let chars = self.chars();
        let at = at.min(chars.len());
        let from = chars[..at].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
        let to = chars[at..]
            .iter()
            .position(|&c| c == '\n')
            .map_or(chars.len(), |i| at + i + 1);
        (from, to)
    }

    fn line_start_at(&self, at: usize) -> usize {
        self.line_range(at).0
    }

    fn line_end_at(&self, at: usize) -> usize {
        let (_, to) = self.line_range(at);
        // The break belongs to the line, but a caret cannot stand past it.
        if to > 0 && self.chars().get(to - 1) == Some(&'\n') { to - 1 } else { to }
    }

    pub fn line_start(&mut self) {
        self.caret = self.line_start_at(self.caret);
    }

    pub fn line_end(&mut self) {
        self.caret = self.line_end_at(self.caret);
    }

    /// Up one line, keeping the column where it can. This is a buffer's line,
    /// not the screen's: a wrapped paragraph is one line to the person who
    /// wrote it as one.
    pub fn up(&mut self) {
        let start = self.line_start_at(self.caret);
        if start == 0 {
            return;
        }
        let column = self.caret - start;
        let previous = self.line_start_at(start - 1);
        let end = start - 1;
        self.caret = (previous + column).min(end);
    }

    pub fn down(&mut self) {
        let start = self.line_start_at(self.caret);
        let (_, end) = self.line_range(self.caret);
        if end >= self.len_chars() {
            return;
        }
        let column = self.caret - start;
        let next = end;
        let next_end = self.line_end_at(next);
        self.caret = (next + column).min(next_end);
    }

    pub fn word_left(&mut self) {
        let chars = self.chars();
        let mut at = self.caret;
        while at > 0 && chars[at - 1].is_whitespace() {
            at -= 1;
        }
        while at > 0 && !chars[at - 1].is_whitespace() {
            at -= 1;
        }
        self.caret = at;
    }

    pub fn word_right(&mut self) {
        let chars = self.chars();
        let mut at = self.caret;
        while at < chars.len() && !chars[at].is_whitespace() {
            at += 1;
        }
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        self.caret = at;
    }

    /// Everything from the caret to the end of its line (`C-k`).
    pub fn kill_to_end(&mut self) -> String {
        let end = self.line_end_at(self.caret);
        if end == self.caret {
            // At the end of a line the break is what is left to take.
            self.cut(self.caret, (self.caret + 1).min(self.len_chars()))
        } else {
            self.cut(self.caret, end)
        }
    }

    /// Everything from the caret to its line's first character (`C-u`).
    pub fn kill_to_start(&mut self) -> String {
        let start = self.line_start_at(self.caret);
        self.cut(start, self.caret)
    }

    /// The word after the caret, breaks and spaces included up to its start
    /// (`M-d`), or the one before it (`M-Backspace`).
    pub fn kill_word(&mut self, forward: bool) -> String {
        let chars = self.chars();
        let mut at = self.caret;
        if forward {
            while at < chars.len() && chars[at].is_whitespace() && chars[at] != '\n' {
                at += 1;
            }
            while at < chars.len() && !chars[at].is_whitespace() {
                at += 1;
            }
            if at == self.caret {
                return String::new();
            }
            self.cut(self.caret, at)
        } else {
            while at > 0 && chars[at - 1].is_whitespace() && chars[at - 1] != '\n' {
                at -= 1;
            }
            while at > 0 && !chars[at - 1].is_whitespace() {
                at -= 1;
            }
            if at == self.caret {
                return String::new();
            }
            self.cut(at, self.caret)
        }
    }

    /// Swap the character before the caret with the one under it and step past
    /// both (`C-t`), the way emacs does in the middle of a line.
    pub fn transpose(&mut self) {
        let chars = self.chars();
        if chars.len() < 2 || self.caret == 0 {
            return;
        }
        let at = if self.caret >= chars.len() { chars.len() - 1 } else { self.caret };
        let mut swapped = String::with_capacity(self.text.len());
        for (i, c) in chars.iter().enumerate() {
            if i + 1 == at {
                swapped.push(chars[at]);
            } else if i == at {
                swapped.push(chars[at - 1]);
            } else {
                swapped.push(*c);
            }
        }
        let caret = (at + 1).min(swapped.chars().count());
        self.undo.push((self.text.clone(), self.caret));
        self.text = swapped;
        self.caret = caret;
        self.mark = None;
    }

    /// Break the line at the caret and leave the caret before the break, which
    /// is what `C-o` is for: room above, without moving the words around.
    pub fn open_line(&mut self) {
        self.mark = None;
        self.undo.push((self.text.clone(), self.caret));
        let at = self.byte_at(self.caret);
        self.text.insert(at, '\n');
    }

    /// The words under the caret and up to the end of them (`M-w`).
    pub fn copy_word(&self) -> String {
        let chars = self.chars();
        let mut at = self.caret;
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        let from = at;
        while at < chars.len() && !chars[at].is_whitespace() {
            at += 1;
        }
        chars[from..at].iter().collect()
    }

    /// The whole line the caret is on, break included where it has one.
    pub fn line_text(&self) -> String {
        let chars = self.chars();
        let (from, to) = self.line_range(self.caret);
        let to = if to > from && chars.get(to - 1) == Some(&'\n') { to - 1 } else { to };
        chars[from..to].iter().collect()
    }

    pub fn first(&mut self) {
        self.caret = 0;
    }

    pub fn last(&mut self) {
        self.caret = self.len_chars();
    }
}

/// The table a buffer with no editor behind it answers: the keys every
/// terminal sends, and nothing that would need a mode.
pub fn plain() -> KeyMap {
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
        (Key::escape(), Command::Cancel),
    ] {
        map = map.bind(key, command);
    }
    map
}

/// Answer a key for a plain text field: the table's commands, and a printable
/// character when the table has nothing to say.
pub fn plain_handle(key: Key, buffer: &mut Buffer, map: &KeyMap) -> Outcome {
    let mut killed = String::new();
    if let Some(command) = map.on(key) {
        return match command {
            Command::Submit | Command::Leave | Command::Cancel => Outcome::Host(command),
            other => execute(other, buffer, &mut killed).unwrap_or(Outcome::Ignored),
        };
    }
    match key.code {
        Code::Char(c) if !key.ctrl && !key.alt => {
            buffer.insert(&c.to_string());
            Outcome::Handled
        }
        _ => Outcome::Ignored,
    }
}

/// Run one command against a buffer, for the commands that are the same in
/// every editor. `None` means the command belongs to an editor's own modes.
pub fn execute(command: Command, buffer: &mut Buffer, killed: &mut String) -> Option<Outcome> {
    let outcome = match command {
        Command::Insert(c) => {
            buffer.insert(&c.to_string());
            Outcome::Handled
        }
        Command::Break => {
            buffer.break_line();
            Outcome::Handled
        }
        Command::Backspace => {
            buffer.backspace();
            Outcome::Handled
        }
        Command::Delete => {
            buffer.delete();
            Outcome::Handled
        }
        Command::Left => {
            buffer.left();
            Outcome::Handled
        }
        Command::Right => {
            buffer.right();
            Outcome::Handled
        }
        Command::Up => {
            buffer.up();
            Outcome::Handled
        }
        Command::Down => {
            buffer.down();
            Outcome::Handled
        }
        Command::WordLeft => {
            buffer.word_left();
            Outcome::Handled
        }
        Command::WordRight => {
            buffer.word_right();
            Outcome::Handled
        }
        Command::LineStart => {
            buffer.line_start();
            Outcome::Handled
        }
        Command::LineEnd => {
            buffer.line_end();
            Outcome::Handled
        }
        Command::Start => {
            buffer.first();
            Outcome::Handled
        }
        Command::End => {
            buffer.last();
            Outcome::Handled
        }
        Command::KillToLineEnd => {
            *killed = buffer.kill_to_end();
            Outcome::Handled
        }
        Command::KillToLineStart => {
            *killed = buffer.kill_to_start();
            Outcome::Handled
        }
        Command::KillWordForward => {
            *killed = buffer.kill_word(true);
            Outcome::Handled
        }
        Command::KillWordBackward => {
            *killed = buffer.kill_word(false);
            Outcome::Handled
        }
        Command::Transpose => {
            buffer.transpose();
            Outcome::Handled
        }
        Command::OpenLine => {
            buffer.open_line();
            Outcome::Handled
        }
        Command::KillSelection => match buffer.selection() {
            Some((from, to)) => {
                *killed = buffer.cut(from, to);
                Outcome::Handled
            }
            None => Outcome::Ignored,
        },
        Command::CopySelection => match buffer.selection() {
            Some((from, to)) => {
                let words = buffer.text().chars().skip(from).take(to - from);
                *killed = words.collect();
                buffer.clear_mark();
                Outcome::Handled
            }
            None => Outcome::Ignored,
        },
        Command::Yank => {
            if killed.is_empty() {
                Outcome::Ignored
            } else {
                buffer.insert(killed);
                Outcome::Handled
            }
        }
        Command::Mark => {
            buffer.toggle_mark();
            Outcome::Handled
        }
        Command::Undo => {
            buffer.undo();
            Outcome::Handled
        }
        _ => return None,
    };
    Some(outcome)
}
