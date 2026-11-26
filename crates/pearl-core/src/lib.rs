//! pearl-core --- the task queue at the heart of PEARL.
//!
//! A persistent, editable tree of work the agent drives and the human shapes.
//! The queue lives outside the model's context, so a task the human asked for
//! survives compaction and resume; the single-focus flat view keeps the human's
//! attention single-threaded while the tree executes in parallel underneath.

pub mod aside;
pub mod event;
pub mod cost;
pub mod gauge;
pub mod id;
pub mod node;
pub mod queue;
pub mod scheduler;
pub mod verb;
pub mod catalog;
pub mod dialect;
pub mod settings;

pub use aside::{Aside, AsideError, AsideState};
pub use event::Event;
pub use cost::{Cost, Turn};
pub use gauge::Gauge;
pub use id::TaskId;
pub use node::{Form, Frame, NodeState, Position, Request, Steering, TaskNode};
pub use queue::{Queue, QueueError};
pub use scheduler::Scheduler;
pub use verb::Verb;
pub use catalog::Candidate;
pub use dialect::Dialect;
pub use settings::{Effort, Keys, Settings};
