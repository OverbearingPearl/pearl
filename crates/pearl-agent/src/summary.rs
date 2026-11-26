//! summary --- the fold written by the model, over the same provider.
//!
//! The projection mechanism is ours; the summarization is a request. This is
//! the only place compaction asks a model anything, and it asks for the things
//! a model resuming the work would otherwise have to re-discover: the paths,
//! the signatures, the errors, the decisions and why they were made, and what
//! is still open.
//!
//! The rules below are the fold's contract. They are load-bearing: a summary
//! that drops a path or a decision is a summary that makes the agent re-read
//! the same files, which is the cost compaction exists to avoid.

use pearl_provider::{ChatMessage, CompletionRequest, Provider, Role};
use pearl_session::{Entry, Summarize};

/// What a fold must preserve, in the order it must be preserved.
const RULES: &str = "\
You are compacting the history of a coding agent that is still working. Your
summary replaces the entries below: it becomes the only record of them. A later
reader (the same agent, continuing the task) must be able to keep working
without the originals.

Keep verbatim, never paraphrased:
- File paths with line numbers, the directory prefix on every mention
  (`crates/pearl-agent/src/agent.rs:93`), never a bare filename.
- Exact signatures of functions and structs, and the line encoding the logic.
- Errors and messages as the tool printed them.
- Values: versions, thresholds, config keys, magic numbers.
- Decisions and their reason, as \"chose X over Y because Z\". The reason is the
  part that stops the next turn from undoing the choice.
- Task state as history: write \"TASK AS OF THIS BLOCK: ...\", never as an
  instruction to carry out now.
- Anything a user asked for that is not finished and not cancelled, on a final
  line beginning `Open objectives:`, with what remains.

Drop the vessel and keep the signal: logs once the error line is recorded,
duplicate reads, explored dead ends (keep one line: \"tried X, failed because
Y\"), and the back-and-forth that led to a decision once the decision is made.

Write dense bullets under short headers. No preamble, no sign-off. Aim for
roughly a thousand tokens: shorter loses the paths, longer stops paying for
itself.";

/// A fold written by the model, over the provider the loop already uses.
pub struct ModelSummary<'a, P: Provider> {
    provider: &'a P,
    model: &'a str,
}

impl<'a, P: Provider> ModelSummary<'a, P> {
    /// Summarize with `model`, through `provider`.
    pub fn new(provider: &'a P, model: &'a str) -> Self {
        Self { provider, model }
    }
}

impl<P: Provider + Sync> Summarize for ModelSummary<'_, P> {
    async fn summarize(&self, entries: &[Entry]) -> String {
        let transcript = render(entries);
        let request = CompletionRequest {
            model: self.model.to_string(),
            messages: vec![
                ChatMessage::new(Role::System, RULES),
                ChatMessage::new(Role::User, transcript),
            ],
            stream: false,
            tools: vec![],
            reasoning_effort: None,
            extra: None,
        };
        match self.provider.complete(request).await {
            Ok(done) => done.content,
            // A fold that cannot be written must not become a wrong summary:
            // the caller keeps the history and tries again next turn.
            Err(e) => format!("[compaction failed: {e}]"),
        }
    }
}

/// Render entries as the transcript a fold reads.
fn render(entries: &[Entry]) -> String {
    let mut out = String::from("Fold these entries:\n\n");
    for e in entries {
        // Thinking is not a turn: folding it forward would carry the model's
        // scratch work into every later context, at full price.
        if e.role == pearl_session::Role::Reasoning {
            continue;
        }
        out.push_str(&format!("[{}] {}: {}\n\n", e.seq, role_name(e), e.content));
    }
    out
}

/// The role as the transcript spells it, including what a summary is.
fn role_name(entry: &Entry) -> &'static str {
    match entry.role {
        pearl_session::Role::System => "system",
        pearl_session::Role::User => "user",
        pearl_session::Role::Assistant => "assistant",
        pearl_session::Role::Reasoning => "thinking",
        pearl_session::Role::Tool => "tool",
        pearl_session::Role::Compaction => "summary",
        pearl_session::Role::Launch => "launch",
        pearl_session::Role::Queue => "queue",
        pearl_session::Role::Usage => "usage",
    }
}
