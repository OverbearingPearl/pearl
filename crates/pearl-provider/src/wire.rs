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

/// How much of a ladder one refusal takes with it.
///
/// The distinction is the whole difference between learning one thing and
/// learning five: an endpoint that does not know the field has rejected every
/// rung at once, and retrying `medium` after `high` was refused only earns
/// the same 400 in another spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// The field itself is not known here: no rung can be sent.
    Field,
    /// The field is understood and this value is not: one rung is gone.
    Rung,
}

/// Read a refusal for what it actually rejected. The narrow reading is the
/// default -- an unrecognised sentence is one rung's fault, not a ladder's --
/// because being wrong narrowly costs one more round trip and being wrong
/// widely throws away a setting the endpoint would have honoured.
pub fn refusal_width(message: &str) -> Width {
    let said = message.to_lowercase();
    const WHOLE_FIELD: [&str; 6] = [
        "unsupported parameter",
        "unknown parameter",
        "unrecognized",
        "unrecognised",
        "extra inputs",
        "not permitted",
    ];
    if WHOLE_FIELD.iter().any(|w| said.contains(w)) {
        Width::Field
    } else {
        Width::Rung
    }
}

/// The extra body fields this dialect needs. `None` means the request needs
/// nothing beyond what it already carries -- which is what `auto` always
/// means, and what a dialect with no rungs means at every rung.
pub fn thinking_fields(
    dialect: Dialect,
    effort: Option<Effort>,
    window: u32,
) -> Option<Map<String, Value>> {
    let e = effort?;
    let mut m = Map::new();
    match dialect {
        // `reasoning_effort` carries it; nothing else to say.
        Dialect::OpenAi => return None,
        // The model id decides. Sending a field would be a guess about a
        // house that has told us it does not take one.
        Dialect::Silent => return None,
        Dialect::Qwen => {
            if e == Effort::Off {
                m.insert(String::from("enable_thinking"), json!(false));
            } else {
                m.insert(String::from("enable_thinking"), json!(true));
                m.insert(String::from("thinking_budget"), json!(budget(e)));
            }
        }
        Dialect::Glm => {
            let kind = if e == Effort::Off {
                "disabled"
            } else {
                "enabled"
            };
            m.insert(String::from("thinking"), json!({ "type": kind }));
        }
        Dialect::Anthropic => {
            if e != Effort::Off {
                // Anthropic wants `max_tokens` on every request and a budget
                // that leaves room under it, so the ceiling is ours to
                // respect. Without one, a high rung would be refused for
                // arithmetic and filed as a model that cannot think hard --
                // the ladder blaming the endpoint for our own sum.
                let ceiling = (window / 4).max(1_024);
                let asked = budget(e).min(ceiling);
                m.insert(String::from("max_tokens"), json!(ceiling * 2));
                m.insert(
                    String::from("thinking"),
                    json!({ "type": "enabled", "budget_tokens": asked }),
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
        Effort::XHigh => 24_576,
        Effort::Max => 32_768,
    }
}
