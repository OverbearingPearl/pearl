//! compaction --- a projection over the log, never a rewrite of it.
//!
//! Compaction does not edit the session file. It produces a view: a summary of
//! the old entries plus the recent ones kept verbatim. The log on disk stays
//! whole, so a resume reads the real history and the projection is recomputed,
//! not trusted. The queue lives outside this entirely and is never compacted.
//!
//! The summary is itself an entry, appended with the range it covers. That is
//! what makes folding resumable: a second process reads the log, sees the
//! summary and the range behind it, and shows the view without asking a model
//! to write the same summary again.

use crate::entry::{Coverage, Entry, Role};
use crate::tokens::Meter;

/// A projected view of the log: what a model is shown, not what is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// A digest of the entries that fell out of the window.
    pub summary: String,
    /// The recent entries, kept verbatim and in order.
    pub retained: Vec<Entry>,
    /// The range the summary stands in for, or `None` when nothing was folded.
    pub covered: Option<Coverage>,
}

/// How much history a fold keeps and how much it must reclaim to be worth a
/// request. The tail is measured in tokens, not in messages, because one tool
/// result can outweigh twenty turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Entries kept no matter how small the tail budget is.
    pub keep_recent: usize,
    /// The most the kept tail may weigh.
    pub tail_tokens: u32,
    /// Below this the fold would cost a request and reclaim nothing.
    pub min_fold_tokens: u32,
}

impl Default for Budget {
    fn default() -> Self {
        Self { keep_recent: 3, tail_tokens: 8_000, min_fold_tokens: 2_000 }
    }
}

/// How the old entries are summarized. The real implementation calls a model;
/// the trait keeps the projection mechanism independent of that choice.
pub trait Summarize {
    fn summarize(&self, entries: &[Entry]) -> impl std::future::Future<Output = String> + Send;
}

/// A deterministic fold used by the tests and as the offline default: it
/// records how many entries were folded and their span, without a model.
#[derive(Debug, Default)]
pub struct FoldSummary;

impl Summarize for FoldSummary {
    async fn summarize(&self, entries: &[Entry]) -> String {
        if entries.is_empty() {
            return String::new();
        }
        let first = entries.first().map(|e| e.seq).unwrap_or(0);
        let last = entries.last().map(|e| e.seq).unwrap_or(0);
        format!("folded {} entries (seq {first}..={last})", entries.len())
    }
}

/// The entries a reader should see: everything except what a compaction entry
/// later in the log stands in for. The log itself is untouched.
pub fn view(entries: &[Entry]) -> Vec<Entry> {
    let mut shown: Vec<Entry> =
        entries.iter().filter(|e| !is_covered(entries, e)).cloned().collect();
    // A summary stands where the range it covers stood, not where it was
    // appended: appending happens at the end, which would put the old turns'
    // digest after the new turns it is supposed to come before.
    shown.sort_by_key(Entry::accounts_from);
    shown
}

/// Whether a compaction entry accounts for `entry`.
fn is_covered(entries: &[Entry], entry: &Entry) -> bool {
    entries.iter().any(|later| {
        later.covers.is_some_and(|c| later.seq > entry.seq && c.first <= entry.seq && entry.seq <= c.last)
    })
}

/// Where a fold would cut: what it summarizes, and what it keeps verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub fold: Vec<Entry>,
    pub keep: Vec<Entry>,
}

impl Split {
    /// The range this fold stands in for, starting where the oldest folded
    /// entry's own range started.
    pub fn coverage(&self) -> Option<Coverage> {
        let first = self.fold.first()?.accounts_from();
        let last = self.fold.last()?.seq;
        Some(Coverage { first, last })
    }
}

/// Choose a cut, or `None` when folding would not be worth a request.
///
/// The tail is as large as the token budget allows but never smaller than
/// `keep_recent` entries, and the cut is moved forward to the start of a user
/// turn: a tool result folded away from the call that produced it leaves the
/// model a result with no question, which is worse than a longer context.
pub fn split(entries: &[Entry], budget: &Budget, meter: &Meter) -> Option<Split> {
    let n = entries.len();
    if n <= budget.keep_recent {
        return None;
    }
    let mut cut = n;
    let mut kept = 0;
    while cut > 0 {
        let weight = meter.tokens(&entries[cut - 1].content);
        if n - cut >= budget.keep_recent && kept + weight > budget.tail_tokens {
            break;
        }
        kept += weight;
        cut -= 1;
    }
    while cut < n && entries[cut].role != Role::User {
        cut += 1;
    }
    if cut == 0 || n - cut < budget.keep_recent {
        return None;
    }
    let (fold, keep) = entries.split_at(cut);
    if meter.total(fold.iter().map(|e| e.content.as_str())) < budget.min_fold_tokens {
        return None;
    }
    Some(Split { fold: fold.to_vec(), keep: keep.to_vec() })
}

/// Project the log: fold what the budget lets go of, keep the rest verbatim.
/// The input is never mutated; the caller writes the summary as a new entry.
pub async fn project<S: Summarize>(
    entries: &[Entry],
    budget: &Budget,
    summarizer: &S,
    meter: &Meter,
) -> Projection {
    match split(entries, budget, meter) {
        None => Projection { summary: String::new(), retained: entries.to_vec(), covered: None },
        Some(p) => {
            let summary = summarizer.summarize(&p.fold).await;
            let covered = p.coverage();
            Projection { summary, retained: p.keep, covered }
        }
    }
}
