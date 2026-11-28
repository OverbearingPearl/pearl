//! pearl-runner --- the loop that moves the queue across the wire.
//!
//! The queue is the subject; the runner is only the hand that carries each task
//! to the model and writes the answer into the log. Steering lands at the
//! round boundary, and nothing is forgotten because the queue outlives context.

pub mod runner;
pub mod spawn;
pub mod summary;
pub mod tools;

pub use pearl_tools::sandbox::Policy;
pub use runner::{FoldPolicy, Runner, RunnerError, replay};
pub use spawn::{Spawned, SubagentPool};
pub use summary::ModelSummary;
pub use tools::ToolRegistry;
