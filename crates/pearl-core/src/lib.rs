//! pearl-core --- the task queue at the heart of PEARL.
//!
//! A persistent, editable tree of work the agent drives and the human shapes.
//! The queue lives outside the model's context, so a task the human asked for
//! survives compaction and resume; the single-focus flat view keeps the human's
//! attention single-threaded while the tree executes in parallel underneath.

pub mod event;
pub mod id;
pub mod node;
pub mod queue;
pub mod scheduler;

pub use event::Event;
pub use id::TaskId;
pub use node::{Draft, Form, Frame, NodeState, Position, Request, Steering, TaskNode};
pub use queue::{Queue, QueueError};
pub use scheduler::Scheduler;
