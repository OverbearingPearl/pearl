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
use pearl_core::{Aside, Form, Gauge, Keys, Position, Queue, QueueError, Request, Settings, TaskId};

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
    /// Nest a part under a task: the parts run first, then the whole.
    Nest(TaskId),
    /// Replace a task's objective.
    Redirect(TaskId),
    /// Edit the title of the request in the hand.
    EditHand,
    /// Ask a side question that enters neither the queue nor the log.
    Aside,
    /// Turn a knob that takes effect on the next request.
    Config,
}

/// Which pane holds the screen. The human is either watching what the model
/// is saying or reviewing what the queue holds, and the split is the resting
/// state between the two -- not a cage that makes both cramped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panes {
    #[default]
    Split,
    Report,
    Queue,
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
    /// Why the last key did nothing. A refused key that says nothing leaves the
    /// human guessing whether the tool is broken or they are.
    pub notice: Option<String>,
    /// The knobs, shared with the runner: an edit here is felt by the next
    /// request rather than by the next process.
    pub settings: Settings,
    /// Whose endpoint the key belongs to, when the source could say. An
    /// environment variable alone cannot; an authinfo machine name can.
    pub provider: Option<String>,
    /// What this process is holding, refreshed by the caller before a draw:
    /// the surface does not ask the OS, it shows what it was told.
    pub footprint: Option<String>,
    /// The ground the work stands on: directory, branch, sandbox. Refreshed
    /// by the caller; the surface shows what it was told.
    pub place: Option<String>,
    /// Where the work lands and what it is allowed to do there. The surface
    /// reads the ground each frame; the caller says which ground it is.
    pub workspace: std::path::PathBuf,
    pub policy: pearl_tools::sandbox::Policy,
    /// The human asked to take the last write back. The surface cannot restore
    /// files itself -- the runner owns the workspace -- so it asks, and the
    /// caller settles it through `settle_undo`.
    /// A half-typed chord: emacs `C-x`, vim `g`. The next key completes it.
    chord_x: bool,
    pending_g: bool,
    /// The whole legend on a screen of its own, for a window too narrow to
    /// carry it and for a human who wants to read it once.
    /// How many rows a page is in each window, told to the surface by the
    /// loop that knows the terminal's size.
    pub queue_page: usize,
    pub report_page: usize,
    pub help: bool,
    pub undo_requested: bool,
    /// The human asked to keep the knobs. One key saves all of them: a save
    /// per knob is a chore nobody performs, and the defaults then drift from
    /// what the screen says.
    pub save_requested: bool,
    /// Which pane has the screen.
    pub panes: Panes,
    /// How many lines the report is held back from its newest. Zero follows
    /// the answer as it arrives; a positive offset is the human reading back.
    pub report_back: usize,
    /// Whether the model's thinking is unfolded in the report. Folded by
    /// default: the answer is what the human came for.
    pub show_thinking: bool,
    /// What the window holds, shared with the runner.
    pub gauge: Gauge,
    pub quit: bool,
}

/// The name, as a single line of Unicode art. Plain letters when the window is
/// too narrow to spend columns on a wordmark.
pub fn wordmark() -> &'static str {
    "\u{1d5e3}\u{1d5d8}\u{1d5d4}\u{1d5e5}\u{1d5df}"
}

impl App {
    pub fn new(queue: Queue) -> Self {
        Self {
            queue,
            mode: Mode::Normal,
            hand: None,
            aside: None,
            notice: None,
            settings: Settings::new("deepseek-chat"),
            provider: None,
            footprint: None,
            place: None,
            workspace: std::path::PathBuf::from("."),
            policy: pearl_tools::sandbox::Policy::ReadOnly,
            chord_x: false,
            pending_g: false,
            queue_page: 10,
            report_page: 10,
            help: false,
            undo_requested: false,
            save_requested: false,
            panes: Panes::Split,
            report_back: 0,
            show_thinking: false,
            gauge: Gauge::new(),
            quit: false,
        }
    }

    /// Share the runner's reading of the window.
    pub fn with_gauge(mut self, gauge: Gauge) -> Self {
        self.gauge = gauge;
        self
    }

    /// Say where the work lands and under which sandbox, so the surface can
    /// name the ground rather than leave the human to guess it.
    pub fn with_place(
        mut self,
        root: impl Into<std::path::PathBuf>,
        policy: pearl_tools::sandbox::Policy,
    ) -> Self {
        self.workspace = root.into();
        self.policy = policy;
        self
    }

    /// Share the runner's configuration, so the surface edits the same knobs.
    /// Name the house the endpoint belongs to, for the status line.
    pub fn with_provider(mut self, name: impl Into<String>) -> Self {
        self.provider = Some(name.into());
        self
    }

    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Report the outcome of one action: keep what worked, say what did not.
    /// The queue's own reason is the message -- it names the state the task is
    /// in and the state the move needed, which is what the human has to know.
    fn did<T>(&mut self, verb: &str, outcome: Result<T, QueueError>) -> Option<T> {
        match outcome {
            Ok(value) => {
                self.notice = None;
                Some(value)
            }
            Err(why) => {
                self.notice = Some(format!("{verb}: {}", reason(&why)));
                None
            }
        }
    }

    /// Move to the next arrangement of the two panes.
    /// Give the screen to one window, or share it. There is no pane focus:
    /// every key already names its window, so a layout is a layout and not a
    /// mode the next keypress has to be interpreted against.
    pub fn set_panes(&mut self, panes: Panes) {
        self.panes = panes;
    }

    /// Scroll the report: negative reads back, positive returns to the newest.
    /// The offset is clamped at the newest line, so scrolling forward past the
    /// end resumes following rather than running off into blank space.
    pub fn scroll_report(&mut self, lines: isize) {
        if lines < 0 {
            self.report_back = self.report_back.saturating_add(lines.unsigned_abs());
        } else {
            self.report_back = self.report_back.saturating_sub(lines as usize);
        }
    }

    /// Open or fold the thinking of the focused task. A task that never
    /// thought says so, rather than leaving the key looking broken.
    pub fn toggle_thinking(&mut self) {
        let has = self
            .queue
            .focus()
            .and_then(|id| self.queue.get(id))
            .and_then(|n| n.reasoning)
            .is_some();
        if !has {
            self.notice = Some(String::from("no thinking recorded for this task"));
            return;
        }
        self.show_thinking = !self.show_thinking;
        self.report_back = 0;
    }

    /// Show where the knobs were kept, or why they were not.
    pub fn settle_save(&mut self, outcome: Result<String, String>) {
        self.save_requested = false;
        self.notice = Some(match outcome {
            Ok(path) => format!("saved defaults to {path}"),
            Err(why) => format!("save: {why}"),
        });
    }

    /// Show what the undo did, or why there was nothing to do.
    pub fn settle_undo(&mut self, outcome: Result<String, String>) {
        self.undo_requested = false;
        self.notice = Some(match outcome {
            Ok(said) if said.is_empty() => String::from("undo: nothing to take back"),
            Ok(said) => said,
            Err(why) => format!("undo: {why}"),
        });
    }

    /// The focused id, or a notice saying there is nothing to act on.
    fn target(&mut self, verb: &str) -> Option<TaskId> {
        match self.focused() {
            Some(id) => Some(id),
            None => {
                self.notice = Some(format!("{verb}: nothing is focused"));
                None
            }
        }
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
        if self.help {
            self.help = false;
            return;
        }
        if key.code == KeyCode::Char('?') && matches!(self.mode, Mode::Normal) {
            self.help = true;
            return;
        }
        match &mut self.mode {
            Mode::Input(_) => self.handle_input_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    /// Motion, in the keyboard the human chose. Returns true when the key was
    /// a motion, so the product's own letters are only reached afterwards.
    fn handle_motion(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let style = self.settings.keys();
        // One rule across every style: the plain key acts on the queue and Alt
        // acts on the report. The region is an argument of the keypress, not a
        // mode the surface remembers. What is universal here is only what all
        // three editors really share -- arrows and page keys. Letters belong to
        // the style that owns them: j/k in a plain legend is a vim key wearing
        // the wrong clothes.
        let qpage = self.queue_page as isize;
        let rpage = self.report_page as isize;
        match key.code {
            KeyCode::Char('1') => return self.take_layout(Panes::Queue),
            KeyCode::Char('2') => return self.take_layout(Panes::Report),
            KeyCode::Char('3') => return self.take_layout(Panes::Split),
            KeyCode::Down if !alt => return self.queue_line(1),
            KeyCode::Up if !alt => return self.queue_line(-1),
            KeyCode::PageDown if !alt => return self.queue_line(qpage),
            KeyCode::PageUp if !alt => return self.queue_line(-qpage),
            KeyCode::Down if alt => return self.report_line(1),
            KeyCode::Up if alt => return self.report_line(-1),
            KeyCode::PageDown if alt => return self.report_line(rpage),
            KeyCode::PageUp if alt => return self.report_line(-rpage),
            KeyCode::End if alt => {
                self.report_back = 0;
                return true;
            }
            _ => {}
        }
        match style {
            Keys::Plain => match key.code {
                KeyCode::Right => self.drill(),
                KeyCode::Left => {
                    self.queue.unnarrow();
                }
                _ => return false,
            },
            Keys::Vim => match key.code {
                KeyCode::Char('j') if !alt => return self.queue_line(1),
                KeyCode::Char('k') if !alt => return self.queue_line(-1),
                KeyCode::Char('J') if !alt => return self.queue_line(qpage),
                KeyCode::Char('K') if !alt => return self.queue_line(-qpage),
                KeyCode::Char('j') if alt => return self.report_line(1),
                KeyCode::Char('k') if alt => return self.report_line(-1),
                KeyCode::Char('J') if alt => return self.report_line(rpage),
                KeyCode::Char('K') if alt => return self.report_line(-rpage),
                KeyCode::Char('G') if alt => {
                    self.report_back = 0;
                    return true;
                }
                KeyCode::Char('h') => {
                    self.queue.unnarrow();
                }
                KeyCode::Char('l') => self.drill(),
                // vim scrolls the window that holds the cursor, and the cursor
                // is carried with it -- which is what queue_line does.
                KeyCode::Char('d') if ctrl && !alt => return self.queue_line(qpage / 2),
                KeyCode::Char('u') if ctrl && !alt => return self.queue_line(-qpage / 2),
                KeyCode::Char('f') if ctrl && !alt => return self.queue_line(qpage),
                KeyCode::Char('b') if ctrl && !alt => return self.queue_line(-qpage),
                KeyCode::Char('d') if ctrl && alt => return self.report_line(rpage / 2),
                KeyCode::Char('u') if ctrl && alt => return self.report_line(-rpage / 2),
                KeyCode::Char('f') if ctrl && alt => return self.report_line(rpage),
                KeyCode::Char('b') if ctrl && alt => return self.report_line(-rpage),
                KeyCode::Char('G') => return self.queue_line(isize::MAX / 2),
                KeyCode::Char('g') => {
                    if self.pending_g {
                        self.pending_g = false;
                        return self.queue_line(isize::MIN / 2);
                    }
                    self.pending_g = true;
                    return true;
                }
                _ => return false,
            },
            Keys::Emacs => match key.code {
                KeyCode::Char('n') if ctrl => return self.queue_line(1),
                KeyCode::Char('p') if ctrl => return self.queue_line(-1),
                KeyCode::Char('v') if ctrl && !alt => return self.queue_line(qpage),
                KeyCode::Char('v') if alt && !ctrl => return self.queue_line(-qpage),
                // scroll-other-window: emacs already has the report's page as
                // a command of its own rather than a focus to switch first.
                KeyCode::Char('v') if ctrl && alt => return self.report_line(rpage),
                KeyCode::Char('V') if ctrl && alt => return self.report_line(-rpage),
                KeyCode::Char('n') if alt => return self.report_line(1),
                KeyCode::Char('p') if alt => return self.report_line(-1),
                KeyCode::Char('<') if alt => return self.queue_line(isize::MIN / 2),
                KeyCode::Char('>') if alt => return self.queue_line(isize::MAX / 2),
                KeyCode::Char('l') if ctrl => {
                    self.report_back = 0;
                }
                KeyCode::Char('g') if ctrl => {
                    self.aside = None;
                    self.notice = None;
                }
                KeyCode::Char('^') => {
                    self.queue.unnarrow();
                }
                KeyCode::Char('x') if ctrl => {
                    self.chord_x = true;
                    return true;
                }
                KeyCode::Char('1') if self.chord_x => {
                    self.chord_x = false;
                    self.set_panes(Panes::Queue);
                }
                KeyCode::Char('2') if self.chord_x => {
                    self.chord_x = false;
                    self.set_panes(Panes::Report);
                }
                KeyCode::Char('3') if self.chord_x => {
                    self.chord_x = false;
                    self.set_panes(Panes::Split);
                }
                _ => return false,
            },
        }
        self.pending_g = false;
        self.chord_x = false;
        true
    }

    fn take_layout(&mut self, panes: Panes) -> bool {
        self.set_panes(panes);
        true
    }

    /// Move the queue's cursor. The window follows the cursor, so a page
    /// carries it along and it can never end up outside what is on screen.
    fn queue_line(&mut self, lines: isize) -> bool {
        self.move_focus(lines);
        self.report_back = 0;
        true
    }

    /// Move the report. Negative goes back in history, positive comes forward
    /// toward the newest line; at the newest there is nothing further forward.
    fn report_line(&mut self, lines: isize) -> bool {
        self.scroll_report(lines);
        true
    }

    /// Enter the focused task's level, saying so when it has none.
    fn drill(&mut self) {
        if let Some(id) = self.target("drill in") {
            let outcome = self.queue.narrow(id);
            self.did("drill in", outcome);
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if self.handle_motion(key) {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Char('b') => self.prompt(InputKind::Aside),
            KeyCode::Char('z') => self.undo_requested = true,
            KeyCode::Char('S') => self.save_requested = true,
            KeyCode::Char('M') => self.settings.cycle_model(),
            KeyCode::Char('E') => self.settings.cycle_effort(),
            KeyCode::Char('W') => self.settings.cycle_window(),
            KeyCode::Char('T') => self.toggle_thinking(),
            KeyCode::Char('\\') => self.settings.cycle_keys(),
            KeyCode::Char(':') => self.prompt(InputKind::Config),
            KeyCode::Esc => {
                self.aside = None;
                self.notice = None;
            }
            KeyCode::Char('a') => self.prompt(InputKind::Append),
            KeyCode::Char('i') => self.prompt(InputKind::InsertNow),
            KeyCode::Char('s') => {
                if let Some(id) = self.target("steer") {
                    self.prompt(InputKind::Steer(id));
                }
            }
            KeyCode::Char('n') => {
                if let Some(id) = self.target("nest") {
                    self.prompt(InputKind::Nest(id));
                }
            }
            KeyCode::Char('R') => {
                if let Some(id) = self.target("redirect") {
                    self.prompt(InputKind::Redirect(id));
                }
            }
            KeyCode::Char('t') => {
                if let Some(id) = self.target("take") {
                    let outcome = self.queue.take(id);
                    if let Some((request, _)) = self.did("take", outcome) {
                        self.hand = Some(request);
                    }
                }
            }
            KeyCode::Char('e') => {
                if self.hand.is_some() {
                    self.prompt(InputKind::EditHand);
                } else {
                    self.notice = Some(String::from("edit: the hand is empty; take a task first"));
                }
            }
            KeyCode::Char('p') => self.place_hand(Position::After),
            KeyCode::Char('P') => self.place_hand(Position::Now),
            KeyCode::Char('h') | KeyCode::Char('H') => {
                if let Some(id) = self.target("hold") {
                    let outcome = self.queue.hold(id, "");
                    self.did("hold", outcome);
                }
            }
            KeyCode::Char('r') => {
                if let Some(id) = self.target("resume") {
                    let outcome = self.queue.resume(id);
                    self.did("resume", outcome);
                }
            }
            KeyCode::Char('c') => {
                if let Some(id) = self.target("cancel") {
                    let outcome = self.queue.cancel(id);
                    self.did("cancel", outcome);
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.target("drill in") {
                    let outcome = self.queue.narrow(id);
                    self.did("drill in", outcome);
                }
            }
            KeyCode::Char('u') | KeyCode::Backspace => self.queue.unnarrow(),
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        // Emacs quits whatever is pending with C-g, and a prompt is the thing
        // most often pending. Esc keeps working in every style.
        let emacs_quit = self.settings.keys() == Keys::Emacs
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('g');
        if emacs_quit {
            self.mode = Mode::Normal;
            return;
        }
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
                // Append means the end of the queue. Placing after the focus
                // put each new task beside the cursor, so the list read in a
                // different order from the one the human typed.
                let outcome = self.queue.place(Request::task(text), Position::Last);
                self.did("append", outcome);
            }
            InputKind::InsertNow => {
                let outcome = self.queue.place(Request::task(text), Position::Now);
                self.did("insert", outcome);
            }
            InputKind::Steer(id) => {
                let outcome = self.queue.steer(id, text);
                self.did("steer", outcome);
            }
            InputKind::Nest(id) => {
                let outcome = self.queue.add_child(id, text).map(|(_, events)| events);
                self.did("nest", outcome);
            }
            InputKind::Redirect(id) => {
                let outcome = self.queue.redirect(id, text);
                self.did("redirect", outcome);
            }
            InputKind::EditHand => {
                if let Some(request) = &mut self.hand {
                    request.title = text;
                }
            }
            InputKind::Aside => {
                self.aside = Some(Aside::new(text));
            }
            InputKind::Config => match self.settings.apply(&text) {
                Ok(said) => self.notice = Some(said),
                Err(why) => self.notice = Some(why),
            },
        }
    }

    fn place_hand(&mut self, position: Position) {
        let Some(request) = self.hand.take() else {
            self.notice = Some(String::from("place: the hand is empty; take a task first"));
            return;
        };
        let outcome = self.queue.place(request.clone(), position);
        if self.did("place", outcome).is_none() {
            // A refused placement must not swallow the request: the human
            // edited it, and it is still theirs to place somewhere else.
            self.hand = Some(request);
        }
    }

    /// What is in force, in one line. A knob nobody can read is a knob nobody
    /// trusts: the human has to see the model and the window they are on.
    /// The status row as whole items, most important first, so a narrow
    /// window drops readings rather than cutting them in half.
    /// The one header line: the name, what it is talking to, where it stands,
    /// and what it has cost. Most important first, so a narrow window drops
    /// readings rather than cutting them.
    pub fn header_items(&self) -> Vec<String> {
        let mut items = vec![wordmark().to_string()];
        let s = self.settings_items();
        // model, ctx, think lead; the keyboard, memory and bill follow the
        // ground, because where the writes land matters more than what they
        // cost.
        items.extend(s.iter().take(3).cloned());
        items.extend(self.place_items());
        items.extend(s.iter().skip(3).cloned());
        items
    }

    /// The keys in the order a hand reaches for them, grouped by what the hand
    /// is doing. One order serves both the row and help: a legend read in a
    /// different order than it is written is two things to learn.
    ///
    /// The knobs M, E and W are not here. The header already prints each with
    /// its key beside the reading it changes, which is a better place to learn
    /// it than a list of letters.
    pub fn key_groups(&self) -> Vec<(&'static str, Vec<String>)> {
        let hold = if self.settings.keys() == Keys::Vim { "H hold" } else { "h hold" };
        let mut look = self.motion_items();
        look.push(String::from("1/2/3 queue/report/both"));
        vec![
            ("start", vec![String::from("a add")]),
            ("look", look),
            ("work", ["i now", "t take", "s steer", "n nest", hold, "r resume"]
                .iter().map(|s| s.to_string()).collect()),
            ("safety", vec![String::from("z undo")]),
            ("ask", vec![String::from("b btw"), String::from("T think")]),
            ("set", vec![String::from("S save"), String::from(": set")]),
        ]
    }

    /// Whether the bottom row belongs to the human's typing rather than to
    /// the legend: an open prompt, or a request held in the hand.
    pub fn is_prompting(&self) -> bool {
        matches!(self.mode, Mode::Input(_)) || self.hand.is_some()
    }

    /// The single line of keys: the groups flattened in their own order.
    pub fn key_items(&self) -> Vec<String> {
        self.key_groups().into_iter().flat_map(|(_, items)| items).collect()
    }

    pub fn settings_items(&self) -> Vec<String> {
        let effort = self.settings.effort().map(|e| e.as_str()).unwrap_or("auto");
        let model = match &self.provider {
            Some(house) => format!("{house}/{}", self.settings.model()),
            None => self.settings.model(),
        };
        let mut items = vec![
            format!("{model} (M)"),
            format!("think {effort} (E)"),
            format!("{} (W)", self.gauge.describe(self.settings.window())),
            format!("{} (\\)", self.settings.keys().as_str()),
        ];
        if let Some(mem) = &self.footprint {
            items.push(format!("mem {mem}"));
        }
        let session = self.gauge.cost();
        if !session.is_empty() {
            items.push(session.describe());
        }
        items
    }

    /// The motion legend as whole items, each naming the window it acts on.
    pub fn motion_items(&self) -> Vec<String> {
        self.motion_line().split("  ").map(String::from).collect()
    }

    /// The action legend without the motions: what the hands do to the work.
    pub fn action_items(&self) -> Vec<String> {
        self.key_groups()
            .into_iter()
            .filter(|(name, _)| *name != "look")
            .flat_map(|(_, items)| items)
            .collect()
    }

    /// The ground row as whole items: where, under what, on which branch.
    pub fn place_items(&self) -> Vec<String> {
        self.place
            .as_deref()
            .unwrap_or("")
            .split("  ")
            .map(String::from)
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn settings_line(&self) -> String {
        let effort = self.settings.effort().map(|e| e.as_str()).unwrap_or("auto");
        let model = match &self.provider {
            Some(house) => format!("{house}/{}", self.settings.model()),
            None => self.settings.model(),
        };
        let mem = self.footprint.as_deref().unwrap_or("-");
        let session = self.gauge.cost();
        let total =
            if session.is_empty() { String::new() } else { format!(" | {}", session.describe()) };
        format!(
            "{model} (M) | think {effort} (E) | {} (W) | {} (\\) | mem {mem}{total}",
            self.gauge.describe(self.settings.window()),
            self.settings.keys().as_str()
        )
    }

    /// The motions of the chosen keyboard. A style whose keys are not written
    /// down is a style nobody can use, so this row changes with the style.
    pub fn motion_line(&self) -> String {
        String::from(match self.settings.keys() {
            Keys::Plain => "queue ↑↓  page PgUp/Dn  level ←→  \
                            report Alt+↑↓  page Alt+PgUp/Dn",
            Keys::Vim => "queue j/k  page C-f/C-b  level h/l  ends gg/G  report M-j/k  \
                          page M-C-f/C-b",
            Keys::Emacs => "queue C-n/p  page C-v/M-v  level ^/RET  ends M-</M->  \
                            report M-n/p  page C-M-v",
        })
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
                    InputKind::Nest(id) => {
                        let parent = self
                            .queue
                            .get(*id)
                            .map(|n| n.title)
                            .unwrap_or_else(|| String::from("this task"));
                        // Say what nesting does, not just where it goes: the
                        // part runs first and the whole reads what it found.
                        return format!("part of \"{parent}\" (runs before it): {}", input.buffer);
                    }
                    InputKind::Redirect(_) => "redirect",
                    InputKind::EditHand => "edit hand",
                    InputKind::Aside => "btw",
                    InputKind::Config => "set",
                };
                format!("{label}: {}_", input.buffer)
            }
            Mode::Normal => match &self.hand {
                Some(request) => {
                    format!("in hand: {}  [p]lace after  [P]lace now  [e]dit", request.title)
                }
                None => format!("{}  ? keys  q quit", self.key_items().join("  ")),
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

/// The refusal in the words the human uses. An id is a handle for the program;
/// what the person needs is the state the task is in and the state the move
/// wanted, both as words they can match against the badge on the screen.
fn reason(why: &QueueError) -> String {
    match why {
        QueueError::WrongState { found, expected, .. } => {
            format!("this task is {}, needs {expected}", state_word(*found))
        }
        QueueError::Terminal(_) => String::from("this task already settled; it cannot change"),
        QueueError::Unknown(_) => String::from("no such task"),
        QueueError::SteeringNeedsTarget => String::from("steering needs a task to aim at"),
    }
}

fn state_word(state: pearl_core::NodeState) -> &'static str {
    use pearl_core::NodeState::*;
    match state {
        Queued => "queued",
        Running => "running",
        AwaitingInput => "waiting for an answer",
        Paused => "paused",
        Done => "done",
        Failed => "failed",
        Cancelled => "cancelled",
    }
}
