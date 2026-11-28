//! Stories for the outputs a request carries: the newest ones whole, the
//! older ones a line saying what was there, and the log holding all of them.

use pearl_log::{Entry, Role, fold_results};
use rstest::rstest;

/// A night of shell work: `count` rounds, each asking for one command and
/// getting `bytes` of output back.
fn night(count: u64, bytes: usize) -> Vec<Entry> {
    let mut entries = Vec::new();
    for i in 0..count {
        let call = format!("c{i}");
        entries.push(Entry::asked(
            i * 2,
            format!("[{{\"id\":\"{call}\",\"name\":\"bash\"}}]"),
            "",
        ));
        entries.push(Entry::answered(
            i * 2 + 1,
            &call,
            format!("[op {i}] {}\n", "x".repeat(bytes)),
        ));
    }
    entries
}

/// The projection as the log's own shape takes it: a tool message is an entry
/// that answers a call, and its content is the output.
fn fold(entries: &mut [Entry], keep: usize, step: usize) -> usize {
    fold_results(
        entries,
        |e| e.role == Role::Tool && e.call.is_some(),
        |e| &mut e.content,
        keep,
        step,
    )
}

/// The tool outputs in a view, in order.
fn outputs(entries: &[Entry]) -> Vec<&str> {
    entries
        .iter()
        .filter(|e| e.role == Role::Tool)
        .map(|e| e.content.as_str())
        .collect()
}

#[rstest]
fn given_a_night_of_shell_work_when_the_request_is_built_then_the_older_outputs_are_a_line_and_the_newest_are_whole()
 {
    let mut entries = night(40, 2_000);
    let folded = fold(&mut entries, 12, 16);
    // Twenty-eight are older than the twelve kept; one whole group of sixteen
    // has gone by, so sixteen are a line and the rest wait for the next group.
    assert_eq!(folded, 16);
    let shown = outputs(&entries);
    assert_eq!(shown.len(), 40);
    assert!(
        shown[0].starts_with("[folded:"),
        "the oldest output was carried whole: {}",
        shown[0]
    );
    assert!(
        shown[0].contains("2008 bytes"),
        "the line says how much was there: {}",
        shown[0]
    );
    assert!(
        shown[0].contains("[op 0]"),
        "the line keeps the handle it had"
    );
    assert!(shown[15].starts_with("[folded:"));
    // Everything past the line is the work in hand and is read as it was.
    assert!(shown[16].contains(&"x".repeat(2_000)));
    assert!(shown[39].contains(&"x".repeat(2_000)));
    // Every call is still answered: a stub is a shorter answer, not a
    // missing one, and an endpoint refuses a question left open.
    for e in entries.iter().filter(|e| e.role == Role::Tool) {
        assert!(e.call.is_some());
    }
}

#[rstest]
fn given_a_request_one_output_longer_when_it_is_built_then_the_older_outputs_read_exactly_as_they_did()
 {
    // The prefix a cache is paid for has to be the same bytes twice running.
    // A fold line that moved with every round would rewrite the middle of the
    // conversation each time and make the whole tail a miss.
    let mut before = night(40, 2_000);
    fold(&mut before, 12, 16);
    let mut after = night(41, 2_000);
    fold(&mut after, 12, 16);
    let was = outputs(&before);
    let now = outputs(&after);
    for (i, old) in was.iter().enumerate() {
        assert_eq!(*old, now[i], "output {i} changed under the cache");
    }
}

#[rstest]
fn given_an_output_of_one_line_when_the_request_is_built_then_it_is_kept_whole() {
    // A stub is longer than an acknowledgement. Folding `ok` would spend
    // tokens to say less.
    let mut entries = night(40, 2_000);
    for e in entries.iter_mut().filter(|e| e.role == Role::Tool) {
        if e.content.contains("[op 0]") {
            e.content = "[op 0] ok".to_string();
        }
    }
    fold(&mut entries, 12, 16);
    assert_eq!(outputs(&entries)[0], "[op 0] ok");
}

#[rstest]
fn given_a_night_of_shell_work_when_the_log_is_read_back_then_every_output_is_still_whole() {
    // The projection is over a copy. The file is the record, and a human
    // reading it back finds what the command actually printed.
    let entries = night(40, 2_000);
    let mut shown = entries.clone();
    fold(&mut shown, 12, 16);
    assert!(outputs(&shown)[0].starts_with("[folded:"));
    assert!(outputs(&entries)[0].contains(&"x".repeat(2_000)));
}
