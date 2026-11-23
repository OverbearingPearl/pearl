//! queue --- the persistent, editable tree of work, and its state machine.
//!
//! The queue is the product. It lives outside the model's context so compaction
//! cannot drop a task the human asked for; "don't forget" is a property of this
//! data structure, not a hope pinned on the model. The data is a tree; the view
//! is always a single-focus flat list at the current level, so the human edits
//! a list and drills in and out, never the tree itself.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::event::Event;
use crate::id::TaskId;
use crate::node::{Frame, NodeState, Steering, TaskNode};

/// An illegal transition or a reference to a node that is not there.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("no such task: {0}")]
    Unknown(TaskId),
    #[error("task {id} is {found:?}, expected {expected}")]
    WrongState { id: TaskId, found: NodeState, expected: &'static str },
    #[error("task {0} is terminal and cannot change")]
    Terminal(TaskId),
}

type Result<T> = std::result::Result<T, QueueError>;

/// The task tree. Serialize it to persist a session; deserialize to resume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
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

    pub fn get(&self, id: TaskId) -> Option<&TaskNode> {
        self.nodes.get(&id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn focus(&self) -> Option<TaskId> {
        self.focus
    }

    pub fn scope(&self) -> Option<TaskId> {
        self.scope
    }

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

    /// Append a new top-level task (follow-up). It enters `Queued`.
    pub fn append(&mut self, title: impl Into<String>) -> (TaskId, Vec<Event>) {
        let id = TaskId::new();
        let title = title.into();
        self.nodes.insert(id, TaskNode::new(id, None, title.clone()));
        self.roots.push(id);
        if self.focus.is_none() {
            self.focus = Some(id);
        }
        (id, vec![Event::Appended { id, title }])
    }

    /// Nest a child task under a parent (a subagent's work is a subtree).
    pub fn add_child(&mut self, parent: TaskId, title: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.node(parent)?;
        let id = TaskId::new();
        let title = title.into();
        self.nodes.insert(id, TaskNode::new(id, Some(parent), title.clone()));
        self.node_mut(parent)?.children.push(id);
        Ok((id, vec![Event::Appended { id, title }]))
    }

    /// Annotate a running task. The message waits in the steering log until the
    /// next safe boundary; it does not interrupt mid-token.
    pub fn steer(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        // Steering is an annotation on work that is queued or running; the
        // human may shape a task before it starts or while it runs.
        let state = self.node(id)?.state;
        if !matches!(state, NodeState::Queued | NodeState::Running) {
            return Err(QueueError::WrongState { id, found: state, expected: "queued or running" });
        }
        let text = text.into();
        self.node_mut(id)?.steering.push(Steering { text: text.clone() });
        // No event yet: the injection happens at the boundary, not on arrival.
        Ok(vec![])
    }

    /// Drain the pending steering at a boundary. The scheduler calls this when
    /// the loop reaches a safe point; each drained message is an injection.
    pub fn drain_steering(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        let drained: Vec<Steering> = std::mem::take(&mut self.node_mut(id)?.steering);
        Ok(drained
            .into_iter()
            .map(|s| Event::SteeringInjected { id, text: s.text })
            .collect())
    }

    /// Preempt: save the running task's frame, pause it, and append an urgent
    /// task that takes the focus. The paused task resumes when the urgent one
    /// settles.
    pub fn preempt(&mut self, id: TaskId, urgent_title: impl Into<String>, progress: impl Into<String>) -> Result<(TaskId, Vec<Event>)> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.frame = Some(Frame { progress: progress.into() });
        let mut events = vec![self.set_state(id, NodeState::Paused)?];
        let (urgent, appended) = self.append(urgent_title);
        events.extend(appended);
        self.focus = Some(urgent);
        events.push(Event::Preempted { paused: id, urgent });
        Ok((urgent, events))
    }

    /// Resume a paused task, restoring its saved frame.
    pub fn resume(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Paused, "paused")?;
        let events = vec![self.set_state(id, NodeState::Running)?, Event::Resumed { id }];
        // The frame stays on the node as restored progress; the loop reads it.
        Ok(events)
    }

    /// Dispatch a queued task into the running state.
    pub fn start(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Queued, "queued")?;
        Ok(vec![self.set_state(id, NodeState::Running)?])
    }

    /// The agent asks a question; the task waits for an answer.
    pub fn ask(&mut self, id: TaskId, question: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.question = Some(question.into());
        Ok(vec![self.set_state(id, NodeState::AwaitingInput)?])
    }

    /// Answer the question; the answer is injected as steering and it runs.
    pub fn answer(&mut self, id: TaskId, text: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::AwaitingInput, "awaiting-input")?;
        self.node_mut(id)?.question = None;
        let mut events = vec![self.set_state(id, NodeState::Running)?];
        events.extend(self.steer(id, text)?);
        Ok(events)
    }

    /// Record a result. The task finishes only when every child has settled;
    /// otherwise it stays running, waiting on its subtree.
    pub fn complete(&mut self, id: TaskId, result: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.result = Some(result.into());
        let settled = self.node(id)?.children_settled(self);
        if settled {
            let mut events = vec![self.set_state(id, NodeState::Done)?];
            events.extend(self.notify_parent(id));
            Ok(events)
        } else {
            Ok(vec![])
        }
    }

    /// End a task in error; it may be retried.
    pub fn fail(&mut self, id: TaskId, error: impl Into<String>) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Running, "running")?;
        self.node_mut(id)?.error = Some(error.into());
        let mut events = vec![self.set_state(id, NodeState::Failed)?];
        events.extend(self.notify_parent(id));
        Ok(events)
    }

    /// Retry a failed task back to the queue.
    pub fn retry(&mut self, id: TaskId) -> Result<Vec<Event>> {
        self.expect(id, NodeState::Failed, "failed")?;
        let n = self.node_mut(id)?;
        n.error = None;
        n.result = None;
        Ok(vec![self.set_state(id, NodeState::Queued)?])
    }

    /// Cancel a task and, recursively, its unfinished children.
    pub fn cancel(&mut self, id: TaskId) -> Result<Vec<Event>> {
        let n = self.node(id)?;
        if n.state.is_terminal() {
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
            .map(|n| n.state == NodeState::Running && n.result.is_some() && n.children_settled(self))
            .unwrap_or(false);
        if waiting {
            if let Ok(ev) = self.set_state(parent, NodeState::Done) {
                events.push(ev);
            }
            events.extend(self.notify_parent(parent));
        }
        events
    }

    // --- the single-focus view ---------------------------------------------

    /// Move the focus cursor to a node.
    pub fn focus_on(&mut self, id: TaskId) -> Result<()> {
        self.node(id)?;
        self.focus = Some(id);
        Ok(())
    }

    /// Narrow the view to a node's children (drill in).
    pub fn narrow(&mut self, id: TaskId) -> Result<()> {
        self.node(id)?;
        self.scope = Some(id);
        self.focus = self.node(id)?.children.first().copied();
        Ok(())
    }

    /// Widen the view one level (drill out).
    pub fn unnarrow(&mut self) {
        self.scope = match self.scope {
            Some(id) => self.nodes.get(&id).and_then(|n| n.parent),
            None => None,
        };
    }

    /// The flat list at the current level: the roots, or the scoped node's
    /// children. This is what the surface renders and the human edits.
    pub fn view(&self) -> Vec<&TaskNode> {
        let ids: &[TaskId] = match self.scope {
            None => &self.roots,
            Some(id) => self.node(id).map(|n| n.children.as_slice()).unwrap_or(&[]),
        };
        ids.iter().filter_map(|id| self.nodes.get(id)).collect()
    }

    /// The next queued task at the top level, in order, for the scheduler.
    pub fn next_queued(&self) -> Option<TaskId> {
        self.roots
            .iter()
            .copied()
            .find(|id| self.nodes.get(id).map(|n| n.state == NodeState::Queued).unwrap_or(false))
    }
}
