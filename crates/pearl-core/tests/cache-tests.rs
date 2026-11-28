//! Stories for the cache reading on the gauge.

use pearl_core::Gauge;
use rstest::rstest;

#[rstest]
fn given_no_request_yet_when_the_reading_is_shown_then_no_rate_is_claimed() {
    let g = Gauge::new();
    assert!(
        !g.describe(128_000).contains("cache"),
        "a rate was claimed before anything was sent"
    );
}

#[rstest]
fn given_two_requests_when_the_rate_is_shown_then_it_is_over_both_not_the_last() {
    let g = Gauge::new();
    g.spent_with_cache(1000, 1000);
    g.spent_with_cache(1000, 0);
    assert_eq!(g.cache_rate(), Some(50));
    // The rate is not an item of its own any more: the money it belongs to
    // says the two halves, and a third number saying the same thing as a ratio
    // is one the human has to divide before it means anything.
    assert!(
        !g.describe(128_000).contains("cache"),
        "the row still carries a rate of its own: {}",
        g.describe(128_000)
    );
}

#[rstest]
fn given_a_server_that_reports_no_cache_when_the_rate_is_shown_then_it_says_zero() {
    let g = Gauge::new();
    g.spent_with_cache(1000, 0);
    assert_eq!(g.cache_rate(), Some(0));
}
