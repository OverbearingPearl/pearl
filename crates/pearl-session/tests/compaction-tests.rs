//! Stories for compaction: a view over the log, priced in tokens.

use pearl_session::{
    Budget, Coverage, Entry, FoldSummary, Meter, Role, estimate_text, project, split, view, view_in,
};
use rstest::rstest;
use tempfile::TempDir;

/// A budget small enough that a handful of short entries can exceed it.
fn budget(keep_recent: usize, tail_tokens: u32) -> Budget {
    Budget { keep_recent, tail_tokens, min_fold_tokens: 1 }
}

fn user(seq: u64, text: impl Into<String>) -> Entry {
    Entry::new(seq, Role::User, text)
}

#[rstest]
fn given_chinese_text_when_estimated_then_a_character_counts_as_a_token() {
    // Four ideographs are four tokens, where len/4 would have said one.
    assert_eq!(estimate_text("你好世界"), 4);
    // Latin text is about a quarter of a character per token.
    assert_eq!(estimate_text("hello"), 2);
    assert_eq!(estimate_text(""), 0);
    // Mixed text counts each part on its own scale.
    assert_eq!(estimate_text("ok 你好"), 3);
}

#[rstest]
fn given_a_low_estimate_when_billed_then_the_meter_learns_the_scale() {
    let mut meter = Meter::new();
    assert_eq!(meter.tokens(&"x".repeat(8_000)), 2_000);
    // The provider billed four times what the character count predicted.
    meter.observe(2_000, 8_000);
    assert_eq!(meter.samples(), 1);
    assert_eq!(meter.tokens(&"x".repeat(8_000)), 8_000);
}

#[rstest]
fn given_a_request_too_small_to_measure_when_observed_then_the_meter_ignores_it() {
    let mut meter = Meter::new();
    // A billed figure under the calibration floor is prompt template, not a
    // reading of this conversation.
    meter.observe(100, 400);
    assert_eq!(meter.samples(), 0);
    assert_eq!(meter.weight(), 1.0);
}

#[rstest]
fn given_a_wild_sample_when_observed_then_the_weight_stays_inside_its_band() {
    let mut meter = Meter::new();
    meter.observe(2_000, 2_000 * 9);
    // Nine times is not this conversation: the sample is refused.
    assert_eq!(meter.samples(), 0);
    // Four times is accepted, and the published factor is clamped to its band.
    meter.observe(2_000, 8_000);
    assert_eq!(meter.weight(), 4.0);
}

#[rstest]
fn given_a_log_with_a_summary_when_viewed_then_the_covered_range_is_hidden_and_kept() {
    let log = vec![
        user(0, "first"),
        user(1, "second"),
        user(2, "third"),
        Entry::compaction(3, "the first two, folded", Coverage { first: 0, last: 1 }),
    ];
    let shown = view(&log);
    assert_eq!(shown.len(), 2);
    // The summary stands at the head of the range it covers, not at the end
    // where it was appended: a digest of the first turns belongs before them.
    assert_eq!(shown[0].role, Role::Compaction);
    assert_eq!(shown[1].seq, 2);
    // The log is a record, not a draft: the folded entries are still in it.
    assert_eq!(log.len(), 4);
    assert_eq!(log[0].content, "first");
}

#[rstest]
fn given_a_full_log_when_split_then_the_tail_fits_the_budget() {
    let entries: Vec<Entry> = (0..10).map(|i| user(i, &format!("m{i}"))).collect();
    let meter = Meter::new();
    let cut = split(&entries, &budget(3, 5), &meter).unwrap();
    assert_eq!(cut.keep.len(), 5);
    assert_eq!(cut.keep[0].seq, 5);
    assert_eq!(cut.fold.len(), 5);
    assert_eq!(cut.coverage(), Some(Coverage { first: 0, last: 4 }));
}

#[rstest]
fn given_a_result_small_enough_when_split_then_the_tail_starts_at_a_user_turn() {
    // The tool result would fit, but the call that produced it would not: a
    // result without its call is worse than a longer context, so the cut moves
    // forward until the kept tail begins at a turn the model can answer.
    let long = "x".repeat(40);
    let entries = vec![
        user(0, "u0"),
        Entry::new(1, Role::Assistant, "a1"),
        user(2, "u2"),
        Entry::new(3, Role::Assistant, "calling a tool"),
        Entry::new(4, Role::Tool, &long),
        user(5, "u5"),
        Entry::new(6, Role::Assistant, "a6"),
    ];
    let meter = Meter::new();
    let cut = split(&entries, &budget(1, 12), &meter).unwrap();
    assert_eq!(cut.keep[0].role, Role::User);
    assert_eq!(cut.keep[0].seq, 5);
    // The result was folded with the call it answers, not away from it.
    assert_eq!(cut.fold.last().unwrap().seq, 4);
}

#[rstest]
fn given_a_prefix_below_the_minimum_when_split_then_nothing_is_folded() {
    let entries: Vec<Entry> = (0..6).map(|i| user(i, &format!("m{i}"))).collect();
    let meter = Meter::new();
    let strict = Budget { keep_recent: 2, tail_tokens: 2, min_fold_tokens: 2_000 };
    // Folding four one-token entries would cost a request to reclaim nothing.
    assert!(split(&entries, &strict, &meter).is_none());
}

#[rstest]
fn given_a_log_that_fits_when_split_then_nothing_is_folded() {
    let entries: Vec<Entry> = (0..3).map(|i| user(i, &format!("m{i}"))).collect();
    let meter = Meter::new();
    assert!(split(&entries, &budget(3, 8_000), &meter).is_none());
}

#[rstest]
fn given_a_summary_at_the_head_when_folded_again_then_the_range_starts_where_it_did() {
    // A summary can only be folded again once it sits at the head of the view:
    // four entries written after it, two of which the tail budget keeps.
    let mut log: Vec<Entry> = (0..7).map(|i| user(i, format!("m{i}"))).collect();
    log.push(Entry::compaction(7, "folded once", Coverage { first: 0, last: 6 }));
    log.extend((8..12).map(|i| user(i, format!("m{i}"))));

    let meter = Meter::new();
    let shown = view(&log);
    assert_eq!(shown[0].role, Role::Compaction);

    let second = split(&shown, &budget(2, 3), &meter).unwrap();
    // The second summary accounts for the first one's whole range, so a reader
    // following it backwards reaches the start of the conversation.
    assert_eq!(second.fold[0].role, Role::Compaction);
    assert_eq!(second.coverage().unwrap().first, 0);
}

#[rstest]
#[tokio::test]
async fn given_a_long_log_when_projected_then_recent_kept_and_old_folded() {
    let entries: Vec<Entry> = (0..10).map(|i| user(i, &format!("m{i}"))).collect();
    let meter = Meter::new();
    let projection = project(&entries, &budget(3, 5), &FoldSummary, &meter).await;
    assert_eq!(projection.retained.len(), 5);
    assert_eq!(projection.retained[0].seq, 5);
    assert_eq!(projection.summary, "folded 5 entries (seq 0..=4)");
    assert_eq!(projection.covered, Some(Coverage { first: 0, last: 4 }));
    // The input log is untouched: projection is a view, not a rewrite.
    assert_eq!(entries.len(), 10);
}

#[rstest]
fn given_one_user_turn_and_a_tail_of_tool_rounds_when_split_then_a_turn_is_kept_first() {
    // A task that is one request and forty tool results has no user turn in
    // its tail. The first version snapped the cut forward to a user turn, found
    // none, and folded nothing -- so a session of single tasks never folded at
    // all. The seam only has to keep a tool result with the call it answers.
    let mut entries = vec![user(0, "the one request of this task")];
    for i in 1..=12u64 {
        entries.push(Entry::new(i, Role::Assistant, format!("[calling 1 tool(s)] {i}")));
        entries.push(Entry::new(i + 100, Role::Tool, "x".repeat(400)));
    }
    let meter = Meter::new();
    let cut = split(&entries, &budget(3, 100), &meter).expect("a long task must be foldable");
    assert!(!cut.fold.is_empty());
    assert!(
        cut.keep[0].role != Role::Tool,
        "the kept tail began with a tool result whose call was folded away"
    );
    assert!(cut.fold.len() > 1, "nothing worth folding was folded");
}

#[rstest]
fn given_a_queue_snapshot_in_the_log_when_the_view_is_read_then_it_is_not_a_turn() {
    let entries = vec![
        Entry::new(0, Role::User, "a task"),
        Entry::new(1, Role::Queue, "{\"nodes\":{}}"),
        Entry::new(2, Role::Assistant, "an answer"),
    ];

    let shown = view(&entries);

    assert!(
        shown.iter().all(|entry| entry.role != Role::Queue),
        "the snapshot was shown to the model: {shown:?}"
    );
    assert_eq!(shown.len(), 2);
}

#[rstest]
fn given_two_topics_when_one_is_read_then_the_others_turns_are_not_on_the_wire() {
    let mut a = Entry::new(1, Role::User, "topic a: read the printer");
    a.topic = Some(String::from("a"));
    let mut b = Entry::new(2, Role::User, "topic b: read the parser");
    b.topic = Some(String::from("b"));
    let global = Entry::new(3, Role::User, "the ground everyone reads");

    let shown = view_in(&[a, b, global], Some("a"));

    let words: Vec<_> = shown.iter().map(|e| e.content.as_str()).collect();
    assert_eq!(words, ["topic a: read the printer", "the ground everyone reads"]);
}

#[rstest]
fn given_a_fold_in_one_topic_when_the_other_is_read_then_its_turns_are_still_there() {
    let mut a = Entry::new(1, Role::User, "a one");
    a.topic = Some(String::from("a"));
    let mut a2 = Entry::new(2, Role::Assistant, "a two");
    a2.topic = Some(String::from("a"));
    let mut b = Entry::new(3, Role::User, "b one");
    b.topic = Some(String::from("b"));
    // The summary covers the sequence numbers both topics used.
    let mut summary = Entry::compaction(4, "a, folded", Coverage { first: 1, last: 3 });
    summary.topic = Some(String::from("a"));

    let shown = view_in(&[a, a2, b, summary], Some("b"));

    let words: Vec<_> = shown.iter().map(|e| e.content.as_str()).collect();
    assert_eq!(words, ["b one"], "the other topic's fold swallowed this one: {words:?}");
}
