//! wire --- how an intent becomes the fields a house actually reads.
//!
//! The human picks a rung on one ladder. Every house spells thinking
//! differently, and a rung spelled in the wrong dialect is not a weaker
//! setting -- it is a field the endpoint ignores or refuses. This is the only
//! place that translation lives, so the ladder and the request cannot
//! disagree about what a word means.

use pearl_core::dialect::Dialect;
use pearl_core::settings::Effort;
use serde_json::{Map, Value, json};

/// The word for `reasoning_effort`, when this dialect uses that field at all.
pub fn effort_word(dialect: Dialect, effort: Option<Effort>) -> Option<String> {
    if dialect != Dialect::OpenAi {
        return None;
    }
    effort.map(|e| match e {
        Effort::Off => String::from("none"),
        other => String::from(other.as_str()),
    })
}

/// The extra body fields this dialect needs. `None` means the request needs
/// nothing beyond what it already carries -- which is what `auto` always
/// means, and what a dialect with no rungs means at every rung.
pub fn thinking_fields(dialect: Dialect, effort: Option<Effort>) -> Option<Map<String, Value>> {
    let e = effort?;
    let mut m = Map::new();
    match dialect {
        // `reasoning_effort` carries it; nothing else to say.
        Dialect::OpenAi => return None,
        // The model id decides. Sending a field would be a guess about a
        // house that has told us it does not take one.
        Dialect::DeepSeek => return None,
        Dialect::Qwen => {
            if e == Effort::Off {
                m.insert(String::from("enable_thinking"), json!(false));
            } else {
                m.insert(String::from("enable_thinking"), json!(true));
                m.insert(String::from("thinking_budget"), json!(budget(e)));
            }
        }
        Dialect::Glm => {
            let kind = if e == Effort::Off { "disabled" } else { "enabled" };
            m.insert(String::from("thinking"), json!({ "type": kind }));
        }
        Dialect::Anthropic => {
            if e != Effort::Off {
                m.insert(
                    String::from("thinking"),
                    json!({ "type": "enabled", "budget_tokens": budget(e) }),
                );
            }
        }
    }
    if m.is_empty() { None } else { Some(m) }
}

/// A token budget for a rung, for the houses that ask for a number rather
/// than a word. The numbers are round and generous rather than tuned: they
/// are a starting point a house's own limits will clip, not a claim about
/// how much thinking a task needs.
fn budget(e: Effort) -> u32 {
    match e {
        Effort::Off => 0,
        Effort::Minimal => 1_024,
        Effort::Low => 4_096,
        Effort::Medium => 8_192,
        Effort::High => 16_384,
        Effort::Max => 32_768,
    }
}
