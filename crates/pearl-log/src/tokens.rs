//! tokens --- how full the context is, measured rather than guessed.
//!
//! Two numbers decide when history is folded: the provider's billed
//! `prompt_tokens`, which is exact but arrives after the fact, and a local
//! estimate, which is available before the request goes out. The estimate is
//! a character count with one correction that matters: CJK text costs about a
//! token per character, while Latin text costs about one per four, so a plain
//! `len / 4` under-reports a Chinese conversation by four times.
//!
//! The estimate is then calibrated against what the provider actually billed.
//! One log does not need a tokenizer for that; it needs to know when its
//! own count is off and by how much.

/// CJK ideographs, kana, and Hangul: each is roughly one token.
fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30ff | 0x4e00..=0x9fff | 0xac00..=0xd7af)
}

/// Estimate the tokens in `text` without a tokenizer: CJK characters count
/// once, everything else a quarter of a character, rounded up so a short
/// string is never free.
pub fn estimate_text(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    let ascii = text.chars().count() - cjk;
    ascii.div_ceil(4) as u32 + cjk as u32
}

/// Below this the billed figure is mostly prompt template, so it teaches
/// nothing about how the estimator reads this conversation.
const MIN_CALIBRATION_ESTIMATE: u32 = 2_000;

/// A billed figure this far from the estimate is not the same conversation:
/// a different route, or a request we did not measure. It must not teach.
const SAMPLE_MIN: f64 = 0.2;
const SAMPLE_MAX: f64 = 5.0;

/// The published factor stays inside this band, so one wild sample cannot
/// swing the accounting for the rest of the log.
const WEIGHT_MIN: f64 = 0.25;
const WEIGHT_MAX: f64 = 4.0;

/// A local estimate of context size, corrected by what the provider billed.
///
/// The correction is one number: the ratio between billed and estimated. It is
/// learned from completed requests, bounded on both sides so a single sample
/// with a different shape cannot distort the log, and it is what makes the
/// character count into a usable meter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    weight: f64,
    samples: u32,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            weight: 1.0,
            samples: 0,
        }
    }
}

impl Meter {
    /// A meter that trusts its own estimate until a request is billed.
    pub fn new() -> Self {
        Self::default()
    }

    /// The factor currently applied to the raw estimate.
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// How many billed requests have taught this meter.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Estimate the tokens in `text`, in the scale the provider bills.
    pub fn tokens(&self, text: &str) -> u32 {
        let raw = f64::from(estimate_text(text));
        (raw * self.weight).round() as u32
    }

    /// Total estimated size of a set of texts.
    pub fn total<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> u32 {
        texts.into_iter().map(|t| self.tokens(t)).sum()
    }

    /// Teach the meter what a request actually cost. `estimated` is what this
    /// meter predicted for the messages that were sent; `billed` is what the
    /// provider reported. A sample outside the band, or one taken on a request
    /// too small to measure, is ignored rather than averaged in.
    pub fn observe(&mut self, estimated: u32, billed: u32) {
        if estimated < MIN_CALIBRATION_ESTIMATE || billed == 0 {
            return;
        }
        let ratio = f64::from(billed) / f64::from(estimated);
        if !(SAMPLE_MIN..=SAMPLE_MAX).contains(&ratio) {
            return;
        }
        self.weight = ratio.clamp(WEIGHT_MIN, WEIGHT_MAX);
        self.samples += 1;
    }
}
