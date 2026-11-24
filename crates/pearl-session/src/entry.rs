//! entry --- one append-only record in the session log.

use serde::{Deserialize, Serialize};

/// Who spoke, or what happened. The log records turns and tool events alike;
/// compaction projects over them without rewriting them. `Compaction` is a
/// derived record: a summary written later in the log that stands in for a
/// range of earlier entries, which stay where they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    System,
    User,
    Assistant,
    /// What the model worked out before answering. Kept for the human and
    /// for the record; it is not a turn, so it is never replayed as input.
    Reasoning,
    Tool,
    Compaction,
}

/// The stretch of the log a compaction entry stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub first: u64,
    pub last: u64,
}

/// One record. `seq` is monotonic so a resumed session keeps its order; the
/// log is append-only and a record is never edited in place. `covers` is set
/// only on a compaction entry, and names the range it replaces in the view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<Coverage>,
}

impl Entry {
    pub fn new(seq: u64, role: Role, content: impl Into<String>) -> Self {
        Self { seq, role, content: content.into(), covers: None }
    }

    /// A compaction entry: the summary of a range, written at `seq`.
    pub fn compaction(seq: u64, content: impl Into<String>, covers: Coverage) -> Self {
        Self { seq, role: Role::Compaction, content: content.into(), covers: Some(covers) }
    }

    /// The first sequence number this entry accounts for: its own, unless it
    /// is a summary of earlier work, in which case the range it covers.
    pub fn accounts_from(&self) -> u64 {
        self.covers.map_or(self.seq, |c| c.first)
    }
}
