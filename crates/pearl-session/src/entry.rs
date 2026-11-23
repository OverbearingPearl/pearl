//! entry --- one append-only record in the session log.

use serde::{Deserialize, Serialize};

/// Who spoke, or what happened. The log records turns and tool events alike;
/// compaction projects over them without rewriting them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One record. `seq` is monotonic so a resumed session keeps its order; the
/// log is append-only and a record is never edited in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub role: Role,
    pub content: String,
}

impl Entry {
    pub fn new(seq: u64, role: Role, content: impl Into<String>) -> Self {
        Self { seq, role, content: content.into() }
    }
}
