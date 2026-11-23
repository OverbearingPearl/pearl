//! pearl-agent --- the loop that moves the queue across the wire.
//!
//! The queue is the subject; the agent is only the hand that carries each task
//! to the model and writes the answer into the session. Steering lands at the
//! turn boundary, and nothing is forgotten because the queue outlives context.

pub mod agent;

pub use agent::{Agent, AgentError};
