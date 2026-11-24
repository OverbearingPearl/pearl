//! Stories for the thinking the endpoint sends back.
//!
//! An effort knob whose entire output is discarded is not a knob. DeepSeek
//! spells the thinking `reasoning_content`; it arrived and was dropped.

use pearl_provider::Completion;
use rstest::rstest;

fn parse(body: &str) -> Completion {
    pearl_provider::openai::parse_response(body).expect("parsed")
}

#[rstest]
fn given_a_reasoning_reply_when_it_is_parsed_then_the_thinking_is_kept() {
    let body = r#"{"model":"deepseek-reasoner","choices":[{"message":
        {"reasoning_content":"weighing the options","content":"the answer"}}]}"#;
    let c = parse(body);
    assert_eq!(c.reasoning.as_deref(), Some("weighing the options"));
    assert_eq!(c.content, "the answer");
}

#[rstest]
fn given_a_plain_reply_when_it_is_parsed_then_there_is_no_thinking() {
    let body = r#"{"model":"m","choices":[{"message":{"content":"hi"}}]}"#;
    assert!(parse(body).reasoning.is_none());
}

#[rstest]
fn given_a_server_that_spells_it_reasoning_when_parsed_then_it_is_kept_too() {
    let body = r#"{"model":"m","choices":[{"message":
        {"reasoning":"other spelling","content":"ok"}}]}"#;
    assert_eq!(parse(body).reasoning.as_deref(), Some("other spelling"));
}
