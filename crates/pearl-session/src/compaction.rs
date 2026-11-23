//! compaction --- a projection over the log, never a rewrite of it.
//!
//! Compaction does not edit the session file. It produces a view: a summary of
//! the old entries plus the recent ones kept verbatim. The log on disk stays
//! whole, so a resume reads the real history and the projection is recomputed,
//! not trusted. The queue lives outside this entirely and is never compacted.

use crate::entry::Entry;

/// A projected view of the log: what a model is shown, not what is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// A digest of the entries that fell out of the window.
    pub summary: String,
    /// The recent entries, kept verbatim and in order.
    pub retained: Vec<Entry>,
}

/// How the old entries are summarized. The real implementation calls a model;
/// the trait keeps the projection mechanism independent of that choice.
pub trait Summarize {
    fn summarize(&self, entries: &[Entry]) -> String;
}

/// A deterministic fold used by the tests and as the offline default: it
/// records how many entries were folded and their span, without a model.
#[derive(Debug, Default)]
pub struct FoldSummary;

impl Summarize for FoldSummary {
    fn summarize(&self, entries: &[Entry]) -> String {
        if entries.is_empty() {
            return String::new();
        }
        let first = entries.first().map(|e| e.seq).unwrap_or(0);
        let last = entries.last().map(|e| e.seq).unwrap_or(0);
        format!("folded {} entries (seq {first}..={last})", entries.len())
    }
}

/// Project the log, keeping the last `keep_recent` entries verbatim and
/// summarizing the rest. The input is never mutated.
pub fn project(entries: &[Entry], keep_recent: usize, summarizer: &dyn Summarize) -> Projection {
    if entries.len() <= keep_recent {
        return Projection { summary: String::new(), retained: entries.to_vec() };
    }
    let split = entries.len() - keep_recent;
    let (old, recent) = entries.split_at(split);
    Projection { summary: summarizer.summarize(old), retained: recent.to_vec() }
}
