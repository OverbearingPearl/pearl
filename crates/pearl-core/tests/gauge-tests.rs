//! Stories for the window being visible.
//!
//! A fold that happens silently is indistinguishable from a model that forgot.
//! The human is owed the fill of the window and the fact that history folded,
//! while they are looking at the queue rather than after they quit.

use pearl_core::Gauge;
use rstest::rstest;

#[rstest]
fn given_a_shared_gauge_when_the_runner_reports_a_cost_then_the_surface_sees_it() {
    let gauge = Gauge::new();
    let surface = gauge.clone();
    gauge.spent(12_000);
    assert_eq!(surface.prompt_tokens(), 12_000);
}

#[rstest]
fn given_a_fold_when_it_is_recorded_then_the_count_rises() {
    let gauge = Gauge::new();
    assert_eq!(gauge.folds(), 0);
    gauge.folded();
    gauge.folded();
    assert_eq!(gauge.folds(), 2);
}

#[rstest]
fn given_a_gauge_and_a_window_when_it_is_described_then_it_reads_as_fill_and_folds() {
    let gauge = Gauge::new();
    gauge.spent(48_000);
    gauge.folded();
    let line = gauge.describe(128_000);
    assert!(line.contains("48"), "{line}");
    assert!(line.contains("128"), "{line}");
    assert!(line.contains("summarized 1x"), "{line}");
}

#[rstest]
fn given_no_cost_yet_when_it_is_described_then_it_says_nothing_is_measured() {
    let gauge = Gauge::new();
    let line = gauge.describe(128_000);
    assert!(line.contains("ctx"), "{line}");
    assert!(
        !line.contains("summarized"),
        "an empty gauge claimed a summary: {line}"
    );
}
