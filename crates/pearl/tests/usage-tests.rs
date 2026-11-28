//! Stories for `pearl --usage`: what a log of presses says back.
//!
//! The report is for a human asking whether the screen names the keys a hand
//! reaches for. So it owes the other half too: the keys the screen offers that
//! nobody pressed, the presses that land on nothing it names, and the lines
//! this reader could not read -- a key called unpressed because its
//! line was torn is the same lie as a key called absent.

use std::path::Path;

use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Given a fresh directory for a log file.
#[fixture]
fn dir() -> TempDir {
    TempDir::new().unwrap()
}

/// A usage entry as a launch that counted those presses wrote it.
fn usage_entry(path: &Path, presses: &[(&str, u64)]) {
    let counted: std::collections::BTreeMap<&str, u64> = presses.iter().copied().collect();
    let entry = serde_json::json!({
        "seq": 0,
        "role": "usage",
        "content": serde_json::to_string(&counted).unwrap(),
    });
    std::fs::write(path, format!("{entry}\n")).unwrap();
}

/// What `pearl --usage` prints for a log, as the human sees it.
fn report(path: &Path) -> String {
    // The row the report measures against is the one the config names, so the
    // story keeps a config of its own: a machine set to vim would otherwise
    // decide what this story is about.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pearl"))
        .arg("--usage")
        .arg("--log")
        .arg(path)
        .env("PEARL_CONFIG", path.with_file_name("config"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "usage left with {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[rstest]
fn given_a_log_where_a_key_was_pressed_when_usage_reports_then_the_keys_it_never_saw_are_named(
    dir: TempDir,
) {
    let path = dir.path().join("pearl-log.jsonl");
    usage_entry(&path, &[("a", 3)]);

    let printed = report(&path);

    assert!(
        printed.contains("never pressed, among the keys the screen names"),
        "the report named no unpressed key:\n{printed}"
    );
    // `a` shares the start group with `A`, and `A` is the one never pressed.
    assert!(
        printed.lines().any(|line| line == "  start   A"),
        "`A` was not the key the report called unpressed:\n{printed}"
    );
}

#[rstest]
fn given_a_log_where_no_window_key_was_pressed_when_usage_reports_then_the_windows_keys_are_named_too(
    dir: TempDir,
) {
    let path = dir.path().join("pearl-log.jsonl");
    usage_entry(&path, &[("a", 1)]);

    let printed = report(&path);

    // The queue's page keys and the report's shifted arrows are on the screen's
    // own rules, and the report owes them the same sentence as the row's keys.
    for name in ["PgUp", "M-⇧"] {
        assert!(
            printed.lines().any(|line| line.contains(name)),
            "`{name}` was offered on a window and the report never named it:\n{printed}"
        );
    }
}

#[rstest]
fn given_a_press_no_row_key_names_when_usage_reports_then_it_is_named_apart(dir: TempDir) {
    let path = dir.path().join("pearl-log.jsonl");
    usage_entry(&path, &[("a", 1), ("'", 2)]);

    let printed = report(&path);

    assert!(
        printed.contains("pressed, not a key the screen names"),
        "a press that answers nothing was not named apart:\n{printed}"
    );
    assert!(
        printed.lines().any(|line| line.ends_with("'")),
        "`'` was not the press the report put outside the row:\n{printed}"
    );
}

#[rstest]
fn given_a_line_that_cannot_be_read_when_usage_reports_then_the_count_is_said(dir: TempDir) {
    let path = dir.path().join("pearl-log.jsonl");
    usage_entry(&path, &[("a", 1)]);
    let mut torn = std::fs::read_to_string(&path).unwrap();
    torn.push_str("this line is half a record\n");
    std::fs::write(&path, torn).unwrap();

    let printed = report(&path);

    assert!(
        printed.contains("1 line(s) could not be read"),
        "a line nobody read went unsaid:\n{printed}"
    );
}
