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
    /// A run of the program began. The view starts after the last one, so a
    /// launch carries the conversation it started rather than the runs before
    /// it: the log keeps those as the file's record, the request does not.
    Launch,
    /// The queue as it stood. The list is a projection of the log like the
    /// conversation is: a snapshot is appended whenever the queue changes, and
    /// the newest one is what a resume restores (ADR 29). It is the human's
    /// state, never a turn, so no request ever reads it.
    Queue,
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
///
/// `calls` and `call` are what make a replayed log a valid request. An
/// assistant turn that asks for tools records the calls it asked for, and a
/// tool turn records the id it answers. Without them the replay is a
/// conversation with an unanswered question in it, which endpoints refuse:
/// DeepSeek answers `messages[84]: missing field 'tool_call_id'`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<Coverage>,
    /// The tool calls an assistant turn asked for, as JSON on the wire. Stored
    /// as text because the session log records the conversation, not the
    /// provider's types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<String>,
    /// The call id a tool turn answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    /// The topic this turn belongs to, as the id of its root heading. `None`
    /// is the shared line: the flat list and every global entry, which every
    /// topic can see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

impl Entry {
    pub fn new(seq: u64, role: Role, content: impl Into<String>) -> Self {
        Self {
            seq,
            role,
            content: content.into(),
            covers: None,
            calls: None,
            call: None,
            topic: None,
        }
    }

    /// A compaction entry: the summary of a range, written at `seq`.
    pub fn compaction(seq: u64, content: impl Into<String>, covers: Coverage) -> Self {
        Self {
            seq,
            role: Role::Compaction,
            content: content.into(),
            covers: Some(covers),
            calls: None,
            call: None,
            topic: None,
        }
    }

    /// An assistant turn that asked for tools, with the calls it asked for.
    pub fn asked(seq: u64, calls: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            seq,
            role: Role::Assistant,
            content: content.into(),
            covers: None,
            calls: Some(calls.into()),
            call: None,
            topic: None,
        }
    }

    /// A tool turn, saying which call it answers.
    pub fn answered(seq: u64, call: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            seq,
            role: Role::Tool,
            content: content.into(),
            covers: None,
            calls: None,
            call: Some(call.into()),
            topic: None,
        }
    }

    /// The first sequence number this entry accounts for: its own, unless it
    /// is a summary of earlier work, in which case the range it covers.
    pub fn accounts_from(&self) -> u64 {
        self.covers.map_or(self.seq, |c| c.first)
    }
}
