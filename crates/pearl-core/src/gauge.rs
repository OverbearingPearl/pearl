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

    pub fn prompt_tokens(&self) -> u32 {
        self.reading().prompt_tokens
    }

    /// History folded once more.
    pub fn folded(&self) {
        self.reading().folds += 1;
    }

    pub fn folds(&self) -> u32 {
        self.reading().folds
    }

    /// One line for the surface: how full the window is, and how many times
    /// history has folded. Thousands, because a token count to the unit is
    /// noise at this scale.
    pub fn describe(&self, window: u32) -> String {
        let r = self.reading();
        let fill = format!("ctx {}k/{}k", r.prompt_tokens / 1000, window / 1000);
        if r.folds == 0 {
            fill
        } else {
            format!("{fill}, {} fold{}", r.folds, if r.folds == 1 { "" } else { "s" })
        }
    }
}
