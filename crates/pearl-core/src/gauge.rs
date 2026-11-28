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

    /// The same climb, told with what the page is measured against: `ctx
    /// 22k/128k·⅜` says how near the history is to folding, which is what a
    /// person watching a night of work acts on, and it names both numbers that
    /// decide it -- the ceiling `w` and `W` change, and the share of it the
    /// trigger sets. The share is spelled against the ceiling rather than as a
    /// token count so that the knob beside the reading is the one the reading
    /// is about.
    pub fn fill_and_fold(&self, window: u32, fold: u32) -> String {
        let r = self.reading();
        format!(
            "ctx {}k/{}k·{}",
            r.prompt_tokens / 1000,
            window / 1000,
            share(fold, window)
        )
    }

    /// What follows the window: how often history has folded.
    ///
    /// The cache is not here. It used to be, as `cache 86%`, and it was the
    /// same fact as the split on the money at the end of the same row --
    /// `↑1.2k+7.4k` says what was new and what came back, in the two numbers
    /// the bill was made of, where a rate was a third number the human had to
    /// divide before it meant anything. One row, one spelling.
    pub fn beside(&self) -> Vec<String> {
        let r = self.reading();
        let mut out = Vec::new();
        if r.folds > 0 {
            out.push(format!("folded {}x", r.folds));
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

/// The share of a window the history folds at, spelled as the fraction a
/// person types when there is one -- `⅜` for the three eighths the trigger
/// defaults to -- and as a percentage when there is not. A reading that said
/// `38%` over a knob set to `0.375` would name a number nobody set, and the
/// fractions a night is run at are short enough to say whole.
fn share(fold: u32, window: u32) -> String {
    const GLYPHS: [(u32, u32, &str); 16] = [
        (1, 2, "½"),
        (1, 3, "⅓"),
        (2, 3, "⅔"),
        (1, 4, "¼"),
        (3, 4, "¾"),
        (1, 5, "⅕"),
        (2, 5, "⅖"),
        (3, 5, "⅗"),
        (4, 5, "⅘"),
        (1, 6, "⅙"),
        (5, 6, "⅚"),
        (1, 8, "⅛"),
        (3, 8, "⅜"),
        (5, 8, "⅝"),
        (7, 8, "⅞"),
        (1, 10, "⅒"),
    ];
    let window = window.max(1);
    for (n, d, glyph) in GLYPHS {
        if u64::from(fold) * u64::from(d) == u64::from(window) * u64::from(n) {
            return glyph.to_string();
        }
    }
    let percent = f64::from(fold) * 100.0 / f64::from(window);
    if (percent - percent.round()).abs() < 0.05 {
        format!("{}%", percent.round() as u32)
    } else {
        format!("{percent:.1}%")
    }
}
