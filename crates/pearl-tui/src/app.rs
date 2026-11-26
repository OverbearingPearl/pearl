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

use crate::draft;
use pearl_text::{Buffer, Code, Command, Encoding, Key, KeyMap, Outcome};
use pearl_core::{
    Aside, Effort, Form, Gauge, Keys, NodeState, Position, Queue, QueueError, Request, Settings,
    TaskId,
};

/// One key, one row, one sentence. The bottom row and the key screen are two
/// views of this table, so a key named in one is named in the other and a key
/// whose job changes changes it in both. `row` is `None` when a contextual
/// line (the hand) or the header already shows the key where it applies: a
/// reading is learned beside the reading, not in a list of letters.
struct Verb {
    group: &'static str,
    /// The keys as the key screen prints them.
    keys: &'static str,
    /// The words the bottom row has room for.
    row: Option<&'static str>,
    /// Kept at the row's end, so a narrow window drops readings before it
    /// drops the way out.
    tail: bool,
    en: &'static str,
    zh: &'static str,
}

fn group_names(group: &str) -> (&'static str, &'static str) {
    match group {
        "start" => ("start", "开始"),
        "look" => ("look", "浏览"),
        "work" => ("work", "干活"),
        "safety" => ("safety", "安全"),
        "ask" => ("ask", "提问"),
        _ => ("set", "设置"),
    }
}

const VERBS: &[Verb] = &[
    Verb {
        group: "start",
        keys: "a",
        row: Some("a add"),
        tail: false,
        en: "add a task to the end of the queue: it runs when everything ahead of it is done",
        zh: "在队列末尾加一个任务，前面的都做完才轮到它",
    },
    Verb {
        group: "look",
        keys: "1|2|3",
        row: Some("1|2|3 panes"),
        tail: false,
        en: "show the queue, the report, or both on the screen",
        zh: "只看队列 / 只看报告 / 都看",
    },
    Verb {
        group: "look",
        keys: "v",
        row: None,
        tail: false,
        en: "read the focused task's request in full: a queue row shortens a long title where the screen ends",
        zh: "看当前任务的完整原始输入：队列里的标题放不下时会截断，这里能看到全文",
    },
    Verb {
        group: "look",
        keys: "f",
        row: None,
        tail: false,
        en: "read what the focused task touched: the writes it is on the hook for, oldest first, and the commands it ran",
        zh: "看这条任务碰过什么：它写过的文件和跑过的命令，从旧到新",
    },
    Verb {
        group: "work",
        keys: "Tab",
        row: None,
        tail: false,
        en: "take the keys to the queue while the draft waits, and bring them back to the words with one more Tab; Esc puts a draft down and a second Esc drops it",
        zh: "把按键交给队列，草稿原样等着，再按一次 Tab 回到草稿；Esc 放下草稿，再按一次丢弃",
    },
    Verb {
        group: "work",
        keys: "i",
        row: Some("i now"),
        tail: false,
        en: "run a task right now: it goes ahead of everything already waiting",
        zh: "插队：马上执行，排在所有还在等的任务前面",
    },
    Verb {
        group: "work",
        keys: "t",
        row: Some("t take"),
        tail: false,
        en: "pick a task up without running it, so you can put it back somewhere else; ⏎ retypes it while it is in your hand, and ␛ drops it",
        zh: "把任务取到手上但不执行，之后放回队列的另一个位置；拿在手上时按 ⏎ 可以改写它，按 ␛ 丢掉它",
    },
    Verb {
        group: "work",
        keys: "r",
        row: Some("r rewrite"),
        tail: false,
        en: "rewrite a live task where it is: a waiting or stopped one gets new words, a running one is re-planned around them, and it keeps its place and the work already done",
        zh: "就地改写当前任务：还在等的换成新的说法，已经开跑的围绕新目标重新计划，位置和已经做完的部分都保留",
    },
    Verb {
        group: "work",
        keys: "p|P",
        row: None,
        tail: false,
        en: "put the task in your hand back, right after the focused one with lowercase, at the very front of the queue with uppercase",
        zh: "把手里那个任务放回队列：小写放在当前任务后面，大写放到最前面",
    },
    Verb {
        group: "work",
        keys: "s",
        row: Some("s steer"),
        tail: false,
        en: "say something to the focused task: a note while it runs, without stopping it, or the answer when it is waiting on you",
        zh: "给当前任务说一句：它在跑就补一句提示、不打断它，它等你答话时这就是回答",
    },
    Verb {
        group: "work",
        keys: "n",
        row: None,
        tail: false,
        en: "split the focused task: add a smaller task inside it, for work that has its own steps",
        zh: "拆分当前任务：在它里面加一个更小的任务，用来装可以单独做完的一步",
    },
    Verb {
        group: "work",
        keys: "␠",
        row: Some("␠ pause"),
        tail: false,
        en: "one key for both ways: the row says which one this task takes -- `␠ pause` stops it where it is, `␠ run` lets a stopped one go on",
        zh: "两个方向一个键，底行会按当前任务写明是哪个：`␠ pause` 就地停住，`␠ run` 让停下的接着跑",
    },
    Verb {
        group: "work",
        keys: "R",
        row: Some("R retry"),
        tail: false,
        en: "run a finished task again from the start: one that failed, was cancelled, or already answered",
        zh: "把已经结束的任务从头再跑一遍：失败的、取消的、答完的都可以",
    },
    Verb {
        group: "work",
        keys: "c",
        row: Some("c cancel"),
        tail: false,
        en: "the row names it for the row at hand: `c cancel` stops a live task and everything inside it, `c dissolve` takes a topic apart while none of its parts has run, `c drop` takes a finished row off the list",
        zh: "底行按当前那一行换词：`c cancel` 停掉还在的任务（含子任务），`c dissolve` 把还没跑过任何一部分的话题拆开，`c drop` 把已结束的那一行从列表去掉",
    },
    Verb {
        group: "safety",
        keys: "z",
        row: Some("z undo"),
        tail: false,
        en: "undo the last command the model ran in its shell, putting back the files that command changed",
        zh: "撤销模型上一次在 shell 里跑的命令，把它改动的文件恢复原样",
    },
    Verb {
        group: "safety",
        keys: "y",
        row: Some("y copy"),
        tail: false,
        en: "copy the focused task to the system clipboard: its words, what the environment noted about it, and the answer, without the bills the report adds",
        zh: "把当前任务复制到系统剪贴板：任务内容、环境提示和答案，不含报告里那些计量",
    },
    Verb {
        group: "ask",
        keys: "/",
        row: Some("/ btw"),
        tail: false,
        en: "ask a quick question beside the work: the answer is shown, never put in the queue, and ␛ clears it",
        zh: "顺手问一句，不占用队列：回答只看一眼，按 ␛ 收起来",
    },
    Verb {
        group: "ask",
        keys: "T",
        row: Some("T reason"),
        tail: false,
        en: "show or hide the model's reasoning under each answer",
        zh: "显示或隐藏模型在每个回答下面的思考过程",
    },
    // The header prints each knob beside its reading, and the row does not
    // repeat it: a key whose job is to change a number printed three columns
    // away is taught once, where the number sits. Help still names it, so the
    // screen that lists every key stays the one place that does.
    Verb {
        group: "set",
        keys: "m|M",
        row: None,
        tail: false,
        en: "walk the models this endpoint offered, or pick one from the list: the header prints the model beside this key",
        zh: "在这些模型之间走，或从清单里挑一个：表头就在这组键旁边显示当前模型",
    },
    Verb {
        group: "set",
        keys: "e|E",
        row: None,
        tail: false,
        en: "walk how hard the model thinks, or pick a level from the list; auto lets the task decide",
        zh: "在思考档位之间走，或从清单里挑一个；auto 表示由任务自己决定",
    },
    Verb {
        group: "set",
        keys: "w|W",
        row: None,
        tail: false,
        en: "walk the context window sizes, or pick one from the list; a smaller window folds its history sooner",
        zh: "在上下文窗口大小之间走，或从清单里挑一个；窗口越小，历史越早被压缩",
    },
    Verb {
        group: "set",
        keys: "\\",
        row: None,
        tail: false,
        en: "switch the keyboard between plain, vim and emacs: the header names the one in use",
        zh: "在 plain、vim、emacs 三种按键风格之间切换：表头显示当前用的是哪种",
    },
    Verb {
        group: "set",
        keys: "␛",
        row: None,
        tail: false,
        en: "close whatever is open -- a prompt, a list, a screen, or the answer beside the work; with a task in your hand, it drops it",
        zh: "关掉当前打开的东西：输入行、清单、整屏，或者旁边那个回答；手上有任务时，它把任务丢掉",
    },
    Verb {
        group: "set",
        keys: "S",
        row: Some("S save"),
        tail: false,
        en: "save these settings to ~/.pearl/config, so the next run starts with them",
        zh: "把这些设置存进 ~/.pearl/config，下次启动直接沿用",
    },
    Verb {
        group: "set",
        keys: ":",
        row: Some(": set"),
        tail: false,
        en: "name a setting directly, for values the lists do not cover",
        zh: "直接敲一条设置，用来设定清单里没有的值",
    },
    Verb {
        group: "set",
        keys: "q|C-c",
        row: Some("q quit"),
        tail: true,
        en: "quit, leaving the queue as it is; C-c puts a draft down and closes a list before it quits",
        zh: "退出，队列保持原样；C-c 先放下草稿、关掉清单，再退出",
    },
];


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
            Lang::En => "keys  (Tab Eng|中, ⇧⇩|PgUp|PgDn scroll, any other key closes)",
            Lang::Zh => "按键  (Tab Eng|中, ⇧⇩|PgUp|PgDn 滚动, 其它键关闭)",
        }
    }
}

/// One line of the key screen: the keys, and what they do in both languages.
#[derive(Clone)]
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
    pub draft: Buffer,
    /// Whether the caret is in the draft. A draft that loses the caret keeps
    /// its words and its rows: `Tab` takes the keys to the work and brings
    /// them back without asking the human to write it twice.
    pub focused: bool,
}

impl Input {
    pub fn new(kind: InputKind, text: impl Into<String>) -> Self {
        Self { kind, draft: Buffer::new(text), focused: true }
    }
}

/// What committing the prompt will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKind {
    /// Place a new task after the focus.
    Append,
    /// Place a new task at the front.
    InsertNow,
    /// Start a topic at the end of the roots: a heading for a line of work.
    Topic,
    /// Annotate a task with steering.
    Steer(TaskId),
    /// Answer the question a task is waiting on.
    Answer(TaskId),
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
    /// The lifted node: a task or a topic, kept whole. The hand is an id, not
    /// a request, because a topic taken off the list must keep its parts.
    pub hand: Option<TaskId>,
    /// A side question held outside the queue, until dismissed.
    pub aside: Option<Aside>,
    /// Why the last key did nothing. A refused key that says nothing leaves the
    /// human guessing whether the tool is broken or they are.
    pub notice: Option<String>,
    /// The address pearl itself sends to. A row may name it, and the list
    /// shows it as the launch endpoint rather than twice.
    pub launch: String,
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
    /// The two editors behind the surface. Each holds only the state a buffer
    /// does not: emacs's one kill, vim's mode and register. They are both kept
    /// even while the other style is in use, because the style can change
    /// mid-draft and a half-typed paragraph outlives the keyboard.
    pub emacs: pearl_emacs::Emacs,
    pub vim: pearl_vim::Vim,
    /// Text the surface has copied but not yet handed to the terminal: the
    /// loop owns stdout, and the surface only says what it decided.
    pub pending_copy: Option<String>,
    /// Questions the human has already put down. A task that asks again on the
    /// next frame would take the line back from someone who refused it once.
    declined: Vec<pearl_core::TaskId>,
    /// The whole legend on a screen of its own, for a window too narrow to
    /// carry it and for a human who wants to read it once.
    /// How many rows a page is in each window. A page is what the drawing
    /// measured, so these are cells: the panes take `&App` to draw and hand
    /// back the height they were given, which is the only figure that matches
    /// the rows on the screen.
    pub queue_page: std::cell::Cell<usize>,
    pub report_page: std::cell::Cell<usize>,
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
    /// Whether the focused task's request is open on a screen of its own. A
    /// queue row cuts a long title where the glass ends, and the rest of the
    /// request has to be readable somewhere or it is lost.
    pub read: bool,
    /// How far the request screen is scrolled, in wrapped lines.
    pub read_at: usize,
    /// Words for the screen `read` shows instead of the focused request: what
    /// one task touched, asked for with `f` and built by the caller, who holds
    /// the ledger.
    pub read_text: Option<String>,
    /// The task whose file report was asked for, waiting on the caller.
    pub files_requested: Option<TaskId>,
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
    /// Whether this terminal speaks the enhanced keyboard encoding. Decided
    /// once, when the surface asks the terminal, and then read by the legend:
    /// a key the terminal cannot send is a key the legend must not name.
    pub keys_capable: bool,
    /// The task the cursor was on, and the state it was in, as of the last
    /// frame. A task that settles has to be noticed once: noticed twice it
    /// would drag the cursor off the answer the human is reading.
    seen: Option<(TaskId, pearl_core::NodeState)>,
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
            emacs: pearl_emacs::Emacs::new(),
            vim: pearl_vim::Vim::typing(),
            pending_copy: None,
            declined: Vec::new(),
            queue_page: std::cell::Cell::new(10),
            report_page: std::cell::Cell::new(10),
            help: false,
            read: false,
            read_at: 0,
            read_text: None,
            files_requested: None,
            undo_requested: false,
            save_requested: false,
            panes: Panes::Split,
            report_back: 0,
            report_len: std::cell::Cell::new(0),
            show_thinking: false,
            gauge: Gauge::new(),
            keys_capable: false,
            seen: None,
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
    /// Say a key cannot act on this task, before it asks for words it cannot
    /// use: the queue's own wording, so the two refusals never disagree.
    fn refused(&mut self, said: &str, verb: pearl_core::Verb, id: TaskId, found: NodeState) {
        self.notice = Some(format!("{said}: {}", reason(&verb.wrong_state(id, found))));
    }

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

    /// How far back the window can sit: the document's length less the pane it
    /// is drawn in, or nothing while no drawing has measured either.
    fn report_room(&self) -> Option<usize> {
        let len = self.report_len.get();
        (len > 0).then(|| len.saturating_sub(self.report_page.get()))
    }

    /// Scroll the report: negative reads back, positive returns to the newest.
    /// A window is one screen tall, so its furthest place is the top of the
    /// first full screen -- not the end of the text less one line, which is a
    /// window shorter than the pane. Clamped at both ends: at the newest line,
    /// and at that first screen, so a page key at the top stays full.
    pub fn scroll_report(&mut self, lines: isize) {
        if self.report_back == usize::MAX {
            // Standing at the far end, the distance back is whatever the last
            // drawing measured; a page forward from there is a real page.
            self.report_back = self.report_room().unwrap_or(0);
        }
        if lines < 0 {
            self.report_back = self.report_back.saturating_add(lines.unsigned_abs());
        } else {
            self.report_back = self.report_back.saturating_sub(lines as usize);
        }
        if let Some(room) = self.report_room() {
            self.report_back = self.report_back.min(room);
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

    /// The focused node's id and form, when the focus is on a node in view.
    fn focused_form(&self) -> Option<(TaskId, Form)> {
        let focus = self.queue.focus()?;
        self.queue.view().iter().find(|n| n.id == focus).map(|n| (n.id, n.form))
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

    /// Where the attention goes when the task in front of you settles: the next
    /// unsettled task below it, and no further. A run ends without a keystroke,
    /// so the cursor has to move with the queue or the report beside it goes on
    /// describing work that is over. Only a change under the cursor moves it:
    /// a human who walked back to a finished task asked for that task, and a
    /// walk is not a queue move. The last task keeps the cursor, because the
    /// answer it just wrote is what there is to read.
    pub fn follow(&mut self) {
        let view = self.queue.view();
        let Some(focus) = self.queue.focus() else {
            return;
        };
        let Some(at) = view.iter().position(|n| n.id == focus) else {
            return;
        };
        let settled = |state: pearl_core::NodeState| {
            matches!(
                state,
                pearl_core::NodeState::Done
                    | pearl_core::NodeState::Failed
                    | pearl_core::NodeState::Cancelled
            )
        };
        let moved = match self.seen {
            Some((id, was)) if id == focus && was != view[at].state && settled(view[at].state) => {
                view[at..]
                    .iter()
                    .find(|n| !settled(n.state))
                    .map(|n| n.id)
            }
            _ => None,
        };
        if let Some(next) = moved {
            let _ = self.queue.focus_on(next);
        }
        self.seen = self
            .queue
            .focus()
            .and_then(|id| self.queue.get(id).map(|n| (id, n.state)));
    }

    /// Close what is open, or leave when nothing is. The order is the order a
    /// human would undo things in: the line being typed, the list over it, the
    /// screen over that, and only then the program.
    fn back_out(&mut self) {
        if self.picker.is_some() {
            self.picker = None;
            return;
        }
        if matches!(self.mode, Mode::Input(_)) {
            // A draft is put down, not thrown away: a paragraph is not
            // something a stray `C-c` gets to delete. The notice says how to
            // come back and how to drop it for good.
            self.put_down();
            return;
        }
        if self.help {
            self.help = false;
            self.help_at = 0;
            return;
        }
        if self.read {
            self.read = false;
            self.read_at = 0;
            return;
        }
        self.quit = true;
    }

    /// Handle one key. Pure: mutates state, returns nothing, needs no terminal.
    pub fn handle_key(&mut self, key: KeyEvent) {
        let key = spelled(key);
        // The one chord the surface answers, and it answers it from every
        // screen. It backs out one step before it leaves: a shell kills the
        // line with C-c and so does an editor, and a human half-way through a
        // title did not ask to lose the session. Nothing open means nothing
        // to back out of, and then it quits.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !meta(key.modifiers)
            && key.code == KeyCode::Char('c')
        {
            self.back_out();
            return;
        }
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
        if self.read {
            if !self.scroll_read(key) {
                self.read = false;
                self.read_at = 0;
            }
            return;
        }
        if key.code == KeyCode::Char('?') && matches!(self.mode, Mode::Normal) {
            self.help = true;
            self.help_at = 0;
            return;
        }
        match &mut self.mode {
            Mode::Input(input) if input.focused => self.handle_input_key(key),
            _ => self.handle_normal_key(key),
        }
    }

    /// Motion, in the keyboard the human chose. Returns true when the key was
    /// a motion, so the product's own letters are only reached afterwards.
    fn handle_motion(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = meta(key.modifiers);
        let style = self.settings.keys();
        // One rule across every style: the plain key acts on the queue and Alt
        // acts on the report. The region is an argument of the keypress, not a
        // mode the surface remembers. What is universal here is only what all
        // three editors really share -- arrows and page keys. Letters belong to
        // the style that owns them: j/k in a plain legend is a vim key wearing
        // the wrong clothes.
        let plain = matches!(style, Keys::Plain);
        let qpage = self.queue_page.get() as isize;
        let rpage = self.report_page.get() as isize;
        match key.code {
            KeyCode::Char('1') => return self.take_layout(Panes::Queue),
            KeyCode::Char('2') => return self.take_layout(Panes::Report),
            KeyCode::Char('3') => return self.take_layout(Panes::Split),
            KeyCode::Down if !alt => return self.queue_line(1),
            KeyCode::Up if !alt => return self.queue_line(-1),
            KeyCode::Down if alt => return self.report_line(1),
            KeyCode::Up if alt => return self.report_line(-1),
            // The four keys that sit outside the letters belong to the plain
            // keyboard. Vim and emacs reach the same places from the home row
            // -- `gg`/`G`, `M-/`/`M-\` -- and a style that taught both would
            // be teaching a hand that never leaves the letters to leave them.
            KeyCode::PageDown if !alt && plain => return self.queue_line(qpage),
            KeyCode::PageUp if !alt && plain => return self.queue_line(-qpage),
            KeyCode::PageDown if alt && plain => return self.report_line(rpage),
            KeyCode::PageUp if alt && plain => return self.report_line(-rpage),
            KeyCode::End if alt && plain => {
                self.report_back = 0;
                return true;
            }
            KeyCode::Home if alt && plain => {
                self.report_oldest();
                return true;
            }
            KeyCode::Home if plain => return self.queue_line(isize::MIN / 2),
            KeyCode::End if plain => return self.queue_line(isize::MAX / 2),
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
                // Down is one control byte, so this chord arrives everywhere.
                KeyCode::Char('v') if ctrl && alt => return self.report_line(rpage),
                // Up needs Shift beside Ctrl, which a terminal without the
                // encoding drops. There the letter carries the shift instead,
                // the spelling vim's report page already uses.
                KeyCode::Char('V') if ctrl && alt => return self.report_line(-rpage),
                KeyCode::Char('P') if alt && !self.keys_capable => return self.report_line(-rpage),
                KeyCode::Char('n') if alt => return self.report_line(1),
                KeyCode::Char('p') if alt => return self.report_line(-1),
                // The queue's ends, the way nano spells them: two letters a
                // hand already rests on, one for each end. Home and End are
                // the plain keyboard's keys, and emacs reaches them here.
                KeyCode::Char('/') if alt => return self.queue_line(isize::MIN / 2),
                KeyCode::Char('\\') if alt => return self.queue_line(isize::MAX / 2),
                // The report's ends. `C-M-<` is one keystroke only where the
                // encoding says so; elsewhere Alt and the bracket is the same
                // motion.
                KeyCode::Char('<') if alt && ctrl => self.report_oldest(),
                KeyCode::Char('>') if alt && ctrl => self.report_back = 0,
                KeyCode::Char('<') if alt => self.report_oldest(),
                KeyCode::Char('>') if alt => self.report_back = 0,
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
        let alt = meta(key.modifiers);
        // Every arm below is a bare key. A chord that arrives wearing a letter
        // is not one: a terminal speaking the kitty encoding sends C-M-v as a
        // single `Char('v')`, and read as a letter it opened the request
        // screen; through `\\` it changed the keyboard itself, and every letter
        // after that was typed into a prompt nobody asked for. C-c is answered
        // before any mode is consulted, so it never reaches this point.
        if ctrl || alt {
            return;
        }
        match key.code {
            KeyCode::Char('q') => {
                if self.hand.is_some() {
                    self.notice = Some(String::from(
                        "quit: a task is in your hand; p puts it back, ␛ drops it",
                    ));
                    return;
                }
                self.quit = true;
            }
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
            // A draft that has lost the caret is still there, and this is the
            // key that says so. With nothing written there is nothing to go
            // back to, and a key that pretends otherwise leaves the human
            // pressing it.
            KeyCode::Tab => {
                if matches!(self.mode, Mode::Input(_)) {
                    if let Mode::Input(input) = &mut self.mode {
                        input.focused = true;
                    }
                    self.notice = None;
                } else {
                    self.notice = Some(String::from("tab: nothing is being written"));
                }
            }
            KeyCode::Char('y') => self.yank_document("copy"),
            KeyCode::Esc => {
                // A draft that lost the caret is still the human's words, so
                // Escape is how they are let go; nothing else on the surface
                // throws them away.
                if matches!(self.mode, Mode::Input(_)) {
                    self.mode = Mode::Normal;
                    self.aside = None;
                    self.notice = Some(String::from("the draft was dropped"));
                } else if let Some(id) = self.hand.take() {
                    // Letting go is a removal: the node stayed in the tree
                    // while it was lifted, so dropping it takes it (and its
                    // parts) out of the list.
                    let _ = self.queue.discard(id);
                    self.aside = None;
                    self.notice = Some(String::from("the task in your hand was dropped"));
                } else {
                    self.aside = None;
                    self.notice = None;
                }
            }
            KeyCode::Char('a') => self.prompt(InputKind::Append),
            KeyCode::Char('A') => {
                // A topic is a root by nature: a heading inside a heading would
                // make membership a tree of trees, and the log records one.
                if self.queue.scope().is_some() {
                    self.notice = Some(String::from(
                        "a topic is a root: leave this one before starting another",
                    ));
                    return;
                }
                self.prompt(InputKind::Topic);
            }
            KeyCode::Char('i') => self.prompt(InputKind::InsertNow),
            KeyCode::Char('s') => {
                // A task waiting on a question gets an answer, not steering:
                // the same key, because the human's move is the same -- say
                // something to this row from the keyboard.
                if let Some(id) = self.target("steer") {
                    // The line opens only for a task that can take words: a
                    // line that accepted typing and refused it on Enter spent
                    // the human's sentence to learn what the key already knew.
                    match self.queue.get(id).map(|n| n.state) {
                        Some(NodeState::AwaitingInput) => self.prompt(InputKind::Answer(id)),
                        Some(NodeState::Queued) | Some(NodeState::Running) => {
                            self.prompt(InputKind::Steer(id))
                        }
                        Some(found) => self.refused("steer", pearl_core::Verb::Steer, id, found),
                        None => {}
                    }
                }
            }
            KeyCode::Char('n') => {
                if let Some(id) = self.target("nest") {
                    // A settled task cannot grow work, and the words would be
                    // typed for nothing: the refusal belongs at the key.
                    match self.queue.get(id).map(|n| n.state) {
                        Some(NodeState::Queued)
                        | Some(NodeState::Running)
                        | Some(NodeState::Paused) => self.prompt(InputKind::Nest(id)),
                        Some(found) => {
                            self.refused("nest", pearl_core::Verb::Nest, id, found)
                        }
                        None => {}
                    }
                }
            }
            // The row cuts a long request where the glass ends; the whole of
            // it is one key away rather than nowhere.
            KeyCode::Char('f') => {
                if let Some(id) = self.target("files") {
                    self.files_requested = Some(id);
                }
            }
            KeyCode::Char('v') => {
                self.read = true;
                self.read_at = 0;
            }
            // Rewrite the focused task where it stands. Moving it through the
            // hand would lose the two things this keeps: its place on the list
            // and the work already done on it.
            KeyCode::Char('r') => {
                if let Some(id) = self.target("rewrite") {
                    // A settled task has nothing to rewrite: the line would
                    // take words the queue then hands back.
                    match self.queue.get(id).map(|n| n.state) {
                        Some(NodeState::Queued)
                        | Some(NodeState::Running)
                        | Some(NodeState::Paused)
                        | Some(NodeState::AwaitingInput) => self.prompt(InputKind::Redirect(id)),
                        Some(found) => {
                            self.refused("rewrite", pearl_core::Verb::Rewrite, id, found)
                        }
                        None => {}
                    }
                }
            }
            KeyCode::Char('t') => {
                // A second take would overwrite the first, and the first has
                // already left the queue: the words would be gone from both.
                if self.hand.is_some() {
                    self.notice = Some(String::from(
                        "take: a task is already in your hand; p puts it back, ␛ drops it",
                    ));
                    return;
                }
                if let Some(id) = self.target("take") {
                    let outcome = self.queue.take(id);
                    if let Some((lifted, _)) = self.did("take", outcome) {
                        self.hand = Some(lifted);
                    }
                }
            }
            // One job: the effort key steps the effort whether or not a task
            // is in the hand. Enter opens the hand, where the footer says so.
            KeyCode::Char('e') => self.settings.cycle_effort(),
            KeyCode::Char('p') => self.place_hand(Position::After),
            KeyCode::Char('P') => self.place_hand(Position::Now),
            // One key for one state: a task that is running or waiting stops,
            // and a task that is stopped goes on. Two keys said the same thing
            // and made the human read the row to know which one applied.
            KeyCode::Char(' ') => {
                if let Some(id) = self.target("pause") {
                    let stopped = matches!(
                        self.queue.get(id).map(|n| (n.state, n.held)),
                        Some((NodeState::Paused, _)) | Some((NodeState::Queued, true))
                    );
                    let outcome = if stopped {
                        self.queue.run(id)
                    } else {
                        self.queue.hold(id, "")
                    };
                    self.did("pause", outcome);
                }
            }
            KeyCode::Char('c') => {
                if let Some(id) = self.target("cancel") {
                    if self.queue.get(id).map(|n| n.form == Form::Topic).unwrap_or(false) {
                        let outcome = self.queue.dissolve(id);
                        self.did("dissolve", outcome);
                        return;
                    }
                    let settled = self
                        .queue
                        .get(id)
                        .map(|node| node.is_settled())
                        .unwrap_or(false);
                    // A finished row is reading material, not work: cancel on
                    // it takes the row away instead of refusing, so the list
                    // can be cleared without another key.
                    let outcome = if settled {
                        self.queue.withdraw(id)
                    } else {
                        self.queue.cancel(id)
                    };
                    self.did("cancel", outcome);
                }
            }
            KeyCode::Char('R') => {
                if let Some(id) = self.target("retry") {
                    let outcome = self.queue.retry(id);
                    self.did("retry", outcome);
                }
            }
            KeyCode::Enter => {
                // A task held in the hand is a request in the footer, and the
                // line under it names Enter: opening it is the one edit the
                // hand has, so it sits where the hand is shown rather than in
                // the work group, where it would mean a level.
                if self.hand.is_some() {
                    self.prompt(InputKind::EditHand);
                    return;
                }
                if let Some(id) = self.target("drill in") {
                    let outcome = self.queue.narrow(id);
                    self.did("drill in", outcome);
                }
            }
            KeyCode::Char('u') | KeyCode::Backspace => self.queue.unnarrow(),
            _ => {}
        }
    }

    /// The table the draft reads from: the style's own, with the two keys
    /// the surface insists on bound by the host and not by the editor. `⏎`
    /// hands the task in and `⇧⏎` breaks the line -- that rebinding is the
    /// whole reason the tables belong to the host. Vim's Normal mode gets no
    /// break: a newline is not something a mode that only moves can type.
    fn draft_map(&self) -> KeyMap {
        let style = self.settings.keys();
        let vim = self.vim.mode();
        let map = match style {
            Keys::Plain => pearl_text::plain(),
            Keys::Emacs => pearl_emacs::map(),
            Keys::Vim => match vim {
                pearl_vim::Mode::Insert => pearl_vim::insert(),
                _ => pearl_vim::normal(),
            },
        };
        let map = map.bind(Key::enter(), Command::Submit);
        if style == Keys::Vim && vim != pearl_vim::Mode::Insert {
            return map;
        }
        // `C-j` is a line feed on the wire; a terminal that cannot tell the
        // two apart sends it as `⏎` with Control, and one that can sends `j`.
        map.bind(Key::shift_enter(), Command::Break)
            .bind(Key::enter().ctrl(), Command::Break)
            .bind(Key::ch('j').ctrl(), Command::Break)
    }

    /// A key while the caret is in the draft. The style decides what the keys
    /// mean; the tables that answer that question live in `pearl-text`,
    /// `pearl-emacs` and `pearl-vim`, and this function is the host's half of
    /// the bargain: it hands the key over, and it does the two things an
    /// editor cannot do alone -- hand the form in, put it down when told.
    fn handle_input_key(&mut self, key: KeyEvent) {
        // A Command/Windows/Super chord belongs to the terminal or the window
        // manager: it is never text, and typing it would put a letter in the
        // draft that the human never saw themselves press.
        if key.modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META)
        {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let key = as_key(key, ctrl, alt);
        // `Tab` is the surface's own: it takes the keys away from the draft
        // without touching the words, so it is answered before any editor.
        if key.code == Code::Tab {
            self.blur_draft();
            return;
        }
        let style = self.settings.keys();
        let map = self.draft_map();
        let Mode::Input(input) = &mut self.mode else {
            return;
        };
        let outcome = match style {
            Keys::Plain => pearl_text::plain_handle(key, &mut input.draft, &map),
            Keys::Emacs => self.emacs.handle(key, &mut input.draft, &map),
            Keys::Vim => self.vim.handle(key, &mut input.draft, &map),
        };
        match outcome {
            Outcome::Host(Command::Submit) => self.commit_input(),
            Outcome::Host(Command::Cancel) => self.put_down(),
            Outcome::Host(Command::Leave) => self.blur_draft(),
            Outcome::Host(_) | Outcome::Handled | Outcome::Ignored => {}
        }
    }

    /// Whether the draft is being written in vim's Normal mode.
    fn vim_normal(&self) -> bool {
        self.settings.keys() == Keys::Vim && self.vim.mode() == pearl_vim::Mode::Normal
    }

    /// Leaves the caret where it is and lets the work have the keys. The draft
    /// keeps its rows and its words; `Tab` brings the caret back.
    /// Put the draft down without losing it: the keys go back to the queue
    /// and `Tab` brings the caret back to the words.
    fn put_down(&mut self) {
        if let Mode::Input(input) = &mut self.mode {
            input.focused = false;
            // A question put down is a question the human chose to leave for
            // later: the next frame must not hand it back to them.
            if let InputKind::Answer(id) = input.kind {
                self.declined.push(id);
            }
        }
        self.notice = Some(String::from("the draft is kept: Tab back to it, ␛ drops it"));
    }

    /// Open the line for a task that stopped to ask, when nothing holds it.
    ///
    /// A run that stops and says nothing leaves a still screen and no idea
    /// that the answer is the human's to type. The row carries the question,
    /// but a row is read only after the eye goes looking; the line that takes
    /// typing is where the eye already is. A draft being typed, a request in
    /// hand, or a question already refused keeps the line where it is.
    pub fn offer_question(&mut self) {
        if !matches!(self.mode, Mode::Normal) {
            return;
        }
        // A request in hand is a move half made: the line that carries it is
        // the one the human is looking at, and taking it away would undo a
        // placement they were in the middle of.
        if self.hand.is_some() {
            return;
        }
        let waiting = self
            .queue
            .view()
            .iter()
            .find(|n| n.state == pearl_core::NodeState::AwaitingInput && !self.declined.contains(&n.id))
            .map(|n| (n.id, n.question.clone().unwrap_or_default()));
        let Some((id, question)) = waiting else {
            return;
        };
        let _ = self.queue.focus_on(id);
        self.mode = Mode::Input(Input::new(InputKind::Answer(id), String::new()));
        self.notice = Some(question);
    }

    pub fn blur_draft(&mut self) {
        if let Mode::Input(input) = &mut self.mode {
            input.focused = false;
            self.notice = None;
        }
    }

    /// Copy the focused task as the document a human would paste: its words,
    /// what the environment noted about it, and the answer -- without the turn
    /// bills, which measure the work instead of being it, and without any of
    /// the glass the pane draws it in.
    fn yank_document(&mut self, verb: &str) {
        let Some(id) = self.target(verb) else {
            return;
        };
        let Some(node) = self.queue.get(id) else {
            return;
        };
        let mut doc = String::new();
        doc.push_str(&node.title);
        doc.push('\n');
        for notice in &node.notices {
            doc.push_str("note: ");
            doc.push_str(notice);
            doc.push('\n');
        }
        if let Some(result) = &node.result {
            doc.push('\n');
            doc.push_str(result);
            doc.push('\n');
        }
        self.pending_copy = Some(doc);
        self.notice = Some(String::from("copying..."));
    }

    /// The rows the draft is allowed to take: a third of the window, never
    /// fewer than two, never more than eight, and never so many that the two
    /// panes fall below three rows each. The dead without a limit is a window
    /// where a paragraph has eaten the queue.
    pub fn draft_cap(&self, height: usize) -> usize {
        let room = height.saturating_sub(8).max(1);
        (height / 3).clamp(1, 8).min(room)
    }

    /// What the draft needs said beside it, if anything: the way to break the
    /// line, until the line has been broken once. A hint about a key that has
    /// already been used is a hint that will never be read again.
    pub fn draft_hint(&self) -> Option<String> {
        let Mode::Input(input) = &self.mode else {
            return None;
        };
        if !input.focused {
            return Some(String::from("Tab to return"));
        }
        // vim's Normal mode has no caret sign of its own, so the words being
        // readable would be the only way to tell it from Insert: the hint says
        // which one the keys are in.
        if self.vim_normal() {
            return Some(String::from("i insert"));
        }
        if input.draft.has_break() || !matches!(
            input.kind,
            InputKind::Append
                | InputKind::InsertNow
                | InputKind::Topic
                | InputKind::Steer(_)
                | InputKind::Nest(_)
                | InputKind::Redirect(_)
                | InputKind::EditHand
                | InputKind::Aside
        ) {
            return None;
        }
        // `C-j` is a break only where the terminal tells it apart from `C-m`;
        // elsewhere it arrives as Return and would hand the words in.
        let breakout = self.settings.keys() == Keys::Emacs && self.keys_capable;
        Some(String::from(match self.settings.keys() {
            Keys::Emacs if breakout => "C-j newline",
            _ if self.keys_capable => "S-⏎ newline",
            _ => "M-⏎ newline",
        }))
    }

    /// What the terminal can tell apart, as the surface last heard it.
    fn draft_encoding(&self) -> Encoding {
        if self.keys_capable {
            Encoding::Enhanced
        } else {
            Encoding::Plain
        }
    }

    /// The editor keys this terminal cannot deliver, in words: empty when every
    /// key the style binds arrives as written.
    pub fn draft_note(&self) -> String {
        if self.settings.keys() == Keys::Plain {
            return String::new();
        }
        // The table the keys will actually meet, not the editor's default one:
        // what the host binds on top is the host's to declare.
        let missing = self.draft_map().unreachable(self.draft_encoding());
        if missing.is_empty() {
            return String::new();
        }
        let said: Vec<String> = missing
            .iter()
            .take(3)
            .map(|entry| format!("{} {}", speak(&entry.key), entry.why))
            .collect();
        format!("this terminal cannot send {}", said.join("; "))
    }

    /// The draft as the glass will draw it.
    pub fn draft_placed(&self, width: usize, rows: usize) -> Option<crate::draft::Placed> {
        let Mode::Input(input) = &self.mode else {
            return None;
        };
        // The furniture keeps one column of margin; the draft is furniture
        // while it is being written, so it keeps the same column.
        let (label, _) = self.prompt_parts();
        let label = format!(" {label}");
        let hint = self.draft_hint();
        Some(draft::place(&input.draft, &label, hint.as_deref(), width, rows))
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
                let rows: Vec<(String, String, Option<&str>)> =
                    self.endpoint_choices().iter().map(|e| self.endpoint_row(e)).collect();
                // The house is the first column and every row has one, so the
                // column is as wide as the longest name in this list rather
                // than a guess. Addresses line up under each other for the
                // same reason: the eye reads a column, not a paragraph.
                let width =
                    rows.iter().map(|(house, _, _)| crate::fit::width_of(house)).max().unwrap_or(0);
                let mut lines: Vec<String> = rows
                    .iter()
                    .map(|(house, address, note)| {
                        let pad = " ".repeat(width - crate::fit::width_of(house));
                        match note {
                            Some(note) => format!("{house}{pad}  {address}  ({note})"),
                            None => format!("{house}{pad}  {address}"),
                        }
                    })
                    .collect();
                // The last row is not an address at all: it is the act of
                // naming one the list does not have.
                lines.push(String::from("enter an address of your own"));
                lines
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

    /// What an endpoint row shows: the house the address belongs to, the
    /// address itself, and the one thing neither can say -- that this is the
    /// place the process was launched against, which `-` names in the grammar
    /// and a sentence names on screen. The house is read off the address
    /// rather than stored, so the word is the one the header and the key
    /// lookup use; a host that belongs to no company is this machine.
    fn endpoint_row(&self, endpoint: &str) -> (String, String, Option<&'static str>) {
        if endpoint.is_empty() {
            let launch = pearl_core::catalog::normalize_endpoint(&self.launch);
            // Which address the launch endpoint is, when this surface was
            // told: a row that says only "the launch" makes the human guess.
            return match launch.is_empty() {
                true => (String::from("-"), String::from("the launch endpoint"), None),
                false => (self.house(&launch), launch, Some("the launch endpoint")),
            };
        }
        (self.house(endpoint), endpoint.to_string(), None)
    }

    /// The house an address belongs to, as one word the human already meets in
    /// the header and in the name of the key: `api.deepseek.com` is `deepseek`.
    fn house(&self, endpoint: &str) -> String {
        pearl_core::catalog::house_of(endpoint).unwrap_or_else(|| String::from("local"))
    }

    /// Where the open list starts: on the value in force, so choosing nothing
    /// and pressing Enter is not a change. The mark on the screen is the same
    /// answer -- a list that marked one row and started on another would make
    /// the first press a jump.
    pub(crate) fn picker_start(&self) -> usize {
        match self.pick_kind {
            Pick::Model => self
                .settings
                .candidates()
                .iter()
                .position(|c| *c == self.settings.chosen())
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
        self.mode = Mode::Input(Input::new(InputKind::ModelOf { row, endpoint }, model));
    }

    /// The first stage, typed: an address the list did not have.
    fn open_endpoint_stage(&mut self, row: Option<usize>) {
        let endpoint = row
            .and_then(|n| self.settings.candidates().get(n).map(|c| c.endpoint.clone()))
            .unwrap_or_default();
        self.notice = Some(String::from("the address requests from this row are sent to"));
        self.mode = Mode::Input(Input::new(InputKind::Endpoint(row), endpoint));
    }

    fn prompt(&mut self, kind: InputKind) {
        // One draft at a time: a second one would replace words the human may
        // still want, so the surface says where they are instead.
        if matches!(self.mode, Mode::Input(_)) {
            self.notice =
                Some(String::from("a draft is open: Tab back to it, or ␛ to drop it"));
            return;
        }
        let buffer = match &kind {
            InputKind::EditHand => self.hand_title().unwrap_or_default(),
            // Rewriting starts from what the task says now: an edit of one
            // clause is not worth an empty line.
            InputKind::Redirect(id) => {
                self.queue.get(*id).map(|n| n.title.clone()).unwrap_or_default()
            }
            _ => String::new(),
        };
        self.mode = Mode::Input(Input::new(kind, buffer));
    }

    /// Put words back on the line, put down, after the queue refused them.
    ///
    /// A refusal that eats the sentence makes the human type it twice to learn
    /// something once; kept, the words are there for a retry or a different
    /// task, and `␛` still lets them go.
    fn keep_draft(&mut self, kind: InputKind, text: String) {
        let mut input = Input::new(kind, text);
        input.focused = false;
        self.mode = Mode::Input(input);
        if let Some(notice) = &self.notice {
            self.notice = Some(format!("{notice} -- the words are kept: Tab back to them"));
        }
    }

    fn commit_input(&mut self) {
        let Mode::Input(input) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        let text = input.draft.into_text();
        match input.kind {
            InputKind::Topic => {
                self.place_in_view("topic", Request::topic(text), Position::Last);
            }
            InputKind::Append => {
                // Append means the end of the queue. Placing after the focus
                // put each new task beside the cursor, so the list read in a
                // different order from the one the human typed.
                self.place_in_view("append", Request::task(text), Position::Last);
            }
            InputKind::InsertNow => {
                self.place_in_view("insert", Request::task(text), Position::Now);
            }
            InputKind::Steer(id) => {
                let outcome = self.queue.steer(id, text.clone());
                if self.did("steer", outcome).is_none() {
                    // The task changed while the words were being typed: the
                    // refusal is the news, not a reason to eat the sentence.
                    self.keep_draft(InputKind::Steer(id), text);
                }
            }
            InputKind::Answer(id) => {
                // Answering ends the refusal: the next question this task
                // asks is a new one, and refusing it too would be silencing
                // a conversation the human just joined.
                self.declined.retain(|held| *held != id);
                let outcome = self.queue.answer(id, text);
                self.did("answer", outcome);
            }
            InputKind::Nest(id) => {
                let outcome = self.queue.add_child(id, text.clone()).map(|(_, events)| events);
                if self.did("nest", outcome).is_none() {
                    self.keep_draft(InputKind::Nest(id), text);
                }
            }
            InputKind::Redirect(id) => {
                let outcome = self.queue.redirect(id, text.clone());
                if self.did("rewrite", outcome).is_none() {
                    self.keep_draft(InputKind::Redirect(id), text);
                }
            }
            InputKind::EditHand => {
                // The node is in the tree while it is in the hand, so editing
                // its words is renaming it -- which is also what `r` does to a
                // heading: a topic is a label, and naming it is the edit.
                if let Some(id) = self.hand {
                    let outcome = self.queue.retitle(id, text);
                    self.did("edit", outcome);
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

    /// Put a task where the act says, and look at it. The human named this
    /// task, so it is the one in front of them: the list puts it where the act
    /// says and the cursor goes with it. Leaving the cursor behind meant the
    /// report beside the list described the task before the one just typed.
    fn place_in_view(&mut self, verb: &str, request: Request, position: Position) {
        let outcome = self.queue.place(request, position);
        if let Ok((id, _)) = &outcome {
            let _ = self.queue.focus_on(*id);
        }
        self.did(verb, outcome);
    }

    fn place_hand(&mut self, position: Position) {
        let Some(id) = self.hand.take() else {
            self.notice = Some(String::from("place: the hand is empty; take a task first"));
            return;
        };
        // A place is a place on the screen: with the cursor on a heading, the
        // hand goes inside it -- the row the human is pointing at is the place
        // they mean.
        // Lowercase is the near place, uppercase the far one: `p` over a
        // heading goes inside it, `P` still means the front of the whole list.
        let position = match (position, self.focused_form()) {
            (Position::After, Some((id, Form::Topic))) => Position::Into(id),
            _ => position,
        };
        let outcome = self.queue.place_node(id, position);
        if self.did("place", outcome).is_none() {
            // A refused placement must not swallow the hand: the node stays
            // lifted, and it is still theirs to put somewhere else.
            self.hand = Some(id);
        }
    }

    /// The words of the thing in the hand, read from the tree: the hand holds
    /// a node, so the footer always shows what the node says now.
    fn hand_title(&self) -> Option<String> {
        self.hand.and_then(|id| self.queue.get(id)).map(|n| n.title.clone())
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
    /// What the row calls this key, given the task the cursor is on.
    ///
    /// The row is the keyboard of one task: a key that the queue would refuse
    /// is not offered, and a key whose work depends on the state is called what
    /// it does to this one. The table in `pearl-core` decides whether a verb
    /// would be taken, so the row cannot promise what the queue would refuse;
    /// the words are the surface's own, because "steer" and "answer" are two
    /// readings of one key and only the screen knows which reading applies.
    ///
    /// With nothing focused there is no task to describe, so the row falls back
    /// to what a queued task offers -- the shape the human meets first.
    fn row_label(&self, verb: &Verb) -> Option<String> {
        use pearl_core::{NodeState, Verb as Act};
        let Some(row) = verb.row else { return None };
        let Some(id) = self.focused() else {
            // Nothing is focused, so the row shows the shape a queued task
            // offers: the keys a list with work in it answers to.
            return match verb.keys {
                "R" => None,
                _ => Some(String::from(row)),
            };
        };
        let found = self.queue.get(id);
        let label = match verb.keys {
            "t" => self.queue.can(id, Act::Take).then_some(row),
            "s" => match found.map(|n| n.state) {
                Some(NodeState::AwaitingInput) => Some("s answer"),
                _ if self.queue.can(id, Act::Steer) => Some("s steer"),
                _ => None,
            },
            "r" => self.queue.can(id, Act::Rewrite).then_some("r rewrite"),
            "c" => {
                if self.queue.can(id, Act::Cancel) {
                    Some("c cancel")
                } else if self.queue.can(id, Act::Drop) {
                    Some("c drop")
                } else {
                    None
                }
            }
            "␠" => {
                // Run first: a stopped task is one key from going on, and a
                // held queued task is stopped too -- hold would be a no-op.
                if self.queue.can(id, Act::Run) {
                    Some("␠ run")
                } else if self.queue.can(id, Act::Hold) {
                    Some("␠ pause")
                } else {
                    None
                }
            }
            "R" => self.queue.can(id, Act::Retry).then_some(row),
            _ => Some(row),
        };
        label.map(String::from)
    }

    pub fn key_groups(&self) -> Vec<(&'static str, Vec<String>)> {
        let mut groups: Vec<(&'static str, Vec<String>)> = Vec::new();
        for verb in VERBS.iter().filter(|v| !v.tail) {
            // The row carries what acts on the queue; what moves a window is
            // written on that window's own rule, so the look seat here is only
            // whatever verbs the row has room for (today, none).
            let Some(label) = self.row_label(verb) else { continue };
            match groups.iter_mut().find(|(name, _)| *name == verb.group) {
                Some((_, items)) => items.push(label),
                None => groups.push((verb.group, vec![label])),
            }
        }
        groups
    }

    /// The two keys the row keeps at its end: the way to the whole table and
    /// the way out. They are words the row is not allowed to drop, so a narrow
    /// window loses readings before it loses the exit.
    pub fn tail_items(&self) -> Vec<String> {
        // The way in is the one key the table cannot teach, because the table
        // is what it opens; the way out is a row like any other.
        let mut items = vec![String::from("? keys")];
        items.extend(VERBS.iter().filter(|v| v.tail).filter_map(|v| v.row.map(String::from)));
        items
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
    /// The report's page, spelled the way this terminal can send it. Shift
    /// beside Ctrl is a fact about the terminal, not about this program: the
    /// enhanced encoding carries it and a legacy one drops it, where the same
    /// page up is the capital letter under Alt.
    fn emacs_page(&self) -> (&'static str, &'static str) {
        if self.keys_capable { ("C-M-v", "C-M-S-v") } else { ("C-M-v", "M-P") }
    }

    /// The report's ends, spelled the way this terminal can send them. A
    /// control byte has no room for a shift and a bracket has no control byte,
    /// so `C-M-<` exists only where the encoding names the modifiers instead.
    fn emacs_ends(&self) -> (&'static str, &'static str) {
        if self.keys_capable { ("C-M-<", "C-M->") } else { ("M-<", "M->") }
    }

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
                row("queue", "⇧⇩ one task · PgUp|PgDn one screen of them · Home|End the first task and the last",
                    "队列  ⇧⇩ 上下一个任务 · PgUp|PgDn 翻一屏 · Home|End 第一个任务和最后一个"),
                row("report", "Alt+⇧⇩ one line · Alt+PgUp|PgDn one screen · Alt+Home|End the oldest line and back to the newest",
                    "报告  Alt+⇧⇩ 上下滚动一行 · Alt+PgUp|PgDn 翻一屏 · Alt+Home|End 最旧的一行和回到最新"),
                row("level", "⇦|u|⌫ out to the parent task · ⇨|⏎ in to the tasks inside",
                    "层级  ⇦|u|⌫ 回到上一层 · ⇨|⏎ 进入子任务"),
            ],
            Keys::Vim => vec![
                row("queue", "j|k one task · f|b one screen of them · C-d|C-u half a screen · gg|G the first task and the last",
                    "队列  j|k 上下一个任务 · f|b 翻一屏 · C-d|C-u 翻半屏 · gg|G 第一个任务和最后一个"),
                row("report", "M-j|M-k one line · M-f|M-b one screen · M-C-d|M-C-u half a screen · M-gg|M-G the oldest line and back to the newest",
                    "报告  M-j|M-k 上下滚动一行 · M-f|M-b 翻一屏 · M-C-d|M-C-u 翻半屏 · M-gg|M-G 最旧的一行和回到最新"),
                row("level", "h|u|⌫ out to the parent task · l|⏎ in to the tasks inside",
                    "层级  h|u|⌫ 回到上一层 · l|⏎ 进入子任务"),
            ],
            Keys::Emacs => {
                let (page_down, page_up) = self.emacs_page();
                let (oldest, newest) = self.emacs_ends();
                vec![
                    row("queue", "C-n|C-p one task · C-v|M-v one screen of them · M-/|M-\\ the first task and the last",
                        "队列  C-n|C-p 上下一个任务 · C-v|M-v 翻一屏 · M-/|M-\\ 第一个任务和最后一个"),
                    row("report", &format!("M-n|M-p one line · {page_down}|{page_up} one screen · {oldest}|{newest} the oldest line and back to the newest · C-l jump back to the newest"),
                        &format!("报告  M-n|M-p 上下滚动一行 · {page_down}|{page_up} 翻一屏 · {oldest}|{newest} 最旧的一行和回到最新 · C-l 直接回到最新")),
                    row("level", "^|u|⌫ out to the parent task · ⏎ in to the tasks inside",
                        "层级  ^|u|⌫ 回到上一层 · ⏎ 进入子任务"),
                ]
            }
        };
        // The editor's keys are the editor's: `pearl-text`, `pearl-emacs` and
        // `pearl-vim` each carry a help of their own, and this screen names the
        // keys this surface answers. A row repeating them was a second copy
        // that went stale the moment a table changed.
        let look = look;
        let mut groups: Vec<(&'static str, &'static str, Vec<HelpRow>)> = Vec::new();
        for verb in VERBS {
            let (en, zh) = group_names(verb.group);
            match groups.iter_mut().find(|(name, _, _)| *name == en) {
                Some((_, _, rows)) => rows.push(row(verb.keys, verb.en, verb.zh)),
                None => {
                    let mut rows = if en == "look" { look.clone() } else { Vec::new() };
                    rows.push(row(verb.keys, verb.en, verb.zh));
                    groups.push((en, zh, rows));
                }
            }
        }
        groups
    }

    /// How many lines the key screen has, so scrolling knows where it ends.
    pub fn help_len(&self) -> usize {
        let groups: usize = self.help_groups().iter().map(|(_, _, e)| 1 + e.len()).sum();
        1 + groups + self.place_items().len()
    }

    /// Move through the key screen. True when the key was a movement, so the
    /// caller knows the human is still reading rather than leaving.
    /// The request in front of the cursor, which is what the queue row cuts.
    pub fn request_text(&self) -> String {
        self.queue
            .focus()
            .and_then(|id| self.queue.get(id))
            .map(|node| node.title)
            .unwrap_or_default()
    }

    /// How far the request screen may move. The wrapped height is only known
    /// where the width is, so this counts whole characters as a floor: the
    /// screen clamps to its own lines when it draws.
    pub fn scroll_read(&mut self, key: KeyEvent) -> bool {
        let style = self.settings.keys();
        let alt = meta(key.modifiers);
        let line = match key.code {
            KeyCode::Down => 1,
            KeyCode::Up => -1,
            KeyCode::PageDown => 10,
            KeyCode::PageUp => -10,
            KeyCode::Home => isize::MIN / 2,
            KeyCode::End => isize::MAX / 2,
            KeyCode::Char('j') if style == Keys::Vim && !alt => 1,
            KeyCode::Char('k') if style == Keys::Vim && !alt => -1,
            _ => return false,
        };
        let at = self.read_at as isize + line;
        self.read_at = at.max(0) as usize;
        true
    }

    fn scroll_help(&mut self, key: KeyEvent) -> bool {
        let style = self.settings.keys();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = meta(key.modifiers);
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

    /// Take pasted text as text. One break is one break: `\r\n` is how
    /// Windows writes it, and turning a break into a Return would commit the
    /// prompt and leave the rest of the paste to be read as keys. With no
    /// prompt open there is nothing a paste is an answer to, so it places
    /// nothing.
    pub fn paste(&mut self, text: &str) {
        let Mode::Input(input) = &mut self.mode else {
            return;
        };
        // The breaks are the text's own now -- a draft may hold a paragraph.
        // A tab has no column to mean anything in, so it becomes one space.
        let text = text.replace("\r\n", "\n").replace('\r', "\n").replace('\t', " ");
        input.draft.insert(&text);
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

    /// What moves each window, in the style the human chose. The queue's rule
    /// names the keys that move the cursor through the tree; the report's rule
    /// names the keys that move the window down the answer. The row stopped
    /// naming them for the same reason it stopped naming the knobs: a key
    /// printed on the window it moves is learned where the eye already is, and
    /// the row's width goes back to the operations that need it.
    pub fn motion_hints(&self) -> (String, String) {
        match self.settings.keys() {
            Keys::Plain => (
                String::from("⇧⇩ task · PgUp|PgDn screen · Home|End ends · ⇦⇨ level"),
                String::from("Alt+⇧⇩ line · Alt+PgUp|PgDn screen · Alt+Home|End ends"),
            ),
            Keys::Vim => (
                String::from("j|k task · f|b screen · C-d|C-u half · gg|G ends · h|l level"),
                String::from("M-j|M-k line · M-f|M-b screen · M-C-d|M-C-u half · M-gg|M-G ends"),
            ),
            Keys::Emacs => {
                let (page_down, page_up) = self.emacs_page();
                let (oldest, newest) = self.emacs_ends();
                (
                    String::from("C-n|C-p task · C-v|M-v screen · M-/|M-\\ ends · ^|⏎ level"),
                    format!("M-n|M-p line · {page_down}|{page_up} screen · {oldest}|{newest} ends · C-l newest"),
                )
            }
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
        let (label, body) = self.prompt_parts();
        format!("{label}{body}")
    }

    /// The prompt split where a narrow window may cut it: the label says which
    /// act is being typed and survives, and the body is what the human is
    /// typing and yields from the front.
    pub fn prompt_parts(&self) -> (String, String) {
        match &self.mode {
            Mode::Input(input) => {
                let body = input.draft.text().to_string();
                let label = match &input.kind {
                    InputKind::Append => String::from("append: "),
                    InputKind::InsertNow => String::from("insert now: "),
                    InputKind::Topic => String::from("new topic: "),
                    InputKind::Steer(_) => String::from("steer: "),
                    InputKind::Answer(_) => String::from("answer: "),
                    InputKind::Nest(id) => {
                        let parent = self
                            .queue
                            .get(*id)
                            .map(|n| n.title)
                            .unwrap_or_else(|| String::from("this task"));
                        // Say what nesting does, not just where it goes: the
                        // part runs first and the whole reads what it found.
                        format!("part of \"{parent}\" (runs before it): ")
                    }
                    InputKind::Redirect(_) => String::from("rewrite: "),
                    InputKind::EditHand => String::from("edit hand: "),
                    InputKind::Aside => String::from("btw: "),
                    InputKind::Config => String::from("set: "),
                    InputKind::Endpoint(None) => String::from("add model: endpoint: "),
                    InputKind::Endpoint(Some(n)) => format!("edit model {}: endpoint ", n + 1),
                    InputKind::ModelOf { row: None, .. } => String::from("add model: model id: "),
                    InputKind::ModelOf { row: Some(n), .. } => {
                        format!("edit model {}: model id ", n + 1)
                    }
                };
                (label, body)
            }
            Mode::Normal => match self.hand_title() {
                Some(title) => (String::new(), format!("in hand: {title}")),
                None => {
                    let tail = self.tail_items().join(" · ");
                    (String::new(), format!("{} · {tail}", self.key_items().join(" · ")))
                }
            },
        }
    }
}

/// Meta is a modifier, not a key. A terminal that speaks the enhanced
/// encoding sends Command as `SUPER` and Option as `ALT`, and the row calls
/// both of them Meta: `M-` is the name, and either key answers it. A terminal
/// that does not speak the encoding sends only Option, which is the same
/// modifier under the only name it has -- so nothing here has to know which
/// terminal it is running in.
/// The key as the legacy encoding would have sent it.
///
/// A terminal may spell a shifted key two ways. The legacy encoding puts the
/// capital or the symbol on the wire and leaves Shift out; the kitty one names
/// the key itself and sets SHIFT instead. Every arm in this file spells the
/// first way, because that is what the help offers and what a terminal without
/// the encoding sends, so the second is read as the first before an arm can
/// look at it. Without this, `Shift+/` opened an aside instead of the help and
/// `Shift+G` started a `g` that was waiting for its second letter.
fn spelled(key: KeyEvent) -> KeyEvent {
    if !key.modifiers.contains(KeyModifiers::SHIFT) {
        return key;
    }
    let KeyCode::Char(c) = key.code else {
        return key;
    };
    let Some(symbol) = shifted(c) else {
        return key;
    };
    KeyEvent {
        code: KeyCode::Char(symbol),
        modifiers: key.modifiers - KeyModifiers::SHIFT,
        ..key
    }
}

/// What Shift makes of a key. Not every keyboard puts these where this table
/// does -- a layout that reaches `{` by another key sends the symbol itself and
/// never asks -- so a key the table does not know is left as the terminal
/// spelled it rather than guessed at.
fn shifted(c: char) -> Option<char> {
    if c.is_ascii_lowercase() {
        return Some(c.to_ascii_uppercase());
    }
    Some(match c {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        _ => return None,
    })
}

fn meta(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::ALT) || modifiers.contains(KeyModifiers::SUPER)
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
        QueueError::Empty => String::from("a task needs a word"),
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

/// The terminal's key, as an editor reads it. One `From`-shaped function is
/// the whole of the dependency between the surface's terminal library and the
/// editors, which is why the editors do not depend on one.
/// A key as the help screen writes it: `C-j`, `M-⏎`, `S-⏎`, `⏎`.
fn speak(key: &Key) -> String {
    let mut said = String::new();
    if key.ctrl {
        said.push_str("C-");
    }
    if key.alt {
        said.push_str("M-");
    }
    if key.shift && !matches!(key.code, Code::Char(_)) {
        said.push_str("S-");
    }
    said.push_str(&match key.code {
        Code::Char(character) => character.to_string(),
        Code::Backspace => String::from("⌫"),
        Code::Delete => String::from("Del"),
        Code::Left => String::from("⇦"),
        Code::Right => String::from("⇨"),
        Code::Up => String::from("⇧"),
        Code::Down => String::from("⇩"),
        Code::Home => String::from("Home"),
        Code::End => String::from("End"),
        Code::Enter => String::from("⏎"),
        Code::Tab => String::from("Tab"),
        Code::Escape => String::from("␛"),
        Code::Other => String::from("?"),
    });
    said
}

fn as_key(key: KeyEvent, ctrl: bool, alt: bool) -> Key {
    let code = match key.code {
        KeyCode::Char(c) => Code::Char(c),
        KeyCode::Backspace => Code::Backspace,
        KeyCode::Delete => Code::Delete,
        KeyCode::Left => Code::Left,
        KeyCode::Right => Code::Right,
        KeyCode::Up => Code::Up,
        KeyCode::Down => Code::Down,
        KeyCode::Home => Code::Home,
        KeyCode::End => Code::End,
        KeyCode::Enter => Code::Enter,
        KeyCode::Tab => Code::Tab,
        KeyCode::Esc => Code::Escape,
        _ => Code::Other,
    };
    // Shift is kept for the keys whose identity it changes -- `⇧⏎` is a
    // different keystroke from `⏎`, `⇧Tab` from `Tab`. For a character it is
    // already in the character: a terminal sends `M->` as `>` wearing Shift,
    // and asking the table to know that would make every shifted letter its
    // own entry.
    let shift = !matches!(key.code, KeyCode::Char(_)) && key.modifiers.contains(KeyModifiers::SHIFT);
    Key { code, shift, ctrl, alt }
}
