//! app --- the interactive surface: a queue, a focus, and a hand.
//!
//! The surface is the queue made editable. The human moves a single focus
//! cursor through the flat view and acts on the focused line with one key. The
//! editing round trip is visible: `take` lifts an unexecuted node into the hand
//! (a request held in the footer), the human edits it, and `place` puts it back
//! at a chosen position. The hand is the loose part made visible.
//!
//! Key handling is pure: it mutates the queue and the mode and needs no
//! terminal, so the stories drive it with synthetic keys.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_core::{Aside, Form, Position, Queue, Request, TaskId};

/// What the surface is doing: navigating, or typing into a prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Input(Input),
}

/// A text prompt the human is typing into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub kind: InputKind,
    pub buffer: String,
}

/// What committing the prompt will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKind {
    /// Place a new task after the focus.
    Append,
    /// Place a new task at the front.
    InsertNow,
    /// Annotate a task with steering.
    Steer(TaskId),
    /// Replace a task's objective.
    Redirect(TaskId),
    /// Edit the title of the request in the hand.
    EditHand,
    /// Ask a side question that enters neither the queue nor the log.
    Aside,
}

/// The interactive surface state.
#[derive(Debug, Clone)]
pub struct App {
    pub queue: Queue,
    pub mode: Mode,
    /// A taken request awaiting placement -- the loose part, made visible.
    pub hand: Option<Request>,
    /// A side question held outside the queue, until dismissed.
    pub aside: Option<Aside>,
    pub quit: bool,
}

impl App {
    pub fn new(queue: Queue) -> Self {
        Self { queue, mode: Mode::Normal, hand: None, aside: None, quit: false }
    }

    /// The focused node's id, if the focus is on a node in the current view.
    fn focused(&self) -> Option<TaskId> {
        let focus = self.queue.focus()?;
        self.queue.view().iter().find(|n| n.id == focus).map(|n| n.id)
    }

    /// Move the focus by `delta` within the flat view, clamped to the ends.
    fn move_focus(&mut self, delta: isize) {
        let ids: Vec<TaskId> = self.queue.view().iter().map(|n| n.id).collect();
        if ids.is_empty() {
            return;
        }
        let current = self.queue.focus().and_then(|f| ids.iter().position(|i| *i == f)).unwrap_or(0);
        let next = (current as isize + delta).clamp(0, ids.len() as isize - 1) as usize;
        let _ = self.queue.focus_on(ids[next]);
    }

    /// Handle one key. Pure: mutates state, returns nothing, needs no terminal.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.mode {
            Mode::Input(_) => self.handle_input_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_focus(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_focus(-1),
            KeyCode::Char('b') => self.prompt(InputKind::Aside),
            KeyCode::Esc => self.aside = None,
            KeyCode::Char('a') => self.prompt(InputKind::Append),
            KeyCode::Char('i') => self.prompt(InputKind::InsertNow),
            KeyCode::Char('s') => {
                if let Some(id) = self.focused() {
                    self.prompt(InputKind::Steer(id));
                }
            }
            KeyCode::Char('R') => {
                if let Some(id) = self.focused() {
                    self.prompt(InputKind::Redirect(id));
                }
            }
            KeyCode::Char('t') => {
                if let Some(id) = self.focused() {
                    if let Ok((request, _)) = self.queue.take(id) {
                        self.hand = Some(request);
                    }
                }
            }
            KeyCode::Char('e') => {
                if self.hand.is_some() {
                    self.prompt(InputKind::EditHand);
                }
            }
            KeyCode::Char('p') => self.place_hand(Position::After),
            KeyCode::Char('P') => self.place_hand(Position::Now),
            KeyCode::Char('h') => {
                if let Some(id) = self.focused() {
                    let _ = self.queue.hold(id, "");
                }
            }
            KeyCode::Char('r') => {
                if let Some(id) = self.focused() {
                    let _ = self.queue.resume(id);
                }
            }
            KeyCode::Char('c') => {
                if let Some(id) = self.focused() {
                    let _ = self.queue.cancel(id);
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.focused() {
                    let _ = self.queue.narrow(id);
                }
            }
            KeyCode::Char('u') | KeyCode::Backspace => self.queue.unnarrow(),
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                if let Mode::Input(input) = &mut self.mode {
                    input.buffer.pop();
                }
            }
            KeyCode::Enter => self.commit_input(),
            KeyCode::Char(c) => {
                if let Mode::Input(input) = &mut self.mode {
                    input.buffer.push(c);
                }
            }
            _ => {}
        }
    }

    fn prompt(&mut self, kind: InputKind) {
        let buffer = match &kind {
            InputKind::EditHand => self.hand.as_ref().map(|r| r.title.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        self.mode = Mode::Input(Input { kind, buffer });
    }

    fn commit_input(&mut self) {
        let Mode::Input(input) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        let text = input.buffer;
        match input.kind {
            InputKind::Append => {
                let _ = self.queue.place(Request::task(text), Position::After);
            }
            InputKind::InsertNow => {
                let _ = self.queue.place(Request::task(text), Position::Now);
            }
            InputKind::Steer(id) => {
                let _ = self.queue.steer(id, text);
            }
            InputKind::Redirect(id) => {
                let _ = self.queue.redirect(id, text);
            }
            InputKind::EditHand => {
                if let Some(request) = &mut self.hand {
                    request.title = text;
                }
            }
            InputKind::Aside => {
                self.aside = Some(Aside::new(text));
            }
        }
    }

    fn place_hand(&mut self, position: Position) {
        if let Some(request) = self.hand.take() {
            let _ = self.queue.place(request, position);
        }
    }

    /// The aside line: the side question and its answer, or its asking state.
    /// `None` when no aside is held. The caller settles the aside through
    /// `App::aside` once the model answers; the surface only shows it.
    pub fn aside_line(&self) -> Option<String> {
        let aside = self.aside.as_ref()?;
        Some(match &aside.answer {
            Some(text) => format!("btw: {} -- {}", aside.question, text),
            None => format!("btw: {} (asking...)", aside.question),
        })
    }

    /// The footer prompt: what the human is typing, or what is in the hand.
    pub fn prompt_line(&self) -> String {
        match &self.mode {
            Mode::Input(input) => {
                let label = match &input.kind {
                    InputKind::Append => "append",
                    InputKind::InsertNow => "insert now",
                    InputKind::Steer(_) => "steer",
                    InputKind::Redirect(_) => "redirect",
                    InputKind::EditHand => "edit hand",
                    InputKind::Aside => "btw",
                };
                format!("{label}: {}_", input.buffer)
            }
            Mode::Normal => match &self.hand {
                Some(request) => {
                    format!("in hand: {}  [p]lace after  [P]lace now  [e]dit", request.title)
                }
                None => String::from(
                    "a append  i now  s steer  t take  h hold  r resume  b btw  c cancel  q quit",
                ),
            },
        }
    }
}

/// A convenience for building a key event in stories and the event loop.
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The form a steering request takes (re-exported for the surface's callers).
pub fn steering(title: impl Into<String>, target: TaskId) -> Request {
    Request { title: title.into(), form: Form::Steering, target: Some(target) }
}
