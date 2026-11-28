//! Stories for what work cost, at three scopes.
//!
//! The same ledger read as a round, a task and the directory: the readings
//! have to add up, or they are three opinions rather than one measurement.

use pearl_core::{Bill, Cost, Gauge, Queue};
use rstest::rstest;

#[rstest]
fn given_two_rounds_when_their_costs_are_added_then_the_totals_are_the_sum() {
    let mut total = Cost::round(1200, 300, 2400);
    total.add(Cost::round(800, 120, 600));
    assert_eq!(total.up, 2000);
    assert_eq!(total.down, 420);
    assert_eq!(total.ms, 3000);
    assert_eq!(total.rounds, 2);
}

#[rstest]
fn given_a_cost_when_it_is_written_then_it_is_short_and_unitful() {
    assert_eq!(Cost::round(1234, 340, 2400).describe(), "↑1.2k+0 ↓340 2.4s");
    assert_eq!(Cost::round(900, 12, 500).describe(), "↑900+0 ↓12 0.5s");
    assert_eq!(Cost::round(1000, 10, 90_000).describe(), "↑1.0k+0 ↓10 1m30s");
}

#[rstest]
fn given_a_night_that_read_millions_when_its_cost_is_written_then_the_unit_climbs_with_it() {
    // A task that reads the same conversation four hundred times passes a
    // million tokens before lunch. The reading has to climb with it: `1456.7k`
    // is a million spelled the long way, and `1050.0k` rounds a thousand into
    // the unit above it.
    assert_eq!(
        Cost::round(1_456_700, 340, 2400).describe(),
        "↑1.5m+0 ↓340 2.4s"
    );
    assert_eq!(Cost::round(1_050_000, 340, 2400).describe(), "↑1.1m+0 ↓340 2.4s");
    assert_eq!(
        Cost::round(999_949, 340, 2400).describe(),
        "↑999.9k+0 ↓340 2.4s"
    );
    assert_eq!(
        Cost::round(999_950, 340, 2400).describe(),
        "↑1.0m+0 ↓340 2.4s",
        "a thousand thousands became `1000.0k`"
    );
}

#[rstest]
fn given_a_round_the_cache_served_when_it_is_billed_then_the_prompt_says_which_half_was_new() {
    // The number that decides the bill is not the total but how much of it was
    // already known: `↑6.8k+77.8k` says the tenth round of a long task spent
    // six thousand new tokens, where `↑84.6k` looks ruinous and was not.
    let mut cost = Cost::round(84_600, 334, 2500);
    cost.cached = 77_800;
    assert_eq!(cost.describe(), "↑6.8k+77.8k ↓334 2.5s");
}

#[rstest]
fn given_two_halves_of_one_reading_when_they_are_written_then_they_share_a_unit() {
    // Two numbers in one `↑` are one reading, and a reading that said
    // `800+84.0k` would make the eye divide before it could compare.
    let mut small = Cost::round(84_800, 334, 2500);
    small.cached = 84_000;
    assert_eq!(small.describe(), "↑0.8k+84.0k ↓334 2.5s");

    let mut nothing = Cost::round(900, 334, 2500);
    nothing.cached = 400;
    assert_eq!(
        nothing.describe(),
        "↑500+400 ↓334 2.5s",
        "a pair under a thousand needs no unit"
    );
}

#[rstest]
fn given_a_round_the_cache_did_not_serve_when_it_is_billed_then_the_cache_half_is_a_zero() {
    let mut cold = Cost::round(84_600, 334, 2500);
    cold.cached = 0;
    assert_eq!(
        cold.describe(),
        "↑84.6k+0 ↓334 2.5s",
        "a cold round dropped the half that says nothing came from the cache"
    );
    let mut all = Cost::round(84_600, 334, 2500);
    all.cached = 84_600;
    assert_eq!(
        all.describe(),
        "↑0+84.6k ↓334 2.5s",
        "a prompt the cache served entire is still a prompt that was sold"
    );
}

#[rstest]
fn given_a_round_that_knows_its_work_when_it_is_billed_then_the_work_leads_the_cost() {
    let bill = Bill::doing(Cost::round(1000, 100, 500), "read crates/x.rs");
    assert_eq!(bill.describe(), "read crates/x.rs  ↑1.0k+0 ↓100 0.5s");
}

#[rstest]
fn given_a_round_nobody_could_name_when_it_is_billed_then_it_is_only_its_cost() {
    let bill: Bill = Cost::round(1000, 100, 500).into();
    assert_eq!(
        bill.describe(),
        "↑1.0k+0 ↓100 0.5s",
        "an unnamed bill grew an empty column"
    );
}

#[rstest]
fn given_nothing_measured_when_a_cost_is_asked_about_then_it_says_it_is_empty() {
    assert!(
        Cost::default().is_empty(),
        "an unrun task would claim a zero cost"
    );
    assert!(!Cost::round(1, 1, 1).is_empty());
}

#[rstest]
fn given_a_task_that_ran_twice_when_its_cost_is_read_then_it_holds_both_rounds() {
    let q = Queue::new();
    let (id, _) = q.append("work");
    q.start(id).unwrap();
    q.charge(id, Cost::round(1000, 100, 500));
    q.charge(id, Cost::round(500, 50, 250));
    let cost = q.get(id).unwrap().cost;
    assert_eq!(
        cost,
        Cost {
            up: 1500,
            down: 150,
            ms: 750,
            rounds: 2,
            cached: 0
        }
    );
}

#[rstest]
fn given_a_task_that_never_ran_when_its_cost_is_read_then_there_is_none() {
    let q = Queue::new();
    let (id, _) = q.append("work");
    assert!(q.get(id).unwrap().cost.is_empty());
}

#[rstest]
fn given_a_directory_of_two_tasks_when_the_gauge_is_read_then_it_holds_both() {
    let g = Gauge::new();
    g.charge(Cost::round(1000, 100, 500));
    g.charge(Cost::round(2000, 200, 1500));
    assert_eq!(
        g.cost(),
        Cost {
            up: 3000,
            down: 300,
            ms: 2000,
            rounds: 2,
            cached: 0
        }
    );
}

#[rstest]
fn given_a_child_task_when_it_is_charged_then_the_parent_is_not_charged_twice() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.charge(child, Cost::round(100, 10, 50));
    assert!(
        q.get(parent).unwrap().cost.is_empty(),
        "the parent was charged for its child's round"
    );
    assert_eq!(q.get(child).unwrap().cost.up, 100);
}

#[rstest]
fn given_a_round_that_read_more_than_it_added_when_it_is_billed_then_it_says_both_numbers() {
    let bill = Bill::reading(Cost::round(1500, 20, 2000), 500, "read crates/x.rs");
    assert_eq!(
        bill.describe(),
        "read crates/x.rs  ↑0.5k+1.0k ↓20 2.0s",
        "a round that re-read the conversation claimed all of it as new, or \
         compared two numbers in two units"
    );
}

#[rstest]
fn given_a_first_round_that_added_all_it_read_when_it_is_billed_then_the_rest_is_a_zero() {
    let bill = Bill::reading(Cost::round(1000, 10, 500), 1000, "read crates/x.rs");
    assert_eq!(
        bill.describe(),
        "read crates/x.rs  ↑1.0k+0 ↓10 0.5s",
        "a first round dropped the half that says nothing was already known"
    );
}

#[rstest]
fn given_a_bill_from_before_the_reading_was_kept_when_it_is_read_then_it_is_still_a_bill() {
    // A log written by an older pearl has no growth on its bills. Reading it
    // must not print `↑0 of`: the bill falls back to its plain description,
    // whose second half reads zero.
    let bill: Bill = serde_json::from_str(
        r#"{"cost":{"up":1500,"down":20,"ms":2000,"rounds":1},"note":"answer"}"#,
    )
    .expect("an older bill did not read");
    assert_eq!(bill.describe(), "answer  ↑1.5k+0 ↓20 2.0s");
}
