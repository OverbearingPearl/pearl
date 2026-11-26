//! cost --- what a piece of work took, in tokens and in time.
//!
//! One ledger read at three scopes: a turn, a task, a session. The numbers are
//! the provider's own counts and the clock's own elapsed time, so the three
//! readings add up rather than being three opinions about the same work.

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
    pub turns: u32,
}

impl Cost {
    /// A single request's cost.
    pub fn turn(up: u32, down: u32, ms: u64) -> Self {
        Self {
            up,
            down,
            ms,
            turns: 1,
        }
    }

    /// Add another request's cost to this one.
    pub fn add(&mut self, other: Cost) {
        self.up = self.up.saturating_add(other.up);
        self.down = self.down.saturating_add(other.down);
        self.ms = self.ms.saturating_add(other.ms);
        self.turns = self.turns.saturating_add(other.turns);
    }

    /// Whether anything has been measured yet. An empty cost is not shown:
    /// `↑0 ↓0 0.0s` on a task that has not run is a claim, not a reading.
    pub fn is_empty(&self) -> bool {
        self.turns == 0
    }

    /// A short reading for a line: `↑1.2k ↓340 2.4s`.
    pub fn describe(&self) -> String {
        format!(
            "↑{} ↓{} {}",
            tokens(self.up),
            tokens(self.down),
            elapsed(self.ms)
        )
    }
}

/// One request: the bill it took and, in a few words, what it was doing.
///
/// A bill on its own is a number that no round owns -- `turn 3  ↑12.4k` says a
/// round was expensive without saying what it spent the money on. The note is
/// the work: the tool call it made, or `answer` when it was the last turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    /// The provider's bill for this one request.
    pub cost: Cost,
    /// What the request asked for, in the width of a line.
    pub note: String,
}

impl Turn {
    /// A turn that knows what it did.
    pub fn doing(cost: Cost, note: impl Into<String>) -> Self {
        Self {
            cost,
            note: note.into(),
        }
    }

    /// A short reading for a line: `read crates/x.rs  ↑1.2k ↓340 0.4s`.
    pub fn describe(&self) -> String {
        if self.note.is_empty() {
            self.cost.describe()
        } else {
            format!("{}  {}", self.note, self.cost.describe())
        }
    }
}

/// A cost with no work attached still has to be a turn: a caller that knows
/// only the bill (a fan-out child, a story) should not have to invent a note.
impl From<Cost> for Turn {
    fn from(cost: Cost) -> Self {
        Self {
            cost,
            note: String::new(),
        }
    }
}

/// Tokens with a `k` once they are worth one, so a five-figure count does not
/// push the title off the screen.
fn tokens(n: u32) -> String {
    if n >= 1000 {
        format!("{:.1}k", f64::from(n) / 1000.0)
    } else {
        n.to_string()
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
