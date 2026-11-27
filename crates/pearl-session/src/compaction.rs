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
        Self {
            keep_recent: 3,
            tail_tokens: 8_000,
            min_fold_tokens: 2_000,
        }
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

/// The entries a reader should see: the conversation the last launch began,
/// without what a compaction entry later in the log stands in for. The log
/// itself is untouched.
pub fn view(entries: &[Entry]) -> Vec<Entry> {
    view_in(entries, None)
}

/// The turns one topic's conversation holds: its own, and the shared line.
///
/// The filter runs before coverage on purpose. A fold in one topic covers a
/// stretch of sequence numbers, and those numbers run across every topic in
/// the file, so asking what is covered first would let one topic's summary
/// swallow another topic's turns.
pub fn view_in(entries: &[Entry], topic: Option<&str>) -> Vec<Entry> {
    view_in_owned(entries, topic, &std::collections::HashMap::new())
}

/// The view of one topic, with each turn read where the task it was spoken for
/// sits now.
///
/// A turn belongs to a task, not to a folder: `owners` maps a task id to the
/// topic it is under at this moment, and the last word wins over the topic
/// stamped when the turn was written. Moving a finished task under another
/// topic therefore moves its record with it — the alternative is a
/// conversation that answers with turns it can no longer show.
pub fn view_in_owned(
    entries: &[Entry],
    topic: Option<&str>,
    owners: &std::collections::HashMap<String, String>,
) -> Vec<Entry> {
    // The roles a view drops are dropped before anything is copied. A night
    // of list snapshots is most of a long log, and copying them on the way to
    // throwing them away cost one question 48 MiB over a 21 MiB log.
    let entries: Vec<Entry> = entries
        .iter()
        .filter(|entry| {
            keeps(
                entry.role,
                entry.task.as_deref(),
                entry.topic.as_deref(),
                topic,
                owners,
            )
        })
        .cloned()
        .collect();
    view_of(&entries)
}

/// The topic a record is spoken for as the view reads it now: the topic its
/// task sits under at this moment, else the one it was written under.
///
/// `owners` is what makes moving a finished task under another topic move its
/// record with it, rather than leaving a conversation answering with turns it
/// can no longer show.
pub fn here(
    task: Option<&str>,
    topic: Option<&str>,
    owners: &std::collections::HashMap<String, String>,
) -> Option<String> {
    task.and_then(|task| owners.get(task).cloned())
        .or_else(|| topic.map(str::to_string))
}

/// Whether one record belongs in the view of `asked`.
///
/// This is the filter a streaming reader applies to a line before it decides
/// to hold it, so it is stated once here: a reader that skipped records and a
/// view that dropped them may not drift apart.
pub fn keeps(
    role: Role,
    task: Option<&str>,
    topic: Option<&str>,
    asked: Option<&str>,
    owners: &std::collections::HashMap<String, String>,
) -> bool {
    if NOT_TURNS.contains(&role) {
        return false;
    }
    match (here(task, topic, owners), asked) {
        (Some(own), Some(topic)) => own == topic,
        // The shared line: entries that belong to no topic, and the reader
        // that asked for no topic either.
        (None, _) => true,
        (Some(_), None) => false,
    }
}

/// The ranges the log's summaries stand in for, gathered once.
///
/// Asking every entry to scan every other entry was quadratic in the length
/// of the run.
pub fn folds<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Vec<(u64, Coverage)> {
    entries
        .into_iter()
        .filter_map(|e| e.covers.map(|c| (e.seq, c)))
        .collect()
}

/// Whether a summary later in the log stands in for the record at `seq`.
///
/// A summary counts only for records written before it: the fold was taken
/// over what existed then, and folds after it may cover a wider stretch.
pub fn covered(seq: u64, folds: &[(u64, Coverage)]) -> bool {
    folds
        .iter()
        .any(|(at, c)| *at > seq && c.first <= seq && seq <= c.last)
}

/// The roles no view carries: the human's list and the keys they pressed are
/// the surface's state, never a turn (ADR 29). Named once so the reader that
/// skips them and the view that drops them cannot drift apart.
pub const NOT_TURNS: &[Role] = &[Role::Queue, Role::Bill, Role::Usage];

fn view_of(entries: &[Entry]) -> Vec<Entry> {
    // A queue snapshot is not a turn and not a token: it is the human's list,
    // read by the surface rather than by the model (ADR 29).
    let entries = conversation(entries);
    let kept: Vec<&Entry> = entries
        .iter()
        .filter(|entry| !NOT_TURNS.contains(&entry.role))
        .collect();
    let folds = folds(kept.iter().copied());
    let mut shown: Vec<Entry> = kept
        .iter()
        .filter(|e| !covered(e.seq, &folds))
        .map(|e| (*e).clone())
        .collect();
    // A summary stands where the range it covers stood, not where it was
    // appended: appending happens at the end, which would put the old turns'
    // digest after the new turns it is supposed to come before.
    shown.sort_by_key(Entry::accounts_from);
    shown
}

/// The stretch of the log the last launch began.
///
/// A session file is every run in a directory, and a request is one
/// conversation. A log written before the launch mark existed has no marker of
/// where a run started, so it is the conversation it holds.
fn conversation(entries: &[Entry]) -> &[Entry] {
    match entries.iter().rposition(|e| e.role == Role::Launch) {
        Some(at) => &entries[at + 1..],
        None => entries,
    }
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

/// The most of a folded output's first line that is kept. A shell result
/// opens with the operation's handle, which is how the model asks to undo it;
/// the rest of that line is the output itself.
const HEAD_CHARS: usize = 60;

/// Below this an output is cheaper to carry than to describe.
const KEEP_BYTES: usize = 200;

/// Shorten the older outputs in a conversation to a line saying what was
/// there, and return how many were shortened.
///
/// The newest `keep` outputs are carried whole: they are the work in hand.
/// Of the older ones only whole groups of `step` are folded, so the line they
/// are folded at moves once every `step` rounds instead of every round. That
/// is what a prefix cache is paid for -- the bytes before the line are the
/// same on two consecutive requests, and an endpoint charges a tenth for
/// them. A line that moved every round would rewrite the middle of the
/// conversation each time and make everything after it a miss.
///
/// `output` says which messages are tool results and `text` reaches the one
/// field that changes. The conversation is a copy either way: the log keeps
/// every output whole, and a human reading the file finds what the command
/// actually printed (ADR 7).
pub fn fold_results<T>(
    messages: &mut [T],
    output: impl Fn(&T) -> bool,
    text: impl Fn(&mut T) -> &mut String,
    keep: usize,
    step: usize,
) -> usize {
    let step = step.max(1);
    let total = messages.iter().filter(|m| output(m)).count();
    let take = total.saturating_sub(keep) / step * step;
    let mut seen = 0;
    let mut folded = 0;
    for m in messages.iter_mut() {
        if !output(m) {
            continue;
        }
        if seen >= take {
            break;
        }
        seen += 1;
        let body = text(m);
        if body.len() <= KEEP_BYTES {
            continue;
        }
        *body = stub(body);
        folded += 1;
    }
    folded
}

/// What stands in for an output: how much was there, and the line it opened
/// with. Derived from the output alone, so the same output always folds to
/// the same bytes however much later it is read.
fn stub(content: &str) -> String {
    let lines = content.lines().count();
    let word = if lines == 1 { "line" } else { "lines" };
    let first = content.lines().next().unwrap_or_default();
    let mut head: String = first.chars().take(HEAD_CHARS).collect();
    if first.chars().count() > HEAD_CHARS {
        head.push('\u{2026}');
    }
    format!(
        "[folded: {} bytes, {lines} {word}] {head}",
        content.len()
    )
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
        let weight = meter.total(entries[cut - 1].weighed());
        if n - cut >= budget.keep_recent && kept + weight > budget.tail_tokens {
            break;
        }
        kept += weight;
        cut -= 1;
    }
    // A tool result kept without the call that asked for it is a request the
    // endpoint refuses, and one folded away from its call leaves the model an
    // answer with no question. A user turn is the seam the model reads best;
    // a task that is one user turn and forty tool rounds has none of them in
    // its tail, so the next turn that is not a tool result will do. Before
    // this a single-task session -- the normal shape, one launch and one
    // request -- never found a seam and never folded at all.
    // The seam is the nearest turn that keeps the tail whole: a user turn if
    // one is in reach, otherwise any turn but a tool result. Forwards first
    // (the tail stays inside the budget), then backwards (a shorter fold is
    // still a fold).
    let from = cut.max(1);
    let fits = |j: usize| n - j >= budget.keep_recent;
    let user = |j: usize| entries[j].role == Role::User;
    let turn = |j: usize| entries[j].role != Role::Tool;
    let Some(cut) = (from..n)
        .find(|&j| fits(j) && user(j))
        .or_else(|| (1..from).rev().find(|&j| fits(j) && user(j)))
        .or_else(|| (from..n).find(|&j| fits(j) && turn(j)))
        .or_else(|| (1..from).rev().find(|&j| fits(j) && turn(j)))
    else {
        return None;
    };
    let (fold, keep) = entries.split_at(cut);
    if meter.total(fold.iter().flat_map(|e| e.weighed())) < budget.min_fold_tokens {
        return None;
    }
    Some(Split {
        fold: fold.to_vec(),
        keep: keep.to_vec(),
    })
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
        None => Projection {
            summary: String::new(),
            retained: entries.to_vec(),
            covered: None,
        },
        Some(p) => {
            let summary = summarizer.summarize(&p.fold).await;
            let covered = p.coverage();
            Projection {
                summary,
                retained: p.keep,
                covered,
            }
        }
    }
}
