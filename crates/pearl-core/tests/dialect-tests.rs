//! Stories for what a house can say about thinking.
//!
//! One ladder for the human, a dialect per house on the wire. A rung the
//! endpoint cannot express is not offered: a setting that is silently ignored
//! is worse than one that is refused, because the human believes it took.

use pearl_core::{Dialect, Effort, Settings};
use rstest::rstest;

#[rstest]
fn given_the_ladder_when_it_is_read_then_it_names_every_intent_a_house_may_have() {
    for (word, want) in [
        ("off", Effort::Off),
        ("minimal", Effort::Minimal),
        ("low", Effort::Low),
        ("medium", Effort::Medium),
        ("high", Effort::High),
        ("max", Effort::Max),
    ] {
        assert_eq!(Effort::parse(word), Some(want), "{word} is not on the ladder");
        assert_eq!(want.as_str(), word);
    }
}

#[rstest]
fn given_a_house_when_its_dialect_is_looked_up_then_it_speaks_its_own_fields() {
    assert_eq!(Dialect::for_house("deepseek"), Dialect::DeepSeek);
    assert_eq!(Dialect::for_house("dashscope"), Dialect::Qwen);
    assert_eq!(Dialect::for_house("bigmodel"), Dialect::Glm);
    assert_eq!(Dialect::for_house("anthropic"), Dialect::Anthropic);
    assert_eq!(Dialect::for_house("openai"), Dialect::OpenAi);
    assert_eq!(Dialect::for_house("localhost"), Dialect::OpenAi);
}

#[rstest]
fn given_deepseek_when_its_rungs_are_read_then_there_are_none_because_the_model_decides() {
    assert!(Dialect::DeepSeek.rungs().is_empty());
    assert!(Dialect::DeepSeek.describe().contains("the model decides"));
}

#[rstest]
fn given_an_openai_dialect_when_max_is_asked_for_then_it_is_refused_rather_than_ignored() {
    let s = Settings::new("gpt-4o");
    let said = s.apply("effort max").unwrap_err();
    assert!(said.contains("max"), "the refusal does not name the rung: {said}");
    assert!(s.effort().is_none(), "a refused rung was taken anyway");
}

#[rstest]
fn given_deepseek_when_the_effort_key_is_pressed_then_it_stays_auto_and_says_why() {
    let s = Settings::new("deepseek-reasoner");
    for _ in 0..4 {
        s.cycle_effort();
        assert!(s.effort().is_none(), "a house with no effort knob was given one");
    }
}

#[rstest]
fn given_a_qwen_dialect_when_the_effort_key_walks_then_it_reaches_the_rungs_qwen_has() {
    let s = Settings::new("qwen3");
    s.apply("add https://dashscope.aliyuncs.com/compatible-mode/v1 qwen3").unwrap();
    s.apply("choose 2").unwrap();
    let mut seen = vec![];
    for _ in 0..6 {
        s.cycle_effort();
        seen.push(s.effort().map(|e| e.as_str()));
    }
    assert!(seen.contains(&Some("max")), "qwen never reached max: {seen:?}");
    assert!(seen.contains(&Some("off")), "qwen never reached off: {seen:?}");
    assert!(!seen.contains(&Some("minimal")), "qwen was offered a rung it cannot say: {seen:?}");
}

#[rstest]
fn given_a_dialect_set_by_hand_when_it_is_read_then_it_overrides_the_house() {
    let s = Settings::new("m");
    s.apply("dialect qwen").unwrap();
    assert_eq!(s.dialect(), Dialect::Qwen);
    s.apply("effort max").unwrap();
    assert_eq!(s.effort(), Some(Effort::Max));
}
