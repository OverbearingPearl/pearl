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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use pearl_core::{
    Aside, Effort, Form, Gauge, Keys, Position, Queue, QueueError, Request, Settings, TaskId,
};

/// The setting a list is choosing. One rule for all three: lowercase steps,
/// uppercase chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Model,
    Effort,
    Window,
    /// The addresses a candidate row can point at, for the row at this index
    /// or for a new one. A candidate is two fields, and the address is picked
    /// from what is already in use before an id is typed against it -- an
    /// endpoint is the half a human reuses.
    Endpoint(Option<usize>),
}

/// Which language the key screen speaks. Two, because the screen is the one
/// place a name cannot be guessed from context -- and because the person who
/// asked for it works in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Zh,
}

impl Lang {
    pub fn header(self) -> &'static str {
        match self {
            Lang::En => "keys  (Tab Eng/中, ↑↓ scroll, any other key closes)",
            Lang::Zh => "按键  (Tab Eng/中, ↑↓ 滚动, 其它键关闭)",
        }
    }

    pub fn readings(self) -> &'static str {
        match self {
            Lang::En => "readings",
            Lang::Zh => "读数",
        }
    }
}

/// One line of the key screen: the keys, and what they do in both languages.
pub struct HelpRow {
    pub keys: String,
    pub en: String,
    pub zh: String,
}

impl HelpRow {
    pub fn text(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Zh => &self.zh,
        }
    }
}

/// A question the surface asks the provider layer. A closure in a struct that
/// is copied and printed needs a name to print as.
#[derive(Clone)]
struct KeyProbe(std::sync::Arc<dyn Fn(&str) -> bool + Send + Sync>);

impl std::fmt::Debug for KeyProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeyProbe")
    }
}

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
    /// The address side of a candidate row, typed by hand because the list
    /// that offers the others has no row for this one.
    Endpoint(Option<usize>),
    /// The id side of a candidate row, against an address already chosen.
    /// An empty address is the launch endpoint, which the grammar calls `-`.
    ModelOf { row: Option<usize>, endpoint: String },
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
    /// The drag the mouse is making, while the left button is down.
    pub drag: Option<crate::select::Drag>,
    /// Which way the pointer is held past the edge of its window: -1 at the
    /// top, 1 at the bottom, `None` while it is over the window. The window
    /// scrolls on the loop's tick, because a pointer resting still sends no
    /// events and the scroll has to keep happening anyway.
    pub auto: Option<i8>,
    /// How far a drag has pulled the queue's window away from the cursor. It
    /// dies with the drag: every other act leaves the window following the
    /// cursor, which is what the list promises.
    pub queue_shift: isize,
    /// The address pearl itself sends to. A row may name it, and the list
    /// shows it as the launch endpoint rather than twice.
    pub launch: String,
    /// Where the panes were drawn last frame, so the mouse can be told which
    /// window it is over: the pointer arrives in terminal coordinates and the
    /// panes are a division of the screen with furniture above and below.
    pub area: std::cell::Cell<ratatui::layout::Rect>,
    /// Set when the button came up, so the frame that follows hands the
    /// selected lines to the clipboard: the text lives in the frame, not in
    /// the queue, so it can only be taken where it is drawn.
    pub copy_pending: bool,
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
    pending_g: Option<bool>,
    /// The whole legend on a screen of its own, for a window too narrow to
    /// carry it and for a human who wants to read it once.
    /// How many rows a page is in each window, told to the surface by the
    /// loop that knows the terminal's size.
    pub queue_page: usize,
    pub report_page: usize,
    pub help: bool,
    /// The row the model list is highlighting, when it is open. The rows
    /// themselves are read from the settings each time, so an edit made from
    /// the list is in the list without a second copy to keep in step.
    pub picker: Option<usize>,
    /// Which setting the open list is choosing. The rows, the legend and what
    /// Enter does all follow from it; the movement keys do not.
    pub pick_kind: Pick,
    /// How far the key screen is scrolled. It is taller than a terminal now
    /// that every key carries its sentence, so it scrolls like the rest.
    pub help_at: usize,
    /// The language the key screen is written in. A reading choice, not a
    /// keystroke: it survives scrolling and closes with the screen.
    pub help_lang: Lang,
    /// Whether a key can be found for an endpoint. Injected because finding
    /// one is the provider's business, not the surface's.
    key_probe: Option<KeyProbe>,
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
    /// How many lines the report held when it was last drawn. The far end of
    /// a scrollback is a place only the view can measure, so the view leaves
    /// the measurement here and the next motion reads it.
    pub report_len: std::cell::Cell<usize>,
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
            launch: String::new(),
            drag: None,
            auto: None,
            queue_shift: 0,
            area: std::cell::Cell::new(ratatui::layout::Rect::new(0, 1, 80, 22)),
            copy_pending: false,
            settings: Settings::new("deepseek-chat"),
            provider: None,
            picker: None,
            pick_kind: Pick::Model,
            help_at: 0,
            help_lang: Lang::En,
            key_probe: None,
            footprint: None,
            place: None,
            workspace: std::path::PathBuf::from("."),
            policy: pearl_tools::sandbox::Policy::ReadOnly,
            chord_x: false,
            pending_g: None,
            queue_page: 10,
            report_page: 10,
            help: false,
            undo_requested: false,
            save_requested: false,
            panes: Panes::Split,
            report_back: 0,
            report_len: std::cell::Cell::new(0),
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
    /// Ask this before claiming a row has a key. Without one the surface says
    /// nothing about keys rather than guessing.
    pub fn with_key_probe(mut self, probe: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        self.key_probe = Some(KeyProbe(std::sync::Arc::new(probe)));
        self
    }

    pub fn with_provider(mut self, name: impl Into<String>) -> Self {
        self.provider = Some(name.into());
        self
    }

    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// The address pearl itself sends to, so the list can offer it once: as
    /// the launch endpoint, not again under its URL.
    pub fn with_launch(mut self, endpoint: impl Into<String>) -> Self {
        self.launch = endpoint.into();
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
        if self.report_back == usize::MAX {
            // Standing at the far end, the distance back is whatever the last
            // drawing measured; a page forward from there is a real page.
            self.report_back = self.report_len.get();
        }
        if lines < 0 {
            self.report_back = self.report_back.saturating_add(lines.unsigned_abs());
        } else {
            self.report_back = self.report_back.saturating_sub(lines as usize);
        }
    }

    /// The oldest line of the report. The count is not known here -- wrapping
    /// decides it -- so this asks for the far end and the view clamps it.
    pub fn report_oldest(&mut self) {
        self.report_back = usize::MAX;
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
    /// The mouse, taken from the terminal so that a drag stays inside the
    /// window. Only the left button is ours: every other button and every
    /// scroll is the human's terminal doing what it already does.
    pub fn handle_mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.drag = self
                    .document_at(row)
                    .map(|(pane, at)| crate::select::Drag::start(pane, at, column));
            }
            MouseEventKind::Drag(MouseButton::Left) => self.drag_to(row),
            MouseEventKind::Up(MouseButton::Left) => self.copy_pending = true,
            _ => {}
        }
    }

    /// Which document row a terminal row is over, and which window it belongs
    /// to. The heading rule between two panes belongs to neither: a press
    /// there names nothing.
    pub fn document_at(&self, row: u16) -> Option<(crate::select::Pane, usize)> {
        let area = self.area.get();
        if row < area.top() || row >= area.bottom() {
            return None;
        }
        let row = row - area.top();
        let width = area.width;
        let height = area.height;
        let (queue_rows, _) = crate::view::pane_rows(self, height);
        // The list's window starts at the top of the screen and the report's
        // below it, each under a heading whose rule takes the last row.
        if row + 1 < queue_rows {
            let len = self.queue.view().len().max(1);
            let start = crate::view::list_offset(&self.queue, (queue_rows - 1) as usize, self.queue_shift);
            return Some((crate::select::Pane::Queue, (start + row as usize).min(len - 1)));
        }
        if row > queue_rows {
            let len = crate::view::report_lines(self, width as usize).len().max(1);
            let inner = (height - queue_rows).saturating_sub(1);
            let start = crate::view::report_offset(self, inner as usize, len);
            let at = start + (row - queue_rows) as usize;
            return Some((crate::select::Pane::Report, at.min(len - 1)));
        }
        None
    }

    /// The window a drag is working in, as the rows it can show: the top row
    /// of the document its pane has on screen, the bottom row of it, how long
    /// the document is, and which row the window starts at.
    fn pane_bounds(&self, pane: crate::select::Pane) -> (u16, u16, usize, usize) {
        let area = self.area.get();
        let width = area.width;
        let height = area.height;
        let (queue_rows, _) = crate::view::pane_rows(self, height);
        match pane {
            crate::select::Pane::Queue => {
                let len = self.queue.view().len().max(1);
                let inner = (queue_rows - 1) as usize;
                let start = crate::view::list_offset(&self.queue, inner, self.queue_shift);
                (area.top(), area.top() + queue_rows.saturating_sub(2), len, start)
            }
            crate::select::Pane::Report => {
                let len = crate::view::report_lines(self, width as usize).len().max(1);
                let inner = (height - queue_rows).saturating_sub(1) as usize;
                let start = crate::view::report_offset(self, inner, len);
                (area.top() + queue_rows, area.top() + height.saturating_sub(2), len, start)
            }
        }
    }

    /// The terminal row a document row is drawn at, when its pane is showing
    /// it -- which is what the mark needs to paint the lines a drag covers.
    pub fn row_of(&self, pane: crate::select::Pane, at: usize) -> Option<u16> {
        let (top, bottom, len, start) = self.pane_bounds(pane);
        if at < start || at >= start + (bottom.saturating_sub(top) as usize) + 1 || at >= len {
            return None;
        }
        Some(top + (at - start) as u16)
    }

    /// Move the pointer, and say which way it is now held past its window.
    fn drag_to(&mut self, row: u16) {
        let Some(pane) = self.drag.map(|d| d.pane) else {
            return;
        };
        let (top, bottom, len, start) = self.pane_bounds(pane);
        let (held, at) = if row < top {
            (-1, start)
        } else if row > bottom {
            (1, (start + bottom.saturating_sub(top) as usize).min(len - 1))
        } else {
            (0, (start + (row - top) as usize).min(len - 1))
        };
        self.auto = match held {
            -1 => Some(-1),
            1 => Some(1),
            _ => None,
        };
        if let Some(drag) = &mut self.drag {
            drag.cursor = at;
        }
    }

    /// One tick of a drag held past the edge of its window: the window moves a
    /// row and the selection's loose corner goes with it. Called from the loop
    /// on its timer, not on an event, because a pointer held still sends none.
    pub fn auto_scroll(&mut self) {
        let Some(pane) = self.drag.map(|d| d.pane) else {
            return;
        };
        let Some(direction) = self.auto else {
            return;
        };
        match pane {
            crate::select::Pane::Report => self.scroll_report(direction as isize),
            crate::select::Pane::Queue => {
                self.queue_shift = (self.queue_shift + direction as isize).clamp(-4096, 4096);
            }
        }
        // The corner follows the edge it is held at, so the take grows by the
        // lines the scroll just revealed.
        let (top, bottom, len, start) = self.pane_bounds(pane);
        let at = if direction < 0 {
            start
        } else {
            (start + bottom.saturating_sub(top) as usize).min(len - 1)
        };
        if let Some(drag) = &mut self.drag {
            drag.cursor = at;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // A selection is a mark the mouse made, and the keyboard is what ends
        // it -- the same rule a notice follows.
        self.drag = None;
        self.auto = None;
        if self.picker.is_some() && matches!(self.mode, Mode::Normal) {
            self.handle_picker_key(key);
            return;
        }
        if self.help {
            // The screen is taller than the terminal, so the moving keys move
            // through it; anything else is the human done reading.
            if !self.scroll_help(key) {
                self.help = false;
                self.help_at = 0;
            }
            return;
        }
        if key.code == KeyCode::Char('?') && matches!(self.mode, Mode::Normal) {
            self.help = true;
            self.help_at = 0;
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
            KeyCode::Home if alt => {
                self.report_oldest();
                return true;
            }
            KeyCode::Home => return self.queue_line(isize::MIN / 2),
            KeyCode::End => return self.queue_line(isize::MAX / 2),
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
                // Bare, not under Ctrl: the row has to name the key a hand
                // presses, and C-f/C-b would have been a second name for the
                // same motion that the legend had no room for.
                KeyCode::Char('f') if !alt && !ctrl => return self.queue_line(qpage),
                KeyCode::Char('b') if !alt && !ctrl => return self.queue_line(-qpage),
                KeyCode::Char('d') if ctrl && alt => return self.report_line(rpage / 2),
                KeyCode::Char('u') if ctrl && alt => return self.report_line(-rpage / 2),
                KeyCode::Char('f') if alt => return self.report_line(rpage),
                KeyCode::Char('b') if alt => return self.report_line(-rpage),
                KeyCode::Char('G') => return self.queue_line(isize::MAX / 2),
                KeyCode::Char('g') => {
                    if self.pending_g == Some(alt) {
                        self.pending_g = None;
                        if alt {
                            self.report_oldest();
                            return true;
                        }
                        return self.queue_line(isize::MIN / 2);
                    }
                    self.pending_g = Some(alt);
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
                KeyCode::Char('<') if alt && ctrl => {
                    self.report_oldest();
                }
                KeyCode::Char('>') if alt && ctrl => {
                    self.report_back = 0;
                }
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
        self.pending_g = None;
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
        // A remark is the answer to the last act, so the next key ends it. A
        // line that waits for a particular key to clear it says the wrong
        // thing about everything that happened in between.
        self.notice = None;
        if self.handle_motion(key) {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Char('/') => self.prompt(InputKind::Aside),
            KeyCode::Char('z') => self.undo_requested = true,
            KeyCode::Char('S') => self.save_requested = true,
            KeyCode::Char('m') => self.settings.cycle_model(),
            KeyCode::Char('M') => self.open_picker(Pick::Model),
            KeyCode::Char('E') => self.open_picker(Pick::Effort),
            KeyCode::Char('w') => self.settings.cycle_window(),
            KeyCode::Char('W') => self.open_picker(Pick::Window),
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
                // Holding a task, the hand is what e edits. With nothing in
                // hand the letter used to refuse; it steps the effort now,
                // which is the same rule the other two settings follow.
                if self.hand.is_some() {
                    self.prompt(InputKind::EditHand);
                } else {
                    self.settings.cycle_effort();
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
            self.notice = None;
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                // The remark belonged to a stage that is now abandoned.
                self.notice = None;
            }
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

    /// The rows of the model list, each saying whether a key was found. A row
    /// that cannot be paid for is still choosable -- a local endpoint needs no
    /// key -- but it must not look like one that can.
    /// The values the open list offers, in the order the stepping key walks
    /// them. A list that offered a different set from the cycle would make the
    /// two keys disagree about what the setting can be.
    pub fn picker_rows(&self) -> Vec<String> {
        match self.pick_kind {
            Pick::Effort => {
                // A rung the endpoint refused keeps its row and gains a word.
                // Deleting it would hide the one thing this screen knows that
                // the human does not: why the word they want is not in force.
                let refused = self.settings.refused();
                let dialect = self.settings.dialect();
                let mut rows = vec![String::from("auto")];
                for rung in Effort::ladder() {
                    let mut row = rung.as_str().to_string();
                    if refused.contains(rung) {
                        row.push_str("   (this endpoint refused it)");
                    } else if !dialect.says(*rung) {
                        row.push_str("   (this endpoint has no setting for this strength)");
                    }
                    rows.push(row);
                }
                rows
            }
            Pick::Window => Settings::windows().iter().map(|w| format!("{}k", w / 1000)).collect(),
            Pick::Model => self.model_rows(),
            Pick::Endpoint(_) => {
                // The rows are what the eye reads, so the empty address is
                // spelled out here and read back as empty on Enter.
                let mut rows: Vec<String> =
                    self.endpoint_choices().iter().map(|e| self.endpoint_label(e)).collect();
                rows.push(String::from("an endpoint not listed here"));
                rows
            }
        }
    }

    /// The addresses on offer, in the order they are shown: every address a
    /// row already names, then the launch endpoint, then the ones the major
    /// houses publish. The last row of the list is the way to an address that
    /// is none of these, so this is not the whole list.
    ///
    /// An address the config already names is not offered twice, and the
    /// launch endpoint keeps its own row: a suggestion is a place requests can
    /// go, and a place already on the list is not a second place.
    fn endpoint_choices(&self) -> Vec<String> {
        let mut rows: Vec<String> = Vec::new();
        for c in self.settings.candidates() {
            if c.endpoint.is_empty() || rows.contains(&c.endpoint) {
                continue;
            }
            rows.push(c.endpoint.clone());
        }
        rows.push(String::new());
        // The launch endpoint answers to the first row: offering the address it
        // sends to a second time, under its URL, would be two rows for one
        // place -- and the eye would have no way to know they are the same.
        let launch = pearl_core::catalog::normalize_endpoint(&self.launch);
        for address in pearl_core::catalog::WELL_KNOWN {
            let address = address.to_string();
            if !rows.contains(&address) && address != launch {
                rows.push(address);
            }
        }
        rows
    }

    /// What an endpoint row shows. The empty address is the one the process
    /// was launched against, which `-` names in the grammar and a sentence
    /// names on screen.
    fn endpoint_label(&self, endpoint: &str) -> String {
        if !endpoint.is_empty() {
            return endpoint.to_string();
        }
        // Which address the launch endpoint is, when this surface was told: a
        // row that says only "the launch" makes the human guess.
        match pearl_core::catalog::normalize_endpoint(&self.launch).is_empty() {
            true => String::from("-  the launch endpoint"),
            false => format!(
                "-  the launch endpoint ({})",
                pearl_core::catalog::normalize_endpoint(&self.launch)
            ),
        }
    }

    /// Where the open list starts: on the value in force, so choosing nothing
    /// and pressing Enter is not a change.
    fn picker_start(&self) -> usize {
        match self.pick_kind {
            Pick::Model => self
                .settings
                .candidates()
                .iter()
                .position(|c| c.model == self.settings.model())
                .unwrap_or(0),
            Pick::Effort => {
                let word = self.settings.effort().map(|e| e.as_str()).unwrap_or("auto");
                // The row may carry its reason after the word, so the word is
                // what is compared, not the line.
                self.picker_rows()
                    .iter()
                    .position(|r| r.split_whitespace().next() == Some(word))
                    .unwrap_or(0)
            }
            Pick::Window => {
                let now = self.settings.window();
                Settings::windows().iter().position(|w| *w == now).unwrap_or(0)
            }
            Pick::Endpoint(row) => {
                let now = row
                    .and_then(|n| self.settings.candidates().get(n).map(|c| c.endpoint.clone()))
                    .unwrap_or_default();
                self.endpoint_choices().iter().position(|e| *e == now).unwrap_or(0)
            }
        }
    }

    /// Open the values of one setting, on the value already in force.
    fn open_picker(&mut self, kind: Pick) {
        self.pick_kind = kind;
        self.picker = Some(self.picker_start());
    }

    fn model_rows(&self) -> Vec<String> {
        self.settings
            .candidates()
            .iter()
            .map(|c| {
                let found = match (&self.key_probe, c.endpoint.is_empty()) {
                    (Some(probe), false) => (probe.0)(&c.endpoint),
                    _ => true,
                };
                if found { c.label() } else { format!("{}  (no key)", c.label()) }
            })
            .collect()
    }

    /// Keys while the model list is open. The rows are the product's own
    /// letters, so they mean the same thing here as anywhere else.
    fn handle_picker_key(&mut self, key: KeyEvent) {
        let rows = self.picker_rows().len();
        let at = self.picker.unwrap_or(0);
        let style = self.settings.keys();
        let up = matches!(key.code, KeyCode::Up)
            || (style == Keys::Vim && key.code == KeyCode::Char('k'))
            || (style == Keys::Emacs
                && key.code == KeyCode::Char('p')
                && key.modifiers.contains(KeyModifiers::CONTROL));
        let down = matches!(key.code, KeyCode::Down)
            || (style == Keys::Vim && key.code == KeyCode::Char('j'))
            || (style == Keys::Emacs
                && key.code == KeyCode::Char('n')
                && key.modifiers.contains(KeyModifiers::CONTROL));
        self.notice = None;
        match key.code {
            _ if up => self.picker = Some(at.saturating_sub(1)),
            _ if down => self.picker = Some((at + 1).min(rows - 1)),
            KeyCode::Enter => {
                let said = match self.pick_kind {
                    Pick::Model => self.settings.choose(at + 1).map(|_| String::new()),
                    Pick::Effort => {
                        let row = self.picker_rows().get(at).cloned().unwrap_or_default();
                        let word = row.split_whitespace().next().unwrap_or("auto");
                        self.settings.apply(&format!("effort {word}"))
                    }
                    Pick::Window => {
                        let size = Settings::windows().get(at).copied().unwrap_or(0);
                        self.settings.apply(&format!("window {size}"))
                    }
                    Pick::Endpoint(row) => {
                        // Choosing an address moves to the id, and the last
                        // row is the one address the list cannot name.
                        let choices = self.endpoint_choices();
                        self.picker = None;
                        return match choices.get(at) {
                            Some(endpoint) => self.open_model_stage(row, endpoint.clone()),
                            None => self.open_endpoint_stage(row),
                        };
                    }
                };
                if let Err(why) = said {
                    self.notice = Some(why);
                }
                self.picker = None;
            }
            KeyCode::Char('d') if self.pick_kind == Pick::Model => {
                let said = self.settings.apply(&format!("remove {}", at + 1));
                self.notice = Some(match said {
                    Ok(s) | Err(s) => s,
                });
                // The list stays open -- dropping several rows is one sitting
                // -- but its cursor cannot point past what is left.
                let left = self.settings.candidates().len();
                self.picker = Some(at.min(left.saturating_sub(1)));
            }
            KeyCode::Char('a') if self.pick_kind == Pick::Model => {
                // One thing on the screen at a time: a list drawn over the
                // typing line ate every letter the human pressed next, which
                // is what "a does nothing" looked like.
                // A row is chosen in two stages: the address first, from the
                // ones already in use or typed, then the id against it.
                self.open_picker(Pick::Endpoint(None));
            }
            KeyCode::Char('e') if self.pick_kind == Pick::Model => {
                self.open_picker(Pick::Endpoint(Some(at)));
            }
            KeyCode::Esc | KeyCode::Char('q') => self.picker = None,
            KeyCode::Char('M') if self.pick_kind == Pick::Model => self.picker = None,
            KeyCode::Char('E') if self.pick_kind == Pick::Effort => self.picker = None,
            KeyCode::Char('W') if self.pick_kind == Pick::Window => self.picker = None,
            _ => {}
        }
    }

    /// The second stage of a candidate row: the id, typed by hand, against
    /// the address just chosen. An empty address is the launch endpoint.
    fn open_model_stage(&mut self, row: Option<usize>, endpoint: String) {
        let model = row
            .and_then(|n| self.settings.candidates().get(n).map(|c| c.model.clone()))
            .unwrap_or_default();
        let where_ = if endpoint.is_empty() { "the launch endpoint" } else { endpoint.as_str() };
        self.notice = Some(match row {
            Some(_) => format!("model id for {where_}"),
            None => format!("model id {where_} serves"),
        });
        self.mode = Mode::Input(Input { kind: InputKind::ModelOf { row, endpoint }, buffer: model });
    }

    /// The first stage, typed: an address the list did not have.
    fn open_endpoint_stage(&mut self, row: Option<usize>) {
        let endpoint = row
            .and_then(|n| self.settings.candidates().get(n).map(|c| c.endpoint.clone()))
            .unwrap_or_default();
        self.notice = Some(String::from("the address requests from this row are sent to"));
        self.mode = Mode::Input(Input { kind: InputKind::Endpoint(row), buffer: endpoint });
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
            InputKind::Endpoint(row) => {
                // An address that speaks another protocol is refused here,
                // before the human is asked for a model id it could never
                // serve: the sentence that names the address that works is
                // worth more than one more question.
                match pearl_core::catalog::protocol_complaint(text.trim()) {
                    Some(why) => {
                        self.notice = Some(why);
                        self.open_picker(Pick::Endpoint(row));
                    }
                    None => self.open_model_stage(row, text.trim().to_string()),
                }
            }
            InputKind::ModelOf { row, endpoint } => {
                if text.trim().is_empty() {
                    self.notice = Some(String::from("a candidate needs a model id"));
                    return;
                }
                let line = match row {
                    Some(n) => format!("edit {} {} {}", n + 1, dash(&endpoint), text.trim()),
                    None => format!("add {} {}", dash(&endpoint), text.trim()),
                };
                match self.settings.apply(&line) {
                    // The notice names the resolved row -- `openai/gpt-4o`
                    // -- so the human sees what the endpoint made of it.
                    Ok(said) => self.notice = Some(said),
                    Err(why) => self.notice = Some(why),
                }
            }
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
        // The legend names the layout switch itself, last, so the row and
        // help read in one order rather than two.
        vec![
            ("start", vec![String::from("a add")]),
            ("look", self.motion_items()),
            ("work", ["i now", "t take", "s steer", "n nest", hold, "r resume"]
                .iter().map(|s| s.to_string()).collect()),
            ("safety", vec![String::from("z undo")]),
            ("ask", vec![String::from("/ btw"), String::from("T think")]),
            ("set", vec![String::from("S save"), String::from(": set")]),
        ]
    }

    /// The same groups in the same order as the row, each key given the
    /// sentence that says what it does -- in both languages, because the
    /// screen can be read in either and a translation invented at draw time
    /// is a translation that drifts.
    ///
    /// The English is written for someone who has never read this repository,
    /// and the Chinese is written for someone who has never read it in
    /// English: neither is the other's shadow. A word this project invented --
    /// the hand, level, steer, nest -- is explained where it is used, because
    /// a glossary the reader has to open first is a key they never press.
    pub fn help_groups(&self) -> Vec<(&'static str, &'static str, Vec<HelpRow>)> {
        let row = |k: &str, en: &str, zh: &str| HelpRow {
            keys: String::from(k),
            en: String::from(en),
            zh: String::from(zh),
        };
        // One line per window, in the order a hand uses them: a line, a page,
        // the ends. The style's own list, not a second copy of it.
        let look = match self.settings.keys() {
            Keys::Plain => vec![
                row("queue", "↑ / ↓ one task · PgUp / PgDn one screen of them · Home / End the first task / the last",
                    "队列  ↑ / ↓ 上下一个任务 · PgUp / PgDn 翻一屏 · Home / End 第一个任务 / 最后一个"),
                row("report", "Alt+↑/↓ one line · Alt+PgUp/Dn one screen · Alt+Home/End the oldest line / back to the newest",
                    "报告  Alt+↑/↓ 上下滚动一行 · Alt+PgUp/Dn 翻一屏 · Alt+Home/End 最旧的一行 / 回到最新"),
                row("level", "← / u / Backspace out to the parent task · → in to the tasks inside · 1 / 2 / 3 show the queue, the report, or both",
                    "层级  ← / u / Backspace 回到上一层 · → 进入子任务 · 1 / 2 / 3 只看队列 / 只看报告 / 都看"),
            ],
            Keys::Vim => vec![
                row("queue", "j / k one task · f / b one screen of them · C-d / C-u half a screen · gg / G the first task / the last",
                    "队列  j / k 上下一个任务 · f / b 翻一屏 · C-d / C-u 翻半屏 · gg / G 第一个任务 / 最后一个"),
                row("report", "M-j/M-k one line · M-f/M-b one screen · M-C-d/M-C-u half a screen · M-gg/M-G the oldest line / back to the newest",
                    "报告  M-j/M-k 上下滚动一行 · M-f/M-b 翻一屏 · M-C-d/M-C-u 翻半屏 · M-gg/M-G 最旧的一行 / 回到最新"),
                row("level", "h / u / Backspace out to the parent task · l in to the tasks inside · 1 / 2 / 3 show the queue, the report, or both",
                    "层级  h / u / Backspace 回到上一层 · l 进入子任务 · 1 / 2 / 3 只看队列 / 只看报告 / 都看"),
            ],
            Keys::Emacs => vec![
                row("queue", "C-n / C-p one task · C-v / M-v one screen of them · M-< / M-> the first task / the last",
                    "队列  C-n / C-p 上下一个任务 · C-v / M-v 翻一屏 · M-< / M-> 第一个任务 / 最后一个"),
                row("report", "M-n/M-p one line · C-M-v/C-M-S-v one screen · C-M-</C-M-> the oldest line / back to the newest · C-l jump back to the newest",
                    "报告  M-n/M-p 上下滚动一行 · C-M-v/C-M-S-v 翻一屏 · C-M-</C-M-> 最旧的一行 / 回到最新 · C-l 直接回到最新"),
                row("level", "^ / u / Backspace out to the parent task · RET in to the tasks inside · 1 / 2 / 3 or C-x 1/2/3 show the queue, the report, or both",
                    "层级  ^ / u / Backspace 回到上一层 · RET 进入子任务 · 1 / 2 / 3 或 C-x 1/2/3 只看队列 / 只看报告 / 都看"),
            ],
        };
        let hold = if self.settings.keys() == Keys::Vim { "H" } else { "h" };
        vec![
            ("start", "开始", vec![row(
                "a",
                "add a task to the end of the queue: it runs when everything ahead of it is done",
                "在队列末尾加一个任务，前面的都做完才轮到它",
            )]),
            ("look", "浏览", look),
            ("work", "干活", vec![
                row("i", "run a task right now: it goes ahead of everything already waiting",
                    "插队：马上执行，排在所有还在等的任务前面"),
                row("t", "pick a task up without running it, so you can rewrite it and put it back somewhere else",
                    "把任务取到手上但不执行，改完之后再放回队列的某个位置"),
                row("e", "rewrite the task in your hand; with an empty hand, this turns thinking up or down instead",
                    "改写手里的任务；手里空着时，这个键改成调大或调小模型的思考档位"),
                row("p / P", "put the task in your hand back, right after the focused one / at the very front of the queue",
                    "把手里那个任务放回队列：放在当前任务后面 / 放到最前面"),
                row("s", "add a note to a task that is already running, without stopping it",
                    "给已经开跑的任务补一句提示，不用中断它"),
                row("n", "split the focused task: add a smaller task inside it, for work that has its own steps",
                    "拆分当前任务：在它里面加一个更小的任务，用来装可以单独做完的一步"),
                row(hold, "stop the focused task from starting and keep it in the list until you start it",
                    "让当前任务先别开始，留在列表里等你之后手动让它跑"),
                row("r", "start a stopped task again", "让停下的任务重新开始"),
                row("R", "rewrite a task that has not started yet, keeping its place in the queue",
                    "改写一个还没开始的任务，它在队列里的位置不变"),
                row("c", "cancel the focused task; everything inside it is cancelled too",
                    "取消当前任务，它里面的子任务也一起取消"),
            ]),
            ("safety", "安全", vec![row(
                "z",
                "undo the last file change the model made through its tools",
                "撤销模型刚才通过工具做出的那次文件改动",
            )]),
            ("ask", "提问", vec![
                row("/", "ask a quick question beside the work: the answer is shown, never put in the queue",
                    "顺手问一句，不占用队列：回答只看一眼，不会变成任务"),
                row("T", "show or hide the model's reasoning under each answer",
                    "显示或隐藏模型在每个回答下面的思考过程"),
            ]),
            ("set", "设置", vec![
                row("m / M", "model: move to the next one / pick one from the list",
                    "模型：换到下一个 / 从清单里选一个"),
                row("e / E", "how hard the model thinks: next level / pick one from the list",
                    "模型的思考档位：换到下一档 / 从清单里选一个"),
                row("w / W", "context window: next size / pick one from the list",
                    "上下文窗口：换到下一档 / 从清单里选一个"),
                row("\\", "keyboard style: plain, vim, or emacs",
                    "键位风格：普通、vim、emacs 三种"),
                row("S", "save these settings to ~/.pearl/config, so the next run starts with them",
                    "把这些设置存进 ~/.pearl/config，下次启动直接沿用"),
                row(":", "name a setting directly, for values the lists do not cover",
                    "直接敲一条设置，用来设定清单里没有的值"),
                row("Esc", "close the side question or the message at the bottom",
                    "关掉侧问或底部那条提示"),
                row("?", "this screen; any other key closes it", "就是这一屏；按其它任意键关闭"),
                row("q / C-c", "quit, leaving the queue as it is",
                    "退出，队列保持原样"),
            ]),
        ]
    }

    /// How many lines the key screen has, so scrolling knows where it ends.
    pub fn help_len(&self) -> usize {
        let groups: usize = self.help_groups().iter().map(|(_, _, e)| 1 + e.len()).sum();
        2 + groups + self.settings_items().len() + self.place_items().len()
    }

    /// Move through the key screen. True when the key was a movement, so the
    /// caller knows the human is still reading rather than leaving.
    fn scroll_help(&mut self, key: KeyEvent) -> bool {
        let style = self.settings.keys();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let line = match key.code {
            KeyCode::Tab => {
                self.help_lang = match self.help_lang {
                    Lang::En => Lang::Zh,
                    Lang::Zh => Lang::En,
                };
                return true;
            }
            KeyCode::Down => 1,
            KeyCode::Up => -1,
            KeyCode::PageDown => 10,
            KeyCode::PageUp => -10,
            KeyCode::Home => isize::MIN / 2,
            KeyCode::End => isize::MAX / 2,
            KeyCode::Char('j') if style == Keys::Vim && !alt => 1,
            KeyCode::Char('k') if style == Keys::Vim && !alt => -1,
            KeyCode::Char('n') if style == Keys::Emacs && ctrl => 1,
            KeyCode::Char('p') if style == Keys::Emacs && ctrl => -1,
            KeyCode::Char('v') if style == Keys::Emacs && ctrl => 10,
            KeyCode::Char('v') if style == Keys::Emacs && alt => -10,
            _ => return false,
        };
        // Clamped to the screen's own length: an offset that runs past the
        // end is a scroll that has to be walked back before anything moves.
        let last = self.help_len().saturating_sub(1);
        self.help_at = ((self.help_at as isize + line).max(0) as usize).min(last);
        true
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

    /// The model as the header names it. A candidate that names its own
    /// endpoint names its own house: the launch credential is not the house
    /// the request went to once the human has chosen somewhere else.
    fn model_label(&self) -> String {
        let c = self.settings.chosen();
        if !c.endpoint.is_empty() {
            return c.label();
        }
        match &self.provider {
            Some(house) => format!("{house}/{}", c.model),
            None => c.model,
        }
    }

    pub fn settings_items(&self) -> Vec<String> {
        let effort = self.settings.effort().map(|e| e.as_str()).unwrap_or("auto");
        let model = self.model_label();
        let mut items = vec![
            format!("{model} (m/M)"),
            match self.settings.stated_dialect() {
                // A dialect that was stated or learned is worth the width: it
                // is why the ladder is the length it is.
                Some(d) => format!("think {effort}/{} (e/E)", d.as_str()),
                None => format!("think {effort} (e/E)"),
            },
            format!("{} (w/W)", self.gauge.describe(self.settings.window())),
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
        let model = self.model_label();
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
            Keys::Plain => "queue ↑↓/PgUp/Dn/Home/End  \
                            report Alt+↑↓/PgUp/Dn/Home/End  level ←→  \
                            1/2/3 queue/report/both",
            Keys::Vim => "queue j/k/f/b/gg/G  \
                          report M-j/M-k/M-f/M-b/M-gg/M-G  level h/l  \
                          1/2/3 queue/report/both",
            Keys::Emacs => "queue C-n/C-p/C-v/M-v/M-</M->  \
                            report M-n/M-p/C-M-v/C-M-S-v/C-M-</C-M->  level ^/RET  \
                            1/2/3 queue/report/both",
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
                    InputKind::Endpoint(None) => "add model: endpoint",
                    InputKind::Endpoint(Some(n)) => {
                        return format!("edit model {}: endpoint _", n + 1)
                    }
                    InputKind::ModelOf { row: None, .. } => "add model: model id",
                    InputKind::ModelOf { row: Some(n), .. } => {
                        return format!("edit model {}: model id {}_", n + 1, input.buffer)
                    }
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

/// How the grammar spells "the address the process launched against".
fn dash(endpoint: &str) -> &str {
    if endpoint.is_empty() { "-" } else { endpoint }
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
