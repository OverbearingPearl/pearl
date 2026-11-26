//! queue --- the persistent, editable tree of work, and its state machine.
//!
//! The queue is the product. It lives outside the model's context so compaction
//! cannot drop a task the human asked for; "don't forget" is a property of this
//! data structure, not a hope pinned on the model. The data is a tree; the view
//! is always a single-focus flat list at the current level, so the human edits
//! a list and drills in and out, never the tree itself.
//!
//! The editing model is two verbs: `place` puts a request on the queue, and
//! `take` lifts an unexecuted node back into a request. An unexecuted node is a
//! loose part -- the human edits the request in hand and places it again in
//! another form or position. `append`, `steer` and `preempt` are sugar over
//! `place`; there is no separate draft type or staging area.
//!
//! `Queue` is a handle, cheap to clone: the surface edits one clone while the
//! runner drains another, and both see one tree. Every method locks for the
//! length of its own call and no longer, so the two never wait on each other
//! for more than the read or write they do.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::event::Event;
use crate::id::TaskId;
use crate::verb::Verb;
use crate::node::{Form, Frame, NodeState, Position, Request, Steering, TaskNode};

/// An illegal transition or a reference to a node that is not there.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("no such task: {0}")]
    Unknown(TaskId),
    #[error("task {id} is {found:?}, expected {expected}")]
    WrongState { id: TaskId, found: NodeState, expected: &'static str },
    #[error("task {0} is terminal and cannot change")]
    Terminal(TaskId),
    #[error("a steering request needs a target task")]
    SteeringNeedsTarget,
    #[error("a task needs a word: an empty title is not work")]
    Empty,
}

type Result<T> = std::result::Result<T, QueueError>;

/// The task tree. Serialize it to persist a session; deserialize to resume.
///
/// Cloning the queue does not copy the tree: every clone points at the same
/// one, which is what lets the surface and the runner be the same session.
#[derive(Debug, Clone, Default)]
pub struct Queue {
    tree: Arc<Mutex<Tree>>,
}

/// The state a queue holds, behind its lock.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Tree {
    nodes: HashMap<TaskId, TaskNode>,
    roots: Vec<TaskId>,
    focus: Option<TaskId>,
    /// Narrow-to-subtree: `None` shows the top level, `Some(id)` shows that
    /// node's children. The view is flat either way.
    scope: Option<TaskId>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// The one lock. A panic that poisoned the tree did it after this was
    /// called, so the state is still consistent: recover it rather than
    /// handing every later caller an error they cannot act on.
    fn tree(&self) -> MutexGuard<'_, Tree> {
        self.tree.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A snapshot of one node. A copy, not a borrow: the surface reads it while
    /// the runner may be about to change it.
    pub fn get(&self, id: TaskId) -> Option<TaskNode> {
        self.tree().nodes.get(&id).cloned()
    }

    pub fn len(&self) -> usize {
        self.tree().nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree().nodes.is_empty()
    }

    pub fn focus(&self) -> Option<TaskId> {
        self.tree().focus
    }

    pub fn scope(&self) -> Option<TaskId> {
        self.tree().scope
    }

    // --- the two editing verbs: place / take -------------------------------

    /// Place a request on the queue. A `Task` form becomes a node at the
    /// position; a `Steering` form annotates its target. This is the primitive
    /// every convenience below delegates to.
    pub fn place(&self, request: Request, position: Position) -> Result<(TaskId, Vec<Event>)> {
        self.tree().place(request, position)
    }

    /// Lift an unexecuted node off the queue, keeping the node whole.
    ///
    /// The hand holds the node, not a request: a topic taken out of the list
    /// keeps its parts, and putting it back puts the whole line of work back.
    /// Only a `Queued` node can be taken, and not while something inside it is
    /// running -- work in flight is not moved. One that has started is steered
    /// or held instead.
    pub fn take(&self, id: TaskId) -> Result<(TaskId, Vec<Event>)> {
        self.tree().take(id)
    }

    /// Put the node in the hand back on the list, keeping its parts.
    pub fn place_node(&self, id: TaskId, position: Position) -> Result<Vec<Event>> {
        self.tree().place_node(id, position)
    }

    /// Rename a node: what `⏎` does to the thing in the hand.
    pub fn retitle(&self, id: TaskId, title: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().retitle(id, title)
    }

    /// Let a lifted node go: it leaves the list and takes its parts with it.
    pub fn discard(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().discard(id)
    }

    // --- conveniences over place -------------------------------------------

    /// Append a new top-level task (follow-up). It enters `Queued`.
    pub fn append(&self, title: impl Into<String>) -> (TaskId, Vec<Event>) {
        self.tree().append(title)
    }

    /// Nest a child task under a parent (a subagent's work is a subtree).
    pub fn add_child(&self, parent: TaskId, title: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.tree().add_child(parent, title)
    }

    /// Add one turn to a task: its bill, and what it was doing. Presentation,
    /// like `note`: a task that has already settled still gets its bill, and a
    /// missing id is ignored. A caller that knows only the cost charges a bare
    /// `Cost` and the turn is written without a note.
    pub fn charge(&self, id: TaskId, turn: impl Into<crate::Turn>) {
        let turn = turn.into();
        if let Some(node) = self.tree().nodes.get_mut(&id) {
            node.cost.add(turn.cost);
            node.turns.push(turn);
        }
    }

    /// Annotate a running or queued task. The message waits in the steering log
    /// until the next safe boundary; it does not interrupt mid-token.
    pub fn steer(&self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().steer(id, text)
    }

    /// Drain the pending steering at a boundary. The scheduler calls this when
    /// the loop reaches a safe point; each drained message is an injection.
    pub fn drain_steering(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().drain_steering(id)
    }

    /// Preempt: hold the running task, then place an urgent task at the front.
    /// Sugar over `hold` + `place(Now)`; the paused task resumes when the
    /// urgent one settles.
    pub fn preempt(&self, id: TaskId, urgent_title: impl Into<String>, progress: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.tree().preempt(id, urgent_title, progress)
    }

    /// Hold a task: stop it without abandoning it. On a running task this saves
    /// the frame and pauses; on a queued task it sets `held` so the scheduler
    /// skips it. Resume reverses either.
    pub fn hold(&self, id: TaskId, progress: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().hold(id, progress)
    }

    /// Resume a held task: a paused one restores its frame and runs; a held
    /// queued one becomes dispatchable again.
    pub fn run(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().run(id)
    }

    /// Whether `verb` would be accepted by the task `id`, for a surface that
    /// writes the key on the row only while it would work. An unknown id takes
    /// nothing, which is the answer a key pressed on an empty list deserves.
    pub fn can(&self, id: TaskId, verb: Verb) -> bool {
        self.get(id)
            .map(|n| verb.accepts_form(n.state, n.held, n.form))
            .unwrap_or(false)
    }

    /// Which heading each task sits under now, by id.
    ///
    /// A turn belongs to the task it was spoken for, and the task belongs to
    /// wherever it sits on the list. This is that second half, read by the
    /// view: it is why moving a finished task under another heading moves its
    /// record too, instead of leaving turns in a conversation nobody reads.
    pub fn owners(&self) -> std::collections::HashMap<String, String> {
        self.tree().owners()
    }

    /// The heading this node's work belongs to, if any: the nearest ancestor
    /// that is a topic.
    pub fn heading_of(&self, id: TaskId) -> Option<TaskId> {
        let tree = self.tree();
        let mut at = tree.nodes.get(&id).and_then(|n| n.parent);
        while let Some(parent) = at {
            let node = tree.nodes.get(&parent)?;
            if node.form == Form::Topic {
                return Some(parent);
            }
            at = node.parent;
        }
        None
    }

    /// Every task under a heading, as the strings the log and the ledger use.
    pub fn tasks_under(&self, id: TaskId) -> Vec<String> {
        self.tree().descendants(id).into_iter().map(|d| d.to_string()).collect()
    }

    /// A new topic at the end of the roots: a heading for a line of work.
    pub fn topic(&self, title: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.tree().place(Request::topic(title), Position::Last)
    }

    /// Rewrite a live task in place: replace its objective and preserve the
    /// work done. A running task also gets a re-plan at the boundary. Distinct
    /// from steer (additive) and from cancel+append (which throws the progress
    /// away), and it keeps the task where it is instead of moving it through
    /// the hand.
    pub fn redirect(&self, id: TaskId, new_title: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().redirect(id, new_title)
    }

    /// Dispatch a queued task into the running state.
    pub fn start(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().start(id)
    }

    /// Say what the task is doing right now. Only live work has an act; the
    /// note is the newest one, and the report replaces it when the task ends.
    pub fn note(&self, id: TaskId, act: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().note(id, act)
    }

    /// Tell the human what the environment did to this task: the sandbox
    /// refused a write, or history was folded into a summary. Unlike the
    /// activity note it outlives the answer, because the answer was shaped by
    /// it. An unknown task is the only refusal: the news is about something
    /// that happened, so no state is wrong to carry it.
    pub fn notice(&self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().notice(id, text)
    }

    /// Keep what the model worked out before answering. Presentation, like a
    /// note: a cancelled node refuses it and the work goes on.
    pub fn thought(&self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().thought(id, text)
    }

    /// The agent asks a question; the task waits for an answer.
    pub fn ask(&self, id: TaskId, question: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().ask(id, question)
    }

    /// Answer the question; the answer is injected as steering and it runs.
    pub fn answer(&self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().answer(id, text)
    }

    /// Record a result. The task finishes only when every child has settled;
    /// otherwise it stays running, waiting on its subtree.
    pub fn complete(&self, id: TaskId, result: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().complete(id, result)
    }

    /// End a task in error; it may be retried.
    pub fn fail(&self, id: TaskId, error: impl Into<String>) -> Result<Vec<Event>> {
        self.tree().fail(id, error)
    }

    /// Retry a failed task back to the queue.
    pub fn retry(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().retry(id)
    }

    /// Cancel a task and, recursively, its unfinished children.
    /// Forget a settled task and everything under it. A finished row is the
    /// human's reading material, but it is not a fact that must stay on the
    /// list for ever: this is the only removal there is, and it refuses a task
    /// that is still alive, so no work is lost by pressing it too early.
    pub fn withdraw(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().withdraw(id)
    }

    pub fn cancel(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().cancel(id)
    }

    /// Take a topic apart: its parts stand where it stood, in their order.
    ///
    /// Only while none of them has run. A turn that ran under a heading
    /// belongs to that heading, and moving it afterwards would make the log
    /// say the work was done somewhere it was not.
    pub fn dissolve(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().dissolve(id)
    }

    /// Make a task into a heading for a line of work, keeping its parts.
    ///
    /// Only work that has not started can become one: a task that has asked
    /// the model for words already has turns, and those turns belong to the
    /// conversation they were spoken in -- the shared line.
    pub fn promote(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().promote(id)
    }

    // --- the single-focus view ---------------------------------------------

    /// Move the focus cursor to a node.
    pub fn focus_on(&self, id: TaskId) -> Result<()> {
        self.tree().focus_on(id)
    }

    /// Narrow the view to a node's children (drill in).
    pub fn narrow(&self, id: TaskId) -> Result<()> {
        self.tree().narrow(id)
    }

    /// Widen the view one level (drill out).
    pub fn unnarrow(&self) {
        self.tree().unnarrow()
    }

    /// The flat list at the current level: the roots, or the scoped node's
    /// children. This is what the surface renders and the human edits.
    pub fn view(&self) -> Vec<TaskNode> {
        self.tree().view()
    }

    /// The next dispatchable queued task at the top level, in order. A held
    /// task is skipped until resumed.
    /// The queue as words, so a log can carry it to a run that has not
    /// started yet. It is the same form `Serialize` writes.
    pub fn snapshot(&self) -> String {
        serde_json::to_string(&*self.tree()).unwrap_or_default()
    }

    /// Put a snapshot back, holding every task that had not settled. A restore
    /// never starts work: the human comes back to the list they left, and says
    /// which row goes on. Returns how many tasks came back.
    pub fn restore(&self, snapshot: &str) -> Option<usize> {
        let tree: Tree = serde_json::from_str(snapshot).ok()?;
        let held = tree.nodes.values().filter(|node| !node.is_settled()).count();
        let mut guard = self.tree();
        *guard = tree;
        for node in guard.nodes.values_mut() {
            if node.is_settled() {
                continue;
            }
            // Nothing is running it any more: a row that says "asking the
            // model" on a launch that has asked nothing is the screen lying
            // about who is working.
            if node.state == NodeState::Running {
                node.state = NodeState::Queued;
            }
            node.held = true;
        }
        Some(held)
    }

    pub fn next_queued(&self) -> Option<TaskId> {
        self.tree().next_queued()
    }
}

/// The queue as data: a snapshot through the lock, taken at serialization.
impl Serialize for Queue {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.tree().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Queue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let tree = Tree::deserialize(deserializer)?;
        Ok(Self { tree: Arc::new(Mutex::new(tree)) })
    }
}

impl Tree {
    fn node(&self, id: TaskId) -> Result<&TaskNode> {
        self.nodes.get(&id).ok_or(QueueError::Unknown(id))
    }

    fn node_mut(&mut self, id: TaskId) -> Result<&mut TaskNode> {
        self.nodes.get_mut(&id).ok_or(QueueError::Unknown(id))
    }

    fn expect(&self, id: TaskId, expected: NodeState, label: &'static str) -> Result<()> {
        let n = self.node(id)?;
        if n.state == expected {
            Ok(())
        } else {
            Err(QueueError::WrongState { id, found: n.state, expected: label })
        }
    }

    fn set_state(&mut self, id: TaskId, to: NodeState) -> Result<Event> {
        let n = self.node_mut(id)?;
        let from = n.state;
        n.state = to;
        Ok(Event::StateChanged { id, from, to })
    }

    fn place(&mut self, request: Request, position: Position) -> Result<(TaskId, Vec<Event>)> {
        // Nothing typed is not a task. A row with no text costs a model request
        // that has no question in it, and on a resume it is a row nobody can
        // read either. The door a human types into refuses here, so every
        // caller -- the surface, a child, `pearl run` -- refuses with it.
        if request.title.trim().is_empty() {
            return Err(QueueError::Empty);
        }
        match request.form {
            Form::Steering => {
                let target = request.target.ok_or(QueueError::SteeringNeedsTarget)?;
                self.steer(target, request.title)?;
                Ok((target, vec![]))
            }
            Form::Task | Form::Topic => {
                let id = TaskId::new();
                let title = request.title.clone();
                let form = request.form;
                match position {
                    Position::Into(parent) => {
                        self.node(parent)?;
                        let mut node =
                            TaskNode::with_form(id, Some(parent), title.clone(), form);
                        node.parent = Some(parent);
                        self.nodes.insert(id, node);
                        self.node_mut(parent)?.children.push(id);
                    }
                    _ => {
                        // A place is a place on the screen: the level the
                        // human is looking at is the level their words land
                        // on. Before this, `a` and `i` wrote to the roots
                        // whatever the screen showed, so a task typed inside a
                        // topic arrived outside it.
                        let level = self.scope;
                        let focus = self.focus;
                        let mut node = TaskNode::with_form(id, level, title.clone(), form);
                        node.parent = level;
                        self.nodes.insert(id, node);
                        let side = self.side(level)?;
                        match position {
                            Position::Now => side.insert(0, id),
                            Position::Last => side.push(id),
                            // After the focused task if it lives on this level,
                            // else last: a focus on another level has no place
                            // here, and the end is the one answer that lies.
                            _ => {
                                let at = focus
                                    .and_then(|f| side.iter().position(|r| *r == f))
                                    .map(|i| i + 1)
                                    .unwrap_or(side.len());
                                side.insert(at, id);
                            }
                        }
                    }
                }
                if self.focus.is_none() {
                    self.focus = Some(id);
                }
                Ok((id, vec![Event::Appended { id, title }]))
            }
        }
    }

    /// The order of one level: the roots, or a node's children.
    ///
    /// Placing a task means inserting into the list the human sees, and the
    /// list they see is the scope's children when they are inside a topic.
    fn side(&mut self, level: Option<TaskId>) -> Result<&mut Vec<TaskId>> {
        match level {
            Some(parent) => Ok(&mut self.node_mut(parent)?.children),
            None => Ok(&mut self.roots),
        }
    }

    fn take(&mut self, id: TaskId) -> Result<(TaskId, Vec<Event>)> {
        // A finished row can be lifted: the list is the human's to rearrange,
        // and a task that ran is still a row on it. Only work in flight is
        // untouchable, because something is using it.
        let found = self.node(id)?.state;
        if matches!(found, NodeState::Running | NodeState::AwaitingInput) {
            return Err(Verb::Take.wrong_state(id, found));
        }
        let flying = self
            .descendants(id)
            .into_iter()
            .filter_map(|d| self.nodes.get(&d))
            .any(|d| matches!(d.state, NodeState::Running | NodeState::AwaitingInput));
        if flying {
            return Err(QueueError::WrongState {
                id,
                found,
                expected: "a topic with nothing in flight",
            });
        }
        // The node stays in `nodes`, and keeps its parent: a lifted topic is
        // still the heading of its parts, and putting it back is hanging the
        // whole subtree somewhere.
        // The cursor stays where the row was: the next row on that level takes
        // its place, and the one after the lift is the one the human was
        // reading. Jumping to the first root moved the screen under their
        // hands, so the place they pressed `p` for was no longer the place
        // they were looking at.
        let level = self.nodes.get(&id).and_then(|n| n.parent);
        let was = self.side(level).ok().and_then(|side| side.iter().position(|r| *r == id));
        self.detach(id);
        let side = self.side(level)?;
        let at = was.unwrap_or(0).min(side.len().saturating_sub(1));
        let next = side.get(at).or_else(|| side.last()).copied();
        if self.focus == Some(id) {
            self.focus = next;
        }
        Ok((id, vec![Event::Withdrawn { id }]))
    }

    /// Take a node out of the order it sits in, leaving the node itself alone.
    fn detach(&mut self, id: TaskId) {
        self.roots.retain(|r| *r != id);
        let parent = self.nodes.get(&id).and_then(|n| n.parent);
        if let Some(parent) = parent {
            if let Ok(p) = self.node_mut(parent) {
                p.children.retain(|c| *c != id);
            }
        }
    }

    /// A node and everything under it, in order.
    fn descendants(&self, id: TaskId) -> Vec<TaskId> {
        let mut out = vec![id];
        let mut at = 0;
        while at < out.len() {
            if let Some(node) = self.nodes.get(&out[at]) {
                out.extend(node.children.iter().copied());
            }
            at += 1;
        }
        out
    }

    fn place_node(&mut self, id: TaskId, position: Position) -> Result<Vec<Event>> {
        let node = self.node(id)?.clone();
        // Where the node goes is the caller's to say: a place is read from the
        // screen by the surface, which knows the cursor the human is on. The
        // queue does not guess from its own focus, because taking a node moves
        // that focus before the node is put back.
        let into = match position {
            Position::Into(parent) => Some(parent),
            _ => None,
        };
        if let Some(parent) = into {
            let host = self.node(parent)?.clone();
            if host.form != Form::Topic {
                return Err(Verb::Join.wrong_state(id, host.state));
            }
            if node.form == Form::Topic {
                // A heading dropped on another is a merge: the parts move in
                // and the empty heading goes. Only work that has not run can
                // change hands, and it is all or nothing -- half a line of work
                // in one conversation and half in another is two records of one
                // intention.
                return self.merge(id, parent);
            }
            // Work in flight cannot change hands: a turn is already being
            // written for it, and the row it belongs to is decided before the
            // request goes out. Everything else can move, because every turn
            // names the task it was spoken for and the view reads that name:
            // the record follows the row.
            let flying = self
                .descendants(id)
                .into_iter()
                .filter_map(|d| self.nodes.get(&d))
                .any(|n| matches!(n.state, NodeState::Running | NodeState::AwaitingInput));
            if flying || id == parent {
                return Err(Verb::Join.wrong_state(id, node.state));
            }
        }
        self.detach(id);
        match into {
            Some(parent) => {
                self.node(parent)?;
                self.node_mut(parent)?.children.push(id);
                if let Some(n) = self.nodes.get_mut(&id) {
                    n.parent = Some(parent);
                }
            }
            None => {
                let level = self.scope;
                if let Some(n) = self.nodes.get_mut(&id) {
                    n.parent = level;
                }
                let at = match position {
                    Position::Now => 0,
                    Position::Last => usize::MAX,
                    _ => self
                        .focus
                        .and_then(|f| {
                            let side = self.side(level).ok()?;
                            side.iter().position(|r| *r == f)
                        })
                        .map(|i| i + 1)
                        .unwrap_or(usize::MAX),
                };
                let side = self.side(level)?;
                side.insert(at.min(side.len()), id);
            }
        }
        self.focus = Some(id);
        Ok(vec![Event::Appended { id, title: node.title }])
    }

    fn retitle(&mut self, id: TaskId, title: impl Into<String>) -> Result<Vec<Event>> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(QueueError::Empty);
        }
        let changed = self.node_mut(id)?;
        changed.title = title.clone();
        Ok(vec![Event::Appended { id, title }])
    }

    fn discard(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.detach(id);
        for part in self.descendants(id) {
            self.nodes.remove(&part);
        }
        if self.focus == Some(id) {
            self.focus = self.roots.first().copied();
        }
        Ok(vec![Event::Withdrawn { id }])
    }

    fn append(&mut self, title: impl Into<String>) -> (TaskId, Vec<Event>) {
        // The convenience is for a title somebody wrote down, so an empty
        // one is a mistake in the caller rather than a thing a human typed:
        // the doors that take a human's words go through `place` and are
        // refused there.
        self.place(Request::task(title), Position::After).expect("a task has a word")
    }

    fn add_child(&mut self, parent: TaskId, title: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        let state = self.node(parent)?.state;
        // A finished task cannot grow work: its answer is written, and a child
        // added under it would run with nothing waiting for the result.
        if !Verb::Nest.accepts(state, false) {
            return Err(Verb::Nest.wrong_state(parent, state));
        }
        self.place(Request::task(title), Position::Into(parent))
    }

    fn steer(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        // A task waiting on an answer can still be steered: the human may want
        // to change what it was asked rather than answer it. A stopped one can
        // be steered too -- the words wait for the boundary that run brings.
        if !Verb::Steer.accepts(state, false) {
            return Err(Verb::Steer.wrong_state(id, state));
        }
        let text = text.into();
        self.node_mut(id)?.steering.push(Steering { text: text.clone() });
        Ok(vec![])
    }

    fn drain_steering(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        let drained: Vec<Steering> = std::mem::take(&mut self.node_mut(id)?.steering);
        Ok(drained
            .into_iter()
            .map(|s| Event::SteeringInjected { id, text: s.text })
            .collect())
    }

    fn preempt(&mut self, id: TaskId, urgent_title: impl Into<String>, progress: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.frame = Some(Frame { progress: progress.into() });
        let mut events = vec![self.set_state(id, NodeState::Paused)?];
        let (urgent, placed) = self.place(Request::task(urgent_title), Position::Now)?;
        events.extend(placed);
        self.focus = Some(urgent);
        events.push(Event::Preempted { paused: id, urgent });
        Ok((urgent, events))
    }

    fn hold(&mut self, id: TaskId, progress: impl Into<String>) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        match state {
            NodeState::Running => {
                self.node_mut(id)?.frame = Some(Frame { progress: progress.into() });
                Ok(vec![self.set_state(id, NodeState::Paused)?, Event::Held { id }])
            }
            NodeState::Queued if Verb::Hold.accepts(state, false) => {
                self.node_mut(id)?.held = true;
                Ok(vec![Event::Held { id }])
            }
            _ => Err(Verb::Hold.wrong_state(id, state)),
        }
    }

    fn run(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let n = self.node(id)?;
        match n.state {
            NodeState::Paused if Verb::Run.accepts(n.state, false) => {
                let events = vec![self.set_state(id, NodeState::Running)?, Event::Resumed { id }];
                Ok(events)
            }
            NodeState::Queued if Verb::Run.accepts(n.state, n.held) => {
                self.node_mut(id)?.held = false;
                Ok(vec![Event::Resumed { id }])
            }
            _ => Err(Verb::Run.wrong_state(id, n.state)),
        }
    }

    fn redirect(&mut self, id: TaskId, new_title: impl Into<String>) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        let new_title = new_title.into();
        match state {
            // A task that has not started has no loop to re-plan: the words
            // are the whole objective, and replacing them moves nothing.
            NodeState::Queued => {
                self.node_mut(id)?.title = new_title.clone();
                Ok(vec![Event::Redirected { id, title: new_title }])
            }
            // The new objective is injected as steering so the loop
            // re-plans around it at the next boundary, keeping the frame
            // and the done children.
            NodeState::Running | NodeState::Paused | NodeState::AwaitingInput => {
                let n = self.node_mut(id)?;
                n.title = new_title.clone();
                n.steering.push(Steering { text: format!("New objective: {new_title}. Re-plan around it, keeping completed work.") });
                Ok(vec![Event::Redirected { id, title: new_title }])
            }
            _ => Err(Verb::Rewrite.wrong_state(id, state)),
        }
    }

    fn start(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Queued, "queued")?;
        Ok(vec![self.set_state(id, NodeState::Running)?])
    }

    fn note(&mut self, id: TaskId, act: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.activity = Some(act.into());
        Ok(vec![])
    }

    fn notice(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        // A task that has already settled cannot learn anything new, and a line
        // under a finished answer would be a change to a record the human has
        // read. Everything else carries the news.
        if self.node(id)?.is_settled() {
            return Err(QueueError::Terminal(id));
        }
        self.node_mut(id)?.notices.push(text.into());
        Ok(vec![])
    }

    fn thought(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.reasoning = Some(text.into());
        Ok(vec![])
    }

    fn ask(&mut self, id: TaskId, question: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.question = Some(question.into());
        Ok(vec![self.set_state(id, NodeState::AwaitingInput)?])
    }

    fn answer(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::AwaitingInput, "awaiting-input")?;
        self.node_mut(id)?.question = None;
        let mut events = vec![self.set_state(id, NodeState::Running)?];
        events.extend(self.steer(id, text)?);
        Ok(events)
    }

    fn complete(&mut self, id: TaskId, result: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.result = Some(result.into());
        let settled = self.node(id)?.children_settled(&self.nodes);
        if settled {
            let mut events = vec![self.set_state(id, NodeState::Done)?];
            events.extend(self.notify_parent(id));
            Ok(events)
        } else {
            Ok(vec![])
        }
    }

    fn fail(&mut self, id: TaskId, error: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.error = Some(error.into());
        let mut events = vec![self.set_state(id, NodeState::Failed)?];
        events.extend(self.notify_parent(id));
        Ok(events)
    }

    fn retry(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        if !matches!(state, NodeState::Failed | NodeState::Cancelled | NodeState::Done) {
            return Err(Verb::Retry.wrong_state(id, state));
        }
        let n = self.node_mut(id)?;
        n.error = None;
        n.result = None;
        Ok(vec![self.set_state(id, NodeState::Queued)?])
    }

    fn promote(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let node = self.node(id)?.clone();
        if node.parent.is_some() {
            return Err(QueueError::WrongState {
                id,
                found: node.state,
                expected: "a root task",
            });
        }
        if node.form != Form::Task || !node.turns.is_empty() || node.state != NodeState::Queued {
            return Err(Verb::Join.wrong_state(id, node.state));
        }
        let node = self.node_mut(id)?;
        node.form = Form::Topic;
        Ok(vec![Event::Appended { id, title: node.title.clone() }])
    }

    /// Move every part of one heading into another, then let the empty one go.
    fn merge(&mut self, from: TaskId, into: TaskId) -> Result<Vec<Event>> {
        if from == into {
            return Err(Verb::Join.wrong_state(from, NodeState::Queued));
        }
        let parts = self.node(from)?.children.clone();
        let ran = self
            .descendants(from)
            .into_iter()
            .filter(|d| *d != from)
            .filter_map(|d| self.nodes.get(&d))
            .any(|n| n.state != NodeState::Queued || !n.turns.is_empty());
        if ran {
            let state = self.node(from)?.state;
            return Err(Verb::Join.wrong_state(from, state));
        }
        for part in &parts {
            if let Some(n) = self.nodes.get_mut(part) {
                n.parent = Some(into);
            }
        }
        if let Some(host) = self.nodes.get_mut(&into) {
            host.children.extend(parts);
        }
        self.detach(from);
        self.nodes.remove(&from);
        self.roots.retain(|r| *r != from);
        if self.focus == Some(from) {
            self.focus = Some(into);
        }
        Ok(vec![Event::Withdrawn { id: from }])
    }

    fn dissolve(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let node = self.node(id)?.clone();
        if node.form != Form::Topic {
            return Err(Verb::Dissolve.wrong_state(id, node.state));
        }
        let ran = node.children.iter().filter_map(|c| self.nodes.get(c)).any(|c| {
            c.is_settled() || !c.turns.is_empty() || c.state != NodeState::Queued
        });
        if ran || node.state != NodeState::Queued {
            return Err(Verb::Dissolve.wrong_state(id, node.state));
        }
        if self.scope == Some(id) {
            self.scope = node.parent;
        }
        // The parts take the topic's place, in their own order.
        let at = {
            let side = self.side(node.parent)?;
            side.iter().position(|r| *r == id).unwrap_or(side.len())
        };
        let parts = node.children.clone();
        for (offset, part) in parts.iter().enumerate() {
            if let Some(n) = self.nodes.get_mut(part) {
                n.parent = node.parent;
            }
            let side = self.side(node.parent)?;
            let at = at.min(side.len());
            side.insert(at + offset.min(side.len()), *part);
        }
        self.nodes.remove(&id);
        Ok(vec![Event::Withdrawn { id }])
    }

    fn withdraw(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let node = self.node(id)?.clone();
        if !node.is_settled() {
            return Err(Verb::Drop.wrong_state(id, node.state));
        }
        if self.scope == Some(id) {
            self.scope = node.parent;
        }
        if let Some(parent) = node.parent.and_then(|p| self.nodes.get_mut(&p)) {
            parent.children.retain(|c| *c != id);
        } else {
            self.roots.retain(|r| *r != id);
        }
        let mut doomed = vec![id];
        while let Some(next) = doomed.pop() {
            if let Some(held) = self.nodes.remove(&next) {
                doomed.extend(held.children);
            }
        }
        if !self.nodes.contains_key(&self.focus.unwrap_or(id)) {
            self.focus = node.parent.or_else(|| self.roots.first().copied());
        }
        Ok(vec![])
    }

    fn cancel(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let n = self.node(id)?;
        if n.is_settled() {
            return Err(QueueError::Terminal(id));
        }
        let children = n.children.clone();
        let mut events = vec![self.set_state(id, NodeState::Cancelled)?];
        for c in children {
            if let Ok(mut more) = self.cancel(c) {
                events.append(&mut more);
            }
        }
        events.extend(self.notify_parent(id));
        Ok(events)
    }

    /// When a child settles, let the parent finish if it was waiting on it.
    fn notify_parent(&mut self, child: TaskId) -> Vec<Event> {
        let Some(parent) = self.nodes.get(&child).and_then(|n| n.parent) else {
            return vec![];
        };
        let mut events = vec![Event::ChildSettled { parent, child }];
        let waiting = self
            .node(parent)
            .map(|n| {
                let parts_done = n.children_settled(&self.nodes);
                // A topic is a place, not a task: when its last part settles it
                // is done, and no request of its own was ever made for it.
                if n.form == Form::Topic {
                    return parts_done && !n.is_settled();
                }
                n.state == NodeState::Running && n.result.is_some() && parts_done
            })
            .unwrap_or(false);
        if waiting {
            if let Ok(ev) = self.set_state(parent, NodeState::Done) {
                events.push(ev);
            }
            events.extend(self.notify_parent(parent));
        }
        events
    }

    fn focus_on(&mut self, id: TaskId) -> Result<()> {
        self.node(id)?;
        self.focus = Some(id);
        Ok(())
    }

    fn narrow(&mut self, id: TaskId) -> Result<()> {
        self.node(id)?;
        self.scope = Some(id);
        self.focus = self.node(id)?.children.first().copied();
        Ok(())
    }

    fn unnarrow(&mut self) {
        self.scope = match self.scope {
            Some(id) => self.nodes.get(&id).and_then(|n| n.parent),
            None => None,
        };
    }

    /// Which heading each task sits under, by id: the nearest ancestor that is
    /// a topic. Read by the view, so a moved task's record moves with it.
    fn owners(&self) -> std::collections::HashMap<String, String> {
        let mut out = std::collections::HashMap::new();
        for id in self.nodes.keys() {
            let mut at = self.nodes.get(id).and_then(|n| n.parent);
            while let Some(parent) = at {
                let Some(node) = self.nodes.get(&parent) else { break };
                if node.form == Form::Topic {
                    out.insert(id.to_string(), parent.to_string());
                    break;
                }
                at = node.parent;
            }
        }
        out
    }

    fn view(&self) -> Vec<TaskNode> {
        let ids: &[TaskId] = match self.scope {
            None => &self.roots,
            Some(id) => self.node(id).map(|n| n.children.as_slice()).unwrap_or(&[]),
        };
        // Depth first, a heading before its parts: the list has to show that a
        // topic holds work, and a part the human cannot see is a part they
        // cannot move, stop or read. The parts are drawn stepped in, so the
        // shape of the line of work is the shape of the rows.
        let mut out = Vec::new();
        let mut at = 0;
        let mut rows: Vec<TaskId> = ids.to_vec();
        while at < rows.len() {
            let id = rows[at];
            at += 1;
            let Some(node) = self.nodes.get(&id) else { continue };
            out.push(node.clone());
            rows.splice(at..at, node.children.iter().copied());
        }
        out
    }

    /// The next dispatchable task, depth-first: a node's unsettled children
    /// are its decomposition, so the parts run before the whole. A held child
    /// holds its parent too -- the human paused a part, and running the whole
    /// without it would answer a question they stopped asking.
    fn next_queued(&self) -> Option<TaskId> {
        self.roots.iter().find_map(|id| self.ready(*id))
    }

    /// The first runnable node in this subtree, or the node itself when every
    /// child has settled. A failed child is settled: the parent reads the
    /// failure and decides, rather than being stranded by it.
    fn ready(&self, id: TaskId) -> Option<TaskId> {
        let node = self.nodes.get(&id)?;
        // A held node is the human having stopped that work, so nothing under
        // it runs either: a paused topic would otherwise go on through its
        // parts, one level down.
        if node.held && !node.children.is_empty() {
            return None;
        }
        for child in &node.children {
            let Some(c) = self.nodes.get(child) else { continue };
            let settled = matches!(
                c.state,
                NodeState::Done | NodeState::Failed | NodeState::Cancelled
            );
            if !settled {
                // A held part stops its whole subtree: the human paused that
                // work, and a detail of it is still that work.
                return if c.held { None } else { self.ready(*child) };
            }
        }
        // A topic is a place to keep work: it has no model turn of its own, so
        // the runner is never handed one.
        let runs = node.state == NodeState::Queued && !node.held && node.form != Form::Topic;
        runs.then_some(id)
    }
}
