//! pearl-core --- the task queue at the heart of PEARL.
//!
//! A persistent, editable tree of work the agent drives and the human shapes.
//! The queue lives outside the model's context, so a task the human asked for
//! survives compaction and resume; the single-focus flat view keeps the human's
//! attention single-threaded while the tree executes in parallel underneath.

pub mod aside;
pub mod catalog;
pub mod cost;
pub mod dialect;
pub mod event;
pub mod gauge;
pub mod id;
pub mod node;
pub mod queue;
pub mod scheduler;
pub mod settings;
pub mod verb;

pub use aside::{Aside, AsideError, AsideState};
pub use catalog::Candidate;
pub use cost::{Cost, Turn};
pub use dialect::Dialect;
pub use event::Event;
pub use gauge::Gauge;
pub use id::TaskId;
pub use node::{Form, Frame, NodeState, Position, Report, Request, Steering, TaskNode};
pub use queue::{Queue, QueueError};
pub use scheduler::Scheduler;
pub use settings::{Effort, Keys, Settings};
pub use verb::Verb;
