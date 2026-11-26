//! dialect --- what a house calls thinking, and which rungs it has.
//!
//! The human picks an intent on one ladder; the wire wants different fields
//! from every house. The ladder itself is one vocabulary -- `Effort::LADDER`
//! -- so a rung means the same thing wherever it is read, and a dialect says
//! which part of it it can put on the wire. The list shows the whole ladder
//! and marks what this endpoint cannot say, because a menu that changes shape
//! with the endpoint teaches nothing about the endpoint.

use crate::settings::Effort;

/// How a house spells thinking on an OpenAI-compatible request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `reasoning_effort`: the field the compatibility layer is named for.
    OpenAi,
    /// No field at all: the model, or the server's own default, decides.
    /// DeepSeek is the house that works this way by design; an endpoint that
    /// refuses every thinking field ends up here too, by being told so.
    Silent,
    /// `enable_thinking` plus a token budget.
    Qwen,
    /// `thinking: {type: enabled|disabled}`, with no strength.
    Glm,
    /// `thinking: {type: enabled, budget_tokens: N}`.
    Anthropic,
}

impl Dialect {
    /// The dialect a house is known to speak. An unknown house gets the one
    /// the compatibility layer is named for, which is the best guess available
    /// and the one a refusal can correct.
    pub fn for_house(house: &str) -> Self {
        match house {
            "deepseek" => Dialect::Silent,
            "dashscope" | "aliyun" | "qwen" => Dialect::Qwen,
            "bigmodel" | "zhipu" | "glm" => Dialect::Glm,
            "anthropic" => Dialect::Anthropic,
            _ => Dialect::OpenAi,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Dialect::OpenAi => "openai",
            Dialect::Silent => "silent",
            Dialect::Qwen => "qwen",
            Dialect::Glm => "glm",
            Dialect::Anthropic => "anthropic",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "openai" => Some(Dialect::OpenAi),
            "silent" | "deepseek" => Some(Dialect::Silent),
            "qwen" => Some(Dialect::Qwen),
            "glm" => Some(Dialect::Glm),
            "anthropic" => Some(Dialect::Anthropic),
            _ => None,
        }
    }

    /// Whether this dialect can put the rung on the wire at all.
    pub fn says(self, rung: Effort) -> bool {
        self.rungs().contains(&rung)
    }

    /// The rungs this dialect can express, weakest first. `auto` -- send
    /// nothing and let the endpoint decide -- is always available and is not
    /// listed here. A rung absent from this list is not offered, because a
    /// setting the endpoint cannot honour is a setting that lies.
    pub fn rungs(self) -> &'static [Effort] {
        match self {
            Dialect::OpenAi => &[
                Effort::Off,
                Effort::Minimal,
                Effort::Low,
                Effort::Medium,
                Effort::High,
                Effort::XHigh,
            ],
            Dialect::Silent => &[],
            // The budget houses take a number, so every rung has somewhere to
            // land: the same seven words mean the same seven strengths.
            Dialect::Qwen => &Effort::LADDER,
            Dialect::Glm => &[Effort::Off],
            Dialect::Anthropic => &Effort::LADDER,
        }
    }

    /// What this endpoint can be told about thinking, in words a reader who
    /// has never seen the code can act on. It is a sentence, not a field list:
    /// what is on the wire is our business, and what the endpoint accepts is
    /// the human's.
    pub fn describe(self) -> String {
        match self {
            Dialect::Silent => {
                String::from("this endpoint has no thinking setting, so the model decides")
            }
            Dialect::Glm => {
                String::from("this endpoint has thinking on or off, with nothing in between")
            }
            Dialect::OpenAi => String::from(
                "this endpoint takes a word for thinking: none, minimal, low, medium, high or xhigh",
            ),
            Dialect::Qwen | Dialect::Anthropic => {
                String::from("this endpoint takes a token budget, so every rung below fits")
            }
        }
    }
}
