//! gauge --- what the window is holding, where the human can see it.
//!
//! A fold that happens silently is indistinguishable from a model that forgot.
//! The runner reports what a request cost and when history folded; the surface
//! reads the same handle, so the fill of the window is on the screen while the
//! work runs rather than in a log afterwards.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[derive(Debug, Default)]
struct Reading {
    prompt_tokens: u32,
    folds: u32,
    cost: crate::Cost,
    /// Cumulative, because a per-request rate jumps with every round and the
    /// figure worth watching is what the log has saved so far.
    prompt_total: u64,
    cache_hit: u64,
}

/// The live cost of the conversation, shared by the runner and the surface.
#[derive(Debug, Clone, Default)]
pub struct Gauge {
    reading: Arc<Mutex<Reading>>,
}

impl Gauge {
    pub fn new() -> Self {
        Self::default()
    }

    fn reading(&self) -> MutexGuard<'_, Reading> {
        self.reading.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What the last request was billed for, in the provider's own tokens.
    pub fn spent(&self, prompt_tokens: u32) {
        self.reading().prompt_tokens = prompt_tokens;
    }

    /// Record a request's billed prompt and how much of it the provider
    /// served from cache.
    pub fn spent_with_cache(&self, prompt_tokens: u32, cached: u32) {
        let mut r = self.reading();
        r.prompt_tokens = prompt_tokens;
        r.prompt_total += u64::from(prompt_tokens);
        r.cache_hit += u64::from(cached);
    }

    /// The share of this log's prompt tokens served from cache, or `None`
    /// before anything has been sent -- an unknown rate is not a zero rate.
    pub fn cache_rate(&self) -> Option<u32> {
        let r = self.reading();
        if r.prompt_total == 0 {
            return None;
        }
        Some((r.cache_hit * 100 / r.prompt_total) as u32)
    }

    pub fn prompt_tokens(&self) -> u32 {
        self.reading().prompt_tokens
    }

    /// Add one round's cost to the running total.
    pub fn charge(&self, cost: crate::Cost) {
        self.reading().cost.add(cost);
    }

    /// What the whole directory has cost so far.
    pub fn cost(&self) -> crate::Cost {
        self.reading().cost
    }

    /// History folded once more.
    pub fn folded(&self) {
        self.reading().folds += 1;
    }

    pub fn folds(&self) -> u32 {
        self.reading().folds
    }

    /// How full the window is, on its own. Thousands, because a token count to
    /// the unit is noise at this scale. This is the one reading `w` and `W`
    /// change, so a surface can put the keys beside it and nowhere else.
    pub fn fill(&self, window: u32) -> String {
        let r = self.reading();
        format!("ctx {}k/{}k", r.prompt_tokens / 1000, window / 1000)
    }

    /// The same climb, told with the point history resets at instead of the
    /// window's ceiling: `ctx 22k→48k` says how near the page is to folding,
    /// which is the number a person watching a night of work acts on. It moves
    /// with `w` and `W` because the fold point is a fraction of the window, so
    /// the hint stays beside this reading and nowhere else.
    pub fn climb(&self, fold: u32) -> String {
        let r = self.reading();
        format!("ctx {}k→{}k", r.prompt_tokens / 1000, fold / 1000)
    }

    /// What follows the window: how often history has folded, and what the
    /// cache did. Neither is a knob, so neither carries a key beside it -- a
    /// hint that outlived its reading moves when the reading does, and points
    /// at the wrong number.
    pub fn beside(&self) -> Vec<String> {
        let r = self.reading();
        let mut out = Vec::new();
        if r.folds > 0 {
            out.push(format!("folded {}x", r.folds));
        }
        if r.prompt_total > 0 {
            let rate = (r.cache_hit * 100 / r.prompt_total) as u32;
            out.push(format!("cache {rate}%"));
        }
        out
    }

    /// The whole reading as one line, for the surfaces that have one line.
    pub fn describe(&self, window: u32) -> String {
        let mut line = self.fill(window);
        for part in self.beside() {
            line = format!("{line} | {part}");
        }
        line
    }
}
