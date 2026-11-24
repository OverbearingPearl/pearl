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

    /// Take an unexecuted node off the queue, returning it as a request -- the
    /// disassembled loose part. The human edits that value in hand and `place`s
    /// it again in another form or position. Only a `Queued` node can be taken;
    /// one that has started running is steered or held instead.
    pub fn take(&self, id: TaskId) -> Result<(Request, Vec<Event>)> {
        self.tree().take(id)
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
    pub fn resume(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().resume(id)
    }

    /// Redirect a running task: replace its objective, preserve the work done,
    /// and signal a re-plan at the boundary. Distinct from steer (additive) and
    /// from cancel+append (which throws the progress away).
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
    pub fn cancel(&self, id: TaskId) -> Result<Vec<Event>> {
        self.tree().cancel(id)
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
        match request.form {
            Form::Steering => {
                let target = request.target.ok_or(QueueError::SteeringNeedsTarget)?;
                self.steer(target, request.title)?;
                Ok((target, vec![]))
            }
            Form::Task => {
                let id = TaskId::new();
                let title = request.title.clone();
                self.nodes.insert(id, TaskNode::new(id, None, title.clone()));
                match position {
                    Position::Now => self.roots.insert(0, id),
                    Position::After => {
                        // After the focused task if there is one, else last.
                        let at = self
                            .focus
                            .and_then(|f| self.roots.iter().position(|r| *r == f))
                            .map(|i| i + 1)
                            .unwrap_or(self.roots.len());
                        self.roots.insert(at, id);
                    }
                    Position::Into(parent) => {
                        self.node(parent)?;
                        self.nodes.get_mut(&id).unwrap().parent = Some(parent);
                        self.node_mut(parent)?.children.push(id);
                        // A child is not a root.
                        self.roots.retain(|r| *r != id);
                    }
                }
                if self.focus.is_none() {
                    self.focus = Some(id);
                }
                Ok((id, vec![Event::Appended { id, title }]))
            }
        }
    }

    fn take(&mut self, id: TaskId) -> Result<(Request, Vec<Event>)> {
        self.expect(id, NodeState::Queued, "queued")?;
        let n = self.nodes.remove(&id).ok_or(QueueError::Unknown(id))?;
        self.roots.retain(|r| *r != id);
        if let Some(parent) = n.parent {
            if let Ok(p) = self.node_mut(parent) {
                p.children.retain(|c| *c != id);
            }
        }
        if self.focus == Some(id) {
            self.focus = self.roots.first().copied();
        }
        let request = Request { title: n.title, form: Form::Task, target: None };
        Ok((request, vec![Event::Withdrawn { id }]))
    }

    fn append(&mut self, title: impl Into<String>) -> (TaskId, Vec<Event>) {
        self.place(Request::task(title), Position::After).expect("task form always places")
    }

    fn add_child(&mut self, parent: TaskId, title: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.node(parent)?;
        self.place(Request::task(title), Position::Into(parent))
    }

    fn steer(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        if !matches!(state, NodeState::Queued | NodeState::Running) {
            return Err(QueueError::WrongState { id, found: state, expected: "queued or running" });
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
            NodeState::Queued => {
                self.node_mut(id)?.held = true;
                Ok(vec![Event::Held { id }])
            }
            _ => Err(QueueError::WrongState { id, found: state, expected: "queued or running" }),
        }
    }

    fn resume(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let n = self.node(id)?;
        match n.state {
            NodeState::Paused => {
                let events = vec![self.set_state(id, NodeState::Running)?, Event::Resumed { id }];
                Ok(events)
            }
            NodeState::Queued if n.held => {
                self.node_mut(id)?.held = false;
                Ok(vec![Event::Resumed { id }])
            }
            _ => Err(QueueError::WrongState { id, found: n.state, expected: "paused or held-queued" }),
        }
    }

    fn redirect(&mut self, id: TaskId, new_title: impl Into<String>) -> Result<Vec<Event>> {
        let state = self.node(id)?.state;
        if !matches!(state, NodeState::Running | NodeState::Paused | NodeState::AwaitingInput) {
            return Err(QueueError::WrongState { id, found: state, expected: "running, paused or awaiting-input" });
        }
        let new_title = new_title.into();
        let n = self.node_mut(id)?;
        n.title = new_title.clone();
        // The new objective is injected as steering so the loop re-plans
        // around it at the next boundary, keeping the frame and done children.
        n.steering.push(Steering { text: format!("New objective: {new_title}. Re-plan around it, keeping completed work.") });
        Ok(vec![Event::Redirected { id, title: new_title }])
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
        self.expect(id, NodeState::Failed, "failed")?;
        let n = self.node_mut(id)?;
        n.error = None;
        n.result = None;
        Ok(vec![self.set_state(id, NodeState::Queued)?])
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
            .map(|n| n.state == NodeState::Running && n.result.is_some() && n.children_settled(&self.nodes))
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

    fn view(&self) -> Vec<TaskNode> {
        let ids: &[TaskId] = match self.scope {
            None => &self.roots,
            Some(id) => self.node(id).map(|n| n.children.as_slice()).unwrap_or(&[]),
        };
        ids.iter().filter_map(|id| self.nodes.get(id)).cloned().collect()
    }

    fn next_queued(&self) -> Option<TaskId> {
        self.roots
            .iter()
            .copied()
            .find(|id| self.nodes.get(id).map(|n| n.state == NodeState::Queued && !n.held).unwrap_or(false))
    }
}
