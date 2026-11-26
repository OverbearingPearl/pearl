//! pearl-agent --- the loop that moves the queue across the wire.
//!
//! The queue is the subject; the agent is only the hand that carries each task
//! to the model and writes the answer into the session. Steering lands at the
//! turn boundary, and nothing is forgotten because the queue outlives context.

pub mod agent;
pub mod spawn;
pub mod summary;
pub mod tools;

pub use agent::{Agent, AgentError, CompactionPolicy, replay};
pub use pearl_tools::sandbox::Policy;
pub use spawn::{Spawned, SubagentPool};
pub use summary::ModelSummary;
pub use tools::ToolRegistry;
