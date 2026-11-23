//! id --- a stable identity for a task, surviving serialization and resume.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The identity of one task. It is a UUID so a queue written to disk and read
/// back keeps the same nodes, and a resumed session refers to the same work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
