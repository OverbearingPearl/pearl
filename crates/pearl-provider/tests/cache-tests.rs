//! Stories for what the cache saved.
//! A cached prompt token costs a tenth of an uncached one on DeepSeek, so the
//! hit rate is the difference between a cheap conversation and an expensive one
//! -- and it was not parsed, let alone shown.

use pearl_provider::openai::parse_response;
use rstest::rstest;

fn body(usage: &str) -> String {
    format!(r#"{{"model":"m","choices":[{{"message":{{"content":"ok"}}}}],"usage":{usage}}}"#)
}

#[rstest]
fn given_deepseeks_spelling_when_usage_is_read_then_the_cached_tokens_are_kept() {
    let c = parse_response(&body(
        r#"{"prompt_tokens":1000,"completion_tokens":10,"prompt_cache_hit_tokens":768,"prompt_cache_miss_tokens":232}"#,
    ))
    .unwrap();
    assert_eq!(c.usage.cached_tokens, 768);
}

#[rstest]
fn given_openais_spelling_when_usage_is_read_then_the_cached_tokens_are_kept() {
    let c = parse_response(&body(
        r#"{"prompt_tokens":1000,"completion_tokens":10,"prompt_tokens_details":{"cached_tokens":512}}"#,
    ))
    .unwrap();
    assert_eq!(c.usage.cached_tokens, 512);
}

#[rstest]
fn given_a_server_that_says_nothing_about_caching_when_usage_is_read_then_nothing_is_claimed() {
    let c = parse_response(&body(r#"{"prompt_tokens":1000,"completion_tokens":10}"#)).unwrap();
    assert_eq!(c.usage.cached_tokens, 0);
}
