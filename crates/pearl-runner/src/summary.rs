//! summary --- the fold written by the model, over the same provider.
//!
//! The projection mechanism is ours; the summarization is a request. This is
//! the only place fold asks a model anything, and it asks for the things
//! a model resuming the work would otherwise have to re-discover: the paths,
//! the signatures, the errors, the decisions and why they were made, and what
//! is still open.
//!
//! The rules below are the fold's contract. They are load-bearing: a summary
//! that drops a path or a decision is a summary that makes the agent re-read
//! the same files, which is the cost fold exists to avoid.

use pearl_log::{Entry, Summarize};
use pearl_provider::{ChatMessage, CompletionRequest, Provider, Role};

/// What a fold must preserve, in the order it must be preserved.
const RULES: &str = "\
You are folding the history of a coding agent that is still working. Your
summary replaces the entries below: it becomes the only record of them. A later
reader (the same agent, continuing the task) must be able to keep working
without the originals.

Keep verbatim, never paraphrased:
- File paths with line numbers, the directory prefix on every mention
  (`crates/pearl-runner/src/runner.rs:93`), never a bare filename.
- Exact signatures of functions and structs, and the line encoding the logic.
- Errors and messages as the tool printed them.
- Values: versions, thresholds, config keys, magic numbers.
- Decisions and their reason, as \"chose X over Y because Z\". The reason is the
  part that stops the next round from undoing the choice.
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
    gauge: pearl_core::Gauge,
    /// The row the fold was made for, when a task asked for it. A fold is a
    /// round of that task's work: the row sums it and the log holds it, so the
    /// human reading the report sees what the summary cost beside what it
    /// replaced.
    row: Option<(pearl_core::Queue, pearl_core::TaskId)>,
}

impl<'a, P: Provider> ModelSummary<'a, P> {
    /// Summarize with `model`, through `provider`, billing the fold to `gauge`.
    ///
    /// A fold is a request like any other: it is paid for whether or not a task
    /// asked for it, and a ledger that misses it under-reports the log by
    /// every fold it ever made.
    pub fn new(provider: &'a P, model: &'a str, gauge: pearl_core::Gauge) -> Self {
        Self {
            provider,
            model,
            gauge,
            row: None,
        }
    }

    /// Bill the fold to the row it was made for as well as to the log.
    pub fn for_row(mut self, queue: pearl_core::Queue, id: pearl_core::TaskId) -> Self {
        self.row = Some((queue, id));
        self
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
        let began = std::time::Instant::now();
        match self.provider.complete(request).await {
            Ok(done) => {
                let mut cost = pearl_core::Cost::round(
                    done.usage.prompt_tokens,
                    done.usage.completion_tokens,
                    began.elapsed().as_millis() as u64,
                );
                cost.cached = done.usage.cached_tokens;
                self.gauge.charge(cost);
                if let Some((queue, id)) = &self.row {
                    queue.charge(*id, pearl_core::Bill::doing(cost, "fold"));
                }
                done.content
            }
            // A fold that cannot be written must not become a wrong summary:
            // the caller keeps the history and tries again next round.
            Err(e) => format!("[fold failed: {e}]"),
        }
    }
}

/// How much of one tool output a fold reads from each end.
///
/// A fold exists to throw the bulk away, and paying it to read the bulk is the
/// one cost it cannot justify. Two thousand characters is what pi gives the
/// whole output; the same budget is spent here on the two ends, because the
/// rules above ask for the error as the tool printed it and a test run prints
/// its failure last. Head-only truncation keeps the command and loses the
/// reason the command mattered.
const FOLD_HEAD: usize = 1_500;
const FOLD_TAIL: usize = 500;

/// Render entries as the transcript a fold reads.
fn render(entries: &[Entry]) -> String {
    let mut out = String::from("Fold these entries:\n\n");
    for e in entries {
        // Thinking is not a message: folding it forward would carry the model's
        // scratch work into every later context, at full price.
        if e.role == pearl_log::Role::Reasoning {
            continue;
        }
        let body = if e.role == pearl_log::Role::Tool {
            ends(&e.content)
        } else {
            e.content.clone()
        };
        out.push_str(&format!("[{}] {}: {}\n\n", e.seq, role_name(e), body));
    }
    out
}

/// The opening and the end of a long output, with the middle named and left
/// out. Short ones are read whole: the elision line is longer than an `ok`.
fn ends(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= FOLD_HEAD + FOLD_TAIL {
        return content.to_string();
    }
    let head: String = chars[..FOLD_HEAD].iter().collect();
    let tail: String = chars[chars.len() - FOLD_TAIL..].iter().collect();
    let gone = chars.len() - FOLD_HEAD - FOLD_TAIL;
    format!("{head}\n…[{gone} characters folded]…\n{tail}")
}

/// The role as the transcript spells it, including what a summary is.
fn role_name(entry: &Entry) -> &'static str {
    match entry.role {
        pearl_log::Role::System => "system",
        pearl_log::Role::User => "user",
        pearl_log::Role::Assistant => "assistant",
        pearl_log::Role::Reasoning => "thinking",
        pearl_log::Role::Tool => "tool",
        pearl_log::Role::Fold => "summary",
        pearl_log::Role::Launch => "launch",
        pearl_log::Role::Queue => "queue",
        pearl_log::Role::Bill => "bill",
        pearl_log::Role::Usage => "usage",
    }
}
