//! cost --- what a piece of work took, in tokens and in time.
//! One ledger read at three scopes: a round, a task, and the directory. The
//! numbers are the provider's own counts and the clock's own elapsed time, so
//! the three readings add up rather than being three opinions about the same
//! work.

use serde::{Deserialize, Serialize};

/// Tokens up, tokens down, and how long the waiting was.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    /// Prompt tokens billed, including the part the cache served.
    pub up: u32,
    /// Completion tokens billed.
    pub down: u32,
    /// Wall time spent waiting for the endpoint, in milliseconds.
    pub ms: u64,
    /// How many requests this covers.
    pub rounds: u32,
    /// How many of the prompt tokens the endpoint served from its cache.
    ///
    /// The part the cache served is billed at a fraction of the part it was
    /// not, so a prompt count without this one cannot be read as money. It
    /// belongs on the bill rather than only in memory: a window reopened on a
    /// directory reads the row's cost from the log, and a reading that was
    /// never written is a reading the next window does not have.
    #[serde(default, skip_serializing_if = "zero")]
    pub cached: u32,
}

/// Whether a count is worth writing down. A log written before the cache was
/// measured has no such field, and a bill that hit nothing has nothing to say.
fn zero(n: &u32) -> bool {
    *n == 0
}

impl Cost {
    /// A single request's cost.
    pub fn round(up: u32, down: u32, ms: u64) -> Self {
        Self {
            up,
            down,
            ms,
            rounds: 1,
            cached: 0,
        }
    }

    /// Add another request's cost to this one.
    pub fn add(&mut self, other: Cost) {
        self.up = self.up.saturating_add(other.up);
        self.down = self.down.saturating_add(other.down);
        self.ms = self.ms.saturating_add(other.ms);
        self.rounds = self.rounds.saturating_add(other.rounds);
        self.cached = self.cached.saturating_add(other.cached);
    }

    /// The share of the prompt the cache served, or `None` when nothing was
    /// billed: an unknown rate is not a zero rate.
    pub fn cache_rate(&self) -> Option<u32> {
        if self.up == 0 {
            return None;
        }
        Some(self.cached * 100 / self.up)
    }

    /// Whether anything has been measured yet. An empty cost is not shown:
    /// `↑0 ↓0 0.0s` on a task that has not run is a claim, not a reading.
    pub fn is_empty(&self) -> bool {
        self.rounds == 0
    }

    /// A short reading for a line: `↑1.2k ↓340 2.4s`.
    /// The time alone, for a row that has not been billed yet: a running task
    /// with no answer has a wait to show and no tokens to show beside it.
    pub fn describe_time(&self) -> String {
        elapsed(self.ms)
    }

    pub fn describe(&self) -> String {
        format!(
            "↑{} ↓{} {}",
            self.prompt(),
            tokens(self.down),
            elapsed(self.ms)
        )
    }

    /// The prompt, told as the two things it was: `6.8k+77.8k` is what this
    /// request sent new and what the cache handed back without being sent.
    ///
    /// The total on its own hides the only number that decides the bill: the
    /// tenth round of a task is cheap or dear depending on how much of it the
    /// cache already had, and `↑84.6k` says the same thing whether it was six
    /// thousand new tokens or eighty. Both halves are spelled in one unit
    /// because they are one reading -- `800+84.0k` would be two units in one
    /// number and no eye compares them.
    fn prompt(&self) -> String {
        if self.cached == 0 {
            return tokens(self.up);
        }
        // A provider that claims more cache than it billed is a provider
        // saying the prompt it charged for was known whole; the reading is
        // still the two halves, with nothing new in the first.
        let cached = self.cached.min(self.up);
        let (fresh, cached) = in_one_unit(self.up - cached, cached);
        format!("{fresh}+{cached}")
    }

    /// The same reading for a ledger total, which does not repeat the rate.
    ///
    /// A row already carries the window's rate beside the climb. A total that
    /// ends `2m30s cache 90%` names a second rate, of a different scope, on the
    /// same line, and neither says which is which. Money and time are the
    /// total's to report; a rate belongs to the reading it explains.
    pub fn describe_total(&self) -> String {
        format!(
            "↑{} ↓{} {}",
            tokens(self.up),
            tokens(self.down),
            elapsed(self.ms)
        )
    }

    /// The same reading with a different figure in the `↑` place.
    ///
    /// A round does not add the tokens it reads: it re-reads the conversation
    /// and bills for all of it. A bill that says what it added has to print two
    /// figures where `describe` prints one, and the money the row sums is the
    /// one that was billed.
    pub fn describe_as(&self, up: &str) -> String {
        format!("↑{up} ↓{} {}", tokens(self.down), elapsed(self.ms))
    }
}

/// One request: the bill it took and, in a few words, what it was doing.
///
/// A bill on its own is a number that no round owns -- `round 3  ↑12.4k` says a
/// round was expensive without saying what it spent the money on. The note is
/// the work: the tool call it made, or `answer` when it was the last round.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bill {
    /// The provider's bill for this one request.
    pub cost: Cost,
    /// How much of that prompt this round had not been billed for before.
    ///
    /// The bill above is the whole conversation the request read, because that
    /// is what a provider charges for. The growth is the part that is new, and
    /// a row of bills that climb is the conversation climbing rather than each
    /// round spending more. `None` when the round is the first of its task, or
    /// when the record was written before this was measured: there is nothing
    /// to compare it with, and `↑1.0k of 1.0k` says one thing twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<u32>,
    /// What the request asked for, in the width of a line.
    pub note: String,
}

impl Bill {
    /// A bill for a round that knows what it did.
    pub fn doing(cost: Cost, note: impl Into<String>) -> Self {
        Self {
            cost,
            added: None,
            note: note.into(),
        }
    }

    /// A bill for a round that knows what it did and how much of what it read
    /// was new.
    pub fn reading(cost: Cost, added: u32, note: impl Into<String>) -> Self {
        Self {
            cost,
            added: Some(added),
            note: note.into(),
        }
    }

    /// The bill the way a line prints it.
    ///
    /// `↑1.5k` is what the provider charged; `↑500 of 1.5k` is the same charge
    /// with the part that round added in front of it. The second is only said
    /// when there is a difference to say, which is why the first round of a
    /// task, or a bill read back from an older log, prints the plain bill.
    pub fn bill(&self) -> String {
        match self.added {
            Some(added) if added < self.cost.up => {
                let (added, up) = in_one_unit(added, self.cost.up);
                self.cost.describe_as(&format!("{added} of {up}"))
            }
            _ => self.cost.describe(),
        }
    }

    /// A short reading for a line: `read crates/x.rs  ↑1.2k ↓340 0.4s`.
    pub fn describe(&self) -> String {
        if self.note.is_empty() {
            self.bill()
        } else {
            format!("{}  {}", self.note, self.bill())
        }
    }
}

/// A cost with no work attached still has to be a bill: a caller that knows
/// only the bill (a fan-out child, a story) should not have to invent a note.
impl From<Cost> for Bill {
    fn from(cost: Cost) -> Self {
        Self {
            cost,
            added: None,
            note: String::new(),
        }
    }
}

/// Tokens with a `k` once they are worth one, so a five-figure count does not
/// push the title off the screen.
fn tokens(n: u32) -> String {
    if n >= ROUNDS_TO_A_MILLION {
        million(n)
    } else if n >= 1000 {
        format!("{:.1}k", f64::from(n) / 1000.0)
    } else {
        n.to_string()
    }
}

/// Where `k` stops being the largest unit: past this, one decimal of a
/// thousand rounds up to `1000.0k`, which is a million spelled the long way.
const ROUNDS_TO_A_MILLION: u32 = 999_950;

fn million(n: u32) -> String {
    format!("{:.1}m", f64::from(n) / 1_000_000.0)
}

/// Two counts that make up one reading, in the unit of the larger so that the
/// pair reads as arithmetic rather than as two measurements.
fn in_one_unit(a: u32, b: u32) -> (String, String) {
    let top = a.max(b);
    if top >= ROUNDS_TO_A_MILLION {
        (million(a), million(b))
    } else if top >= 1000 {
        let k = |n: u32| format!("{:.1}k", f64::from(n) / 1000.0);
        (k(a), k(b))
    } else {
        (a.to_string(), b.to_string())
    }
}

/// Elapsed time in seconds with one decimal below a minute, then minutes.
fn elapsed(ms: u64) -> String {
    let secs = ms as f64 / 1000.0;
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        format!("{}m{:02.0}s", (secs / 60.0).floor(), secs % 60.0)
    }
}
