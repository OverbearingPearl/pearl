//! Stories for configuration that takes effect while the agent is running.
//!
//! A knob you can only set before launch is not a knob: the human notices the
//! model is wrong mid-task, and restarting throws the session away. The surface
//! and the runner hold the same settings, so an edit is felt by the next turn.

use pearl_core::{Effort, Settings};
use rstest::rstest;

#[rstest]
fn given_a_shared_settings_when_one_holder_changes_the_model_then_the_other_sees_it() {
    let settings = Settings::new("deepseek-chat");
    let runner = settings.clone();
    settings.set_model("deepseek-reasoner");
    assert_eq!(runner.model(), "deepseek-reasoner");
}

#[rstest]
fn given_a_settings_line_when_the_model_is_named_then_it_changes_and_says_so() {
    let settings = Settings::new("deepseek-chat");
    let said = settings.apply("model deepseek-reasoner").unwrap();
    assert_eq!(settings.model(), "deepseek-reasoner");
    assert!(said.contains("deepseek-reasoner"), "{said}");
}

#[rstest]
fn given_a_settings_line_when_the_effort_is_named_then_it_changes() {
    let settings = Settings::new("m");
    settings.apply("effort high").unwrap();
    assert_eq!(settings.effort(), Some(Effort::High));
    settings.apply("effort off").unwrap();
    assert_eq!(settings.effort(), None);
}

#[rstest]
fn given_a_settings_line_when_the_window_is_named_then_the_threshold_moves_with_it() {
    let settings = Settings::new("m");
    settings.apply("window 64000").unwrap();
    assert_eq!(settings.window(), 64_000);
    assert_eq!(settings.fold_at(), 48_000, "three quarters of the new window");
}

#[rstest]
fn given_a_settings_line_when_the_trigger_is_named_then_the_fold_point_moves() {
    let settings = Settings::new("m");
    settings.apply("trigger 0.5").unwrap();
    assert_eq!(settings.fold_at(), 64_000);
}

#[rstest]
#[case("effort enormous")]
#[case("window wide")]
#[case("window 0")]
#[case("trigger 2")]
#[case("trigger 0")]
#[case("colour blue")]
#[case("")]
fn given_a_settings_line_that_does_not_parse_when_it_is_applied_then_it_says_why(
    #[case] line: &str,
) {
    let settings = Settings::new("m");
    let err = settings.apply(line).unwrap_err();
    assert!(!err.is_empty(), "a refused setting said nothing");
    assert_eq!(settings.model(), "m", "a refused line changed something");
}

#[rstest]
fn given_a_settings_when_it_is_described_then_the_line_names_what_is_in_force() {
    let settings = Settings::new("deepseek-chat");
    settings.apply("effort low").unwrap();
    let line = settings.describe();
    assert!(line.contains("deepseek-chat"), "{line}");
    assert!(line.contains("low"), "{line}");
    assert!(line.contains("128000") || line.contains("128k"), "{line}");
}
