//! node --- one unit of work in the queue, and the states it moves through.

use serde::{Deserialize, Serialize};

use crate::id::TaskId;

/// The observable state of a task. The running state is composite in the
/// scheduler (executing, draining steering at a boundary, waiting on children)
/// but a reader of the queue sees one `Running`; the sub-phase is not a state
/// the human edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeState {
    /// Appended, waiting for the scheduler to dispatch it.
    Queued,
    /// The agent loop is working on it.
    Running,
    /// The agent asked a question and waits for an answer.
    AwaitingInput,
    /// Preempted by an urgent task; its frame is saved for resume.
    Paused,
    /// Finished, and every child finished too.
    Done,
    /// Ended in error; may be retried back to `Queued`.
    Failed,
    /// Cancelled by the human; terminal.
    Cancelled,
}

impl NodeState {
    /// Whether no further transition leaves this state.
    pub fn is_terminal(self) -> bool {
        matches!(self, NodeState::Done | NodeState::Cancelled)
    }
}

/// A steering message: an annotation on a running task, injected at the next
/// safe boundary rather than mid-token. It is a self-loop on `Running`, not a
/// new task and not a new state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steering {
    pub text: String,
}

/// The saved progress of a preempted task, restored when it resumes. The frame
/// is what makes "put that down and do this first" reversible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub progress: String,
}

/// One node of the task tree. Children are nested tasks (a subagent's queue is
/// a subtree); the parent does not finish until they all do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: TaskId,
    pub parent: Option<TaskId>,
    pub title: String,
    pub state: NodeState,
    pub children: Vec<TaskId>,
    /// Pending steering, drained at the next boundary.
    pub steering: Vec<Steering>,
    /// Saved when paused, restored on resume.
    pub frame: Option<Frame>,
    /// The question asked, when awaiting input.
    pub question: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl TaskNode {
    pub(crate) fn new(id: TaskId, parent: Option<TaskId>, title: String) -> Self {
        Self {
            id,
            parent,
            title,
            state: NodeState::Queued,
            children: Vec::new(),
            steering: Vec::new(),
            frame: None,
            question: None,
            result: None,
            error: None,
        }
    }

    /// Whether this node may finish: no child is still unfinished.
    pub fn children_settled(&self, queue: &crate::queue::Queue) -> bool {
        self.children.iter().all(|c| {
            queue
                .get(*c)
                .map(|n| n.state.is_terminal() || n.state == NodeState::Failed)
                .unwrap_or(true)
        })
    }
}
