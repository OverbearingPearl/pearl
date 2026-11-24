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
    settings.apply("dialect openai").unwrap();
    settings.apply("effort high").unwrap();
    assert_eq!(settings.effort(), Some(Effort::High));
    settings.apply("effort off").unwrap();
    assert_eq!(settings.effort(), Some(Effort::Off), "off is a rung, not the absence of one");
    settings.apply("effort auto").unwrap();
    assert_eq!(settings.effort(), None, "auto is the absence: send nothing");
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
    settings.apply("dialect openai").unwrap();
    settings.apply("effort low").unwrap();
    let line = settings.describe();
    assert!(line.contains("deepseek-chat"), "{line}");
    assert!(line.contains("low"), "{line}");
    assert!(line.contains("128000") || line.contains("128k"), "{line}");
}

#[rstest]
fn given_a_model_when_it_is_cycled_then_it_moves_to_the_next_and_wraps() {
    let s = Settings::new("deepseek-chat");
    s.apply("add - deepseek-reasoner").unwrap();
    s.cycle_model();
    assert_eq!(s.model(), "deepseek-reasoner");
    s.cycle_model();
    assert_eq!(s.model(), "deepseek-chat");
}

#[rstest]
fn given_no_effort_when_it_is_cycled_then_it_steps_through_the_ladder_and_back_to_auto() {
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    assert!(s.effort().is_none());
    for want in ["off", "minimal", "low", "medium", "high"] {
        s.cycle_effort();
        assert_eq!(s.effort().map(|e| e.as_str()), Some(want));
    }
    s.cycle_effort();
    assert!(s.effort().is_none(), "the ladder did not return to auto");
}

#[rstest]
fn given_a_window_when_it_is_cycled_then_it_steps_through_the_ladder() {
    let s = Settings::new("m");
    let first = s.window();
    s.cycle_window();
    assert_ne!(s.window(), first);
    let mut steps = 1;
    while s.window() != first && steps < 32 {
        s.cycle_window();
        steps += 1;
    }
    assert_eq!(s.window(), first, "the window ladder did not wrap");
    assert!(steps > 4, "the ladder stopped at {steps} steps: it cannot reach a large window");
}

#[rstest]
fn given_a_model_off_the_ladder_when_it_is_cycled_then_it_joins_the_ladder() {
    let s = Settings::new("some-local-model");
    s.apply("add - another").unwrap();
    s.apply("choose 1").unwrap();
    s.cycle_model();
    assert_ne!(s.model(), "some-local-model");
}

#[rstest]
fn given_edited_knobs_when_they_are_saved_and_loaded_then_they_come_back() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config");
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.apply("model deepseek-reasoner").unwrap();
    s.apply("effort high").unwrap();
    s.apply("window 64000").unwrap();
    s.save(&path).unwrap();

    let back = Settings::new("m");
    let read = back.load(&path).unwrap();
    assert_eq!(read, 8, "not every knob came back");
    assert_eq!(back.dialect(), pearl_core::Dialect::OpenAi, "the dialect has to load before the rung it allows");
    assert_eq!(back.model(), "deepseek-reasoner");
    assert_eq!(back.effort().map(|e| e.as_str()), Some("high"));
    assert_eq!(back.window(), 64_000);
}

#[rstest]
fn given_no_config_file_when_it_is_loaded_then_the_defaults_stand() {
    let dir = tempfile::TempDir::new().unwrap();
    let s = Settings::new("deepseek-chat");
    assert_eq!(s.load(dir.path().join("absent")).unwrap(), 0);
    assert_eq!(s.model(), "deepseek-chat");
}

#[rstest]
fn given_a_saved_config_when_it_is_read_as_text_then_it_is_the_same_language_as_the_prompt() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config");
    let s = Settings::new("deepseek-chat");
    s.apply("dialect openai").unwrap();
    s.apply("effort low").unwrap();
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.lines().any(|l| l == "effort low"), "{text}");
}

#[rstest]
fn given_the_default_keyboard_when_the_style_is_cycled_then_it_goes_plain_vim_emacs() {
    use pearl_core::Keys;
    let s = Settings::new("m");
    assert_eq!(s.keys(), Keys::Plain);
    s.cycle_keys();
    assert_eq!(s.keys(), Keys::Vim);
    s.cycle_keys();
    assert_eq!(s.keys(), Keys::Emacs);
    s.cycle_keys();
    assert_eq!(s.keys(), Keys::Plain);
}

#[rstest]
fn given_a_chosen_keyboard_when_the_defaults_are_saved_and_loaded_then_it_returns() {
    use pearl_core::Keys;
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config");
    let s = Settings::new("m");
    s.apply("keys emacs").unwrap();
    s.save(&path).unwrap();
    let back = Settings::new("m");
    back.load(&path).unwrap();
    assert_eq!(back.keys(), Keys::Emacs);
}

#[rstest]
fn given_a_provider_with_its_own_models_when_the_model_is_cycled_then_it_stays_on_that_house() {
    let s = Settings::new("deepseek-chat");
    s.set_models(["gpt-4o", "gpt-4o-mini"]);
    assert_eq!(s.model(), "gpt-4o", "a model the endpoint does not have was kept");
    s.cycle_model();
    assert_eq!(s.model(), "gpt-4o-mini");
    s.cycle_model();
    assert_eq!(s.model(), "gpt-4o");
}

#[rstest]
fn given_an_unknown_endpoint_when_models_are_offered_then_the_chosen_one_is_left_alone() {
    let s = Settings::new("my-local-model");
    s.set_models(Vec::<String>::new());
    s.cycle_model();
    assert_eq!(s.model(), "my-local-model", "a local model was replaced by a guess");
}

#[test]
fn given_the_window_ladder_when_it_is_walked_then_it_reaches_the_windows_models_actually_have() {
    let s = Settings::new("m");
    let mut seen = vec![s.window()];
    for _ in 0..16 {
        s.cycle_window();
        if seen.contains(&s.window()) {
            break;
        }
        seen.push(s.window());
    }
    assert!(seen.contains(&200_000), "no 200k step for a Claude-sized window: {seen:?}");
    assert!(seen.contains(&1_000_000), "no million step for a Gemini-sized window: {seen:?}");
}
