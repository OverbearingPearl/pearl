//! entry --- one append-only record in the log.

use serde::{Deserialize, Serialize};

/// Who spoke, or what happened. The log records messages and tool events alike;
/// folding projects over them without rewriting them. `Fold` is a derived
/// record: a summary written later in the log that stands in for a range of
/// earlier entries, which stay where they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    System,
    User,
    Assistant,
    /// What the model worked out before answering. Kept for the human and
    /// for the record; it is not a message, so it is never replayed as input.
    Reasoning,
    Tool,
    /// A summary standing in for a range of earlier entries: one fold.
    Fold,
    /// A run of the program began. The view starts after the last one, so a
    /// launch carries the conversation it started rather than the runs before
    /// it: the log keeps those as the file's record, the request does not.
    Launch,
    /// The queue as it stood. The list is a projection of the log like the
    /// conversation is: a snapshot is appended whenever the queue changes, and
    /// the newest one is what a resume restores (ADR 29). It is the human's
    /// state, never a message, so no request ever reads it.
    Queue,
    /// What one round was billed: tokens up, tokens down, the wait, and what
    /// the round asked for. The report itemises the rounds, so a resume reads
    /// them from here rather than from a second copy inside every snapshot.
    /// Like the list, it is the human's reading and never a message on the
    /// wire.
    Bill,
    Usage,
}

/// The stretch of the log a fold entry stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub first: u64,
    pub last: u64,
}

/// One record. `seq` is monotonic so a resumed run keeps its order; the
/// log is append-only and a record is never edited in place. `covers` is set
/// only on a fold entry, and names the range it replaces in the view.
///
/// `calls` and `call` are what make a replayed log a valid request. An
/// assistant message that asks for tools records the calls it asked for, and a
/// tool message records the id it answers. Without them the replay is a
/// conversation with an unanswered question in it, which endpoints refuse:
/// DeepSeek answers `messages[84]: missing field 'tool_call_id'`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<Coverage>,
    /// The tool calls an assistant message asked for, as JSON on the wire.
    /// Stored as text because the log records the conversation, not the
    /// provider's types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<String>,
    /// The call id a tool message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    /// The topic this entry belongs to, as the id of its root topic. `None`
    /// is the shared line: the flat list and every global entry, which every
    /// topic can see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// The task this entry was spoken for. An entry belongs to a task, and a
    /// task belongs to wherever it sits on the list; the topic an entry is
    /// read in is looked up from here, so moving a finished task under another
    /// topic moves its record with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
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
            task: None,
        }
    }

    /// A fold entry: the summary of a range, written at `seq`.
    pub fn fold(seq: u64, content: impl Into<String>, covers: Coverage) -> Self {
        Self {
            seq,
            role: Role::Fold,
            content: content.into(),
            covers: Some(covers),
            calls: None,
            call: None,
            topic: None,
            task: None,
        }
    }

    /// An assistant message that asked for tools, with the calls it asked for.
    pub fn asked(seq: u64, calls: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            seq,
            role: Role::Assistant,
            content: content.into(),
            covers: None,
            calls: Some(calls.into()),
            call: None,
            topic: None,
            task: None,
        }
    }

    /// A tool message, saying which call it answers.
    pub fn answered(seq: u64, call: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            seq,
            role: Role::Tool,
            content: content.into(),
            covers: None,
            calls: None,
            call: Some(call.into()),
            topic: None,
            task: None,
        }
    }

    /// The first sequence number this entry accounts for: its own, unless it
    /// is a summary of earlier work, in which case the range it covers.
    pub fn accounts_from(&self) -> u64 {
        self.covers.map_or(self.seq, |c| c.first)
    }

    /// Everything this record puts on the wire, as far as a token count is
    /// concerned: its text, and the calls it asked for.
    /// A message that asks for six tools carries six arguments, and the
    /// endpoint bills for every character of them. On a night of shell work the
    /// calls outweighed the prose five to one, so a count of the prose alone
    /// left most of the prompt out of the decision to fold.
    pub fn weighed(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.content.as_str()).chain(self.calls.as_deref())
    }
}
