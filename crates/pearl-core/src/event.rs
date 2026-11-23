//! event --- what the queue reports upward, so the surface can show status.

use serde::{Deserialize, Serialize};

use crate::id::TaskId;
use crate::node::NodeState;

/// An event flows up from the queue to the flow surface. The human reads these
/// as status badges; they are awareness, not a second conversation to tend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Event {
    /// A node changed state.
    StateChanged { id: TaskId, from: NodeState, to: NodeState },
    /// A steering message was injected at a boundary.
    SteeringInjected { id: TaskId, text: String },
    /// A task was appended (follow-up).
    Appended { id: TaskId, title: String },
    /// A task preempted another, saving its frame.
    Preempted { paused: TaskId, urgent: TaskId },
    /// A paused task resumed, restoring its frame.
    Resumed { id: TaskId },
    /// A child finished; the parent may now be able to finish.
    ChildSettled { parent: TaskId, child: TaskId },
    /// A placed-but-unstarted task was withdrawn back into a draft.
    Withdrawn { id: TaskId },
    /// A task was held: paused if running, or marked held if queued.
    Held { id: TaskId },
    /// A running task's objective was replaced; it re-plans at the boundary.
    Redirected { id: TaskId, title: String },
}
