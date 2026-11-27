//! Stories for what work cost, at three scopes.
//!
//! The same ledger read as a turn, a task and a session: the readings have to
//! add up, or they are three opinions rather than one measurement.

use pearl_core::{Cost, Gauge, Queue, Turn};
use rstest::rstest;

#[rstest]
fn given_two_turns_when_their_costs_are_added_then_the_totals_are_the_sum() {
    let mut total = Cost::turn(1200, 300, 2400);
    total.add(Cost::turn(800, 120, 600));
    assert_eq!(total.up, 2000);
    assert_eq!(total.down, 420);
    assert_eq!(total.ms, 3000);
    assert_eq!(total.turns, 2);
}

#[rstest]
fn given_a_cost_when_it_is_written_then_it_is_short_and_unitful() {
    assert_eq!(Cost::turn(1234, 340, 2400).describe(), "↑1.2k ↓340 2.4s");
    assert_eq!(Cost::turn(900, 12, 500).describe(), "↑900 ↓12 0.5s");
    assert_eq!(Cost::turn(1000, 10, 90_000).describe(), "↑1.0k ↓10 1m30s");
}

#[rstest]
fn given_a_turn_that_knows_its_work_when_it_is_written_then_the_work_leads_the_cost() {
    let turn = Turn::doing(Cost::turn(1000, 100, 500), "read crates/x.rs");
    assert_eq!(turn.describe(), "read crates/x.rs  ↑1.0k ↓100 0.5s");
}

#[rstest]
fn given_a_turn_nobody_could_name_when_it_is_written_then_it_is_only_its_cost() {
    let turn: Turn = Cost::turn(1000, 100, 500).into();
    assert_eq!(
        turn.describe(),
        "↑1.0k ↓100 0.5s",
        "an unnamed turn grew an empty column"
    );
}

#[rstest]
fn given_nothing_measured_when_a_cost_is_asked_about_then_it_says_it_is_empty() {
    assert!(
        Cost::default().is_empty(),
        "an unrun task would claim a zero cost"
    );
    assert!(!Cost::turn(1, 1, 1).is_empty());
}

#[rstest]
fn given_a_task_that_ran_twice_when_its_cost_is_read_then_it_holds_both_turns() {
    let q = Queue::new();
    let (id, _) = q.append("work");
    q.start(id).unwrap();
    q.charge(id, Cost::turn(1000, 100, 500));
    q.charge(id, Cost::turn(500, 50, 250));
    let cost = q.get(id).unwrap().cost;
    assert_eq!(
        cost,
        Cost {
            up: 1500,
            down: 150,
            ms: 750,
            turns: 2
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
fn given_a_session_of_two_tasks_when_the_gauge_is_read_then_it_holds_both() {
    let g = Gauge::new();
    g.charge(Cost::turn(1000, 100, 500));
    g.charge(Cost::turn(2000, 200, 1500));
    assert_eq!(
        g.cost(),
        Cost {
            up: 3000,
            down: 300,
            ms: 2000,
            turns: 2
        }
    );
}

#[rstest]
fn given_a_child_task_when_it_is_charged_then_the_parent_is_not_charged_twice() {
    let q = Queue::new();
    let (parent, _) = q.append("the whole");
    let (child, _) = q.add_child(parent, "a part").unwrap();
    q.charge(child, Cost::turn(100, 10, 50));
    assert!(
        q.get(parent).unwrap().cost.is_empty(),
        "the parent was charged for its child's turn"
    );
    assert_eq!(q.get(child).unwrap().cost.up, 100);
}

#[rstest]
fn given_a_turn_that_read_more_than_it_added_when_it_is_written_then_it_says_both_numbers() {
    let turn = Turn::reading(Cost::turn(1500, 20, 2000), 500, "read crates/x.rs");
    assert_eq!(
        turn.describe(),
        "read crates/x.rs  ↑500 of 1.5k ↓20 2.0s",
        "a round that re-read the conversation claimed all of it as new"
    );
}

#[rstest]
fn given_a_first_turn_that_added_all_it_read_when_it_is_written_then_it_says_it_once() {
    let turn = Turn::reading(Cost::turn(1000, 10, 500), 1000, "read crates/x.rs");
    assert_eq!(
        turn.describe(),
        "read crates/x.rs  ↑1.0k ↓10 0.5s",
        "a first round compared its prompt with itself"
    );
}

#[rstest]
fn given_a_turn_from_before_the_reading_was_kept_when_it_is_written_then_it_is_its_bill() {
    // A log written by an older pearl has no growth on its bills. Reading it
    // must not print `↑0 of` or invent a second number it never measured.
    let turn: Turn = serde_json::from_str(
        r#"{"cost":{"up":1500,"down":20,"ms":2000,"turns":1},"note":"answer"}"#,
    )
    .expect("an older bill did not read");
    assert_eq!(turn.describe(), "answer  ↑1.5k ↓20 2.0s");
}
