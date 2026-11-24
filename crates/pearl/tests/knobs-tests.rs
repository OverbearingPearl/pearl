//! Stories for the knobs a process starts with.
//!
//! A one-shot run and the surface answer the same question, so they have to
//! read the same file. A run that built its own defaults would ignore every
//! choice the human had kept.

use rstest::rstest;

#[rstest]
fn given_a_kept_config_when_a_run_builds_its_knobs_then_the_stated_dialect_and_rung_are_in_force() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config");
    std::fs::write(&path, "dialect openai\neffort high\nwindow 200000\n").unwrap();
    let settings = pearl::knobs_at("m", "openai", &path);

    assert_eq!(settings.dialect(), pearl_core::Dialect::OpenAi);
    assert_eq!(settings.effort().map(|e| e.as_str()), Some("high"));
    assert_eq!(settings.window(), 200_000, "the run would fold at a point nobody chose");
}
