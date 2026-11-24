//! Stories for the fields the wire carries, and for how much of a ladder one
//! refusal takes with it.
//!
//! A rung on a ladder is a word the human picked; the wire wants a field, and
//! sometimes a number. Getting that arithmetic wrong blames the endpoint for
//! our own sum.

use pearl_core::dialect::Dialect;
use pearl_core::settings::Effort;
use pearl_provider::wire::{Width, refusal_width, thinking_fields};
use rstest::rstest;

#[rstest]
fn given_a_sentence_about_the_parameter_when_it_is_read_then_the_whole_ladder_is_meant() {
    let said = "Unsupported parameter: 'reasoning_effort' is not supported with this model.";
    assert_eq!(refusal_width(said), Width::Field);
}

#[rstest]
fn given_a_sentence_about_a_value_when_it_is_read_then_one_rung_is_meant() {
    let said = "Invalid value for 'reasoning_effort': 'high' is not supported. Try low, medium.";
    assert_eq!(refusal_width(said), Width::Rung);
}

#[rstest]
fn given_a_sentence_nobody_expected_when_it_is_read_then_the_narrow_reading_wins() {
    // Being wrong narrowly costs one more round trip; being wrong widely
    // throws away a setting the endpoint would have honoured.
    assert_eq!(refusal_width("400 Bad Request"), Width::Rung);
}

#[rstest]
fn given_an_anthropic_style_endpoint_when_a_rung_is_asked_for_then_the_budget_leaves_room_under_max_tokens() {
    let fields = thinking_fields(Dialect::Anthropic, Some(Effort::Max), 128_000).unwrap();
    let max = fields["max_tokens"].as_u64().unwrap();
    let budget = fields["thinking"]["budget_tokens"].as_u64().unwrap();
    assert!(max > 0, "the endpoint requires a ceiling and none was sent");
    assert!(budget * 2 <= max, "the budget would be refused by the endpoint's own rule");
    assert_eq!(fields["thinking"]["type"], "enabled");
}

#[rstest]
fn given_a_narrow_window_when_a_high_rung_is_asked_for_then_the_budget_is_clamped_rather_than_sent() {
    let fields = thinking_fields(Dialect::Anthropic, Some(Effort::Max), 8_192).unwrap();
    let max = fields["max_tokens"].as_u64().unwrap();
    let budget = fields["thinking"]["budget_tokens"].as_u64().unwrap();
    assert!(budget * 2 <= max, "{budget} does not fit under {max}");
    assert!(budget >= 1_024, "the endpoint refuses a budget below its floor: {budget}");
}

#[rstest]
fn given_auto_when_the_request_is_built_then_no_thinking_field_is_sent() {
    assert_eq!(thinking_fields(Dialect::Anthropic, None, 128_000), None);
    assert_eq!(thinking_fields(Dialect::Qwen, None, 128_000), None);
}
