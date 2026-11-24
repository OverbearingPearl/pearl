//! node --- one unit of work in the queue, and the states it moves through.
//!
//! A task is a request that has been placed on the queue. Before it executes
//! it is a loose part: the human may take it off the queue, edit the request,
//! and place it again in another form or position. The node carries only what
//! survives that round trip: its intent, its state, and the frame that lets a
//! paused task resume.

use serde::{Deserialize, Serialize};

use crate::id::TaskId;

/// Where a request is placed relative to the current focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// After the focused task: do it after this one. The deliberate move a
    /// taken request is placed with.
    After,
    /// At the end of the queue: do it last. What append means.
    Last,
    /// Before everything: do it now. The urgent insert.
    Now,
    /// Into a running or queued task as a steering hint, not a new task.
    Into(TaskId),
}

/// The form a request takes when placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Form {
    /// An independent task node with its own lifecycle.
    Task,
    /// A hint attached to another task; it has no lifecycle of its own.
    Steering,
}

/// A unit of intent the human places on the queue. An unexecuted node is a
/// loose part: `take` lifts it back into a request, the human edits that value
/// in hand, and `place` puts it back in a different form or position. The
/// request is the disassembled form; there is no separate draft type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub title: String,
    pub form: Form,
    /// The task a steering form attaches to; `None` for an independent task.
    pub target: Option<TaskId>,
}

impl Request {
    pub fn task(title: impl Into<String>) -> Self {
        Self { title: title.into(), form: Form::Task, target: None }
    }

    pub fn steering(title: impl Into<String>, target: TaskId) -> Self {
        Self { title: title.into(), form: Form::Steering, target: Some(target) }
    }
}

/// The observable state of a task. The running state is composite in the
/// scheduler (executing, draining steering at a boundary, waiting on children)
/// but a reader of the queue sees one `Running`; the sub-phase is not a state
/// the human edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeState {
    /// Placed, waiting for the scheduler to dispatch it.
    Queued,
    /// The agent loop is working on it.
    Running,
    /// The agent asked a question and waits for an answer.
    AwaitingInput,
    /// Held by the human; its frame is saved for resume.
    Paused,
    /// Finished successfully.
    Done,
    /// Finished with an error.
    Failed,
    /// Abandoned by the human or the system.
    Cancelled,
}

/// A steering hint waiting to be injected at a safe boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steering {
    pub text: String,
}

/// The saved progress of a paused task; resume restores rather than restarts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub progress: String,
}

/// One task in the queue tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: TaskId,
    pub parent: Option<TaskId>,
    pub title: String,
    pub state: NodeState,
    pub children: Vec<TaskId>,
    pub steering: Vec<Steering>,
    pub frame: Option<Frame>,
    pub question: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    /// What the task is doing right now, in the runner's words. The human reads
    /// it while the work runs; the report takes its place when the task ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    /// What the model worked out before answering. The human's to read; it is
    /// never sent back to the endpoint.
    pub reasoning: Option<String>,
    /// A queued task the human has held; the scheduler skips it until resumed.
    pub held: bool,
}

impl TaskNode {
    pub fn new(id: TaskId, parent: Option<TaskId>, title: impl Into<String>) -> Self {
        Self {
            id,
            parent,
            title: title.into(),
            state: NodeState::Queued,
            children: Vec::new(),
            steering: Vec::new(),
            frame: None,
            question: None,
            result: None,
            error: None,
            activity: None,
            reasoning: None,
            held: false,
        }
    }

    /// Whether the task has reached a terminal state.
    pub fn is_settled(&self) -> bool {
        matches!(self.state, NodeState::Done | NodeState::Failed | NodeState::Cancelled)
    }

    /// Whether every child has settled, so a waiting parent may finish.
    pub fn children_settled(&self, nodes: &std::collections::HashMap<TaskId, TaskNode>) -> bool {
        self.children.iter().all(|c| nodes.get(c).map(|n| n.is_settled()).unwrap_or(true))
    }
}
