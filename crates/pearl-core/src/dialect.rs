//! dialect --- what a house calls thinking, and which rungs it has.
//!
//! The human picks an intent on one ladder; the wire wants different fields
//! from every house. Keeping one enum and sending it verbatim would mean
//! offering rungs the endpoint cannot express and silently getting nothing,
//! so the ladder the human walks is the one this dialect can actually say.

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

    /// The rungs this dialect can express, weakest first. `auto` -- send
    /// nothing and let the endpoint decide -- is always available and is not
    /// listed here. A rung absent from this list is not offered, because a
    /// setting the endpoint cannot honour is a setting that lies.
    pub fn rungs(self) -> &'static [Effort] {
        match self {
            Dialect::OpenAi => {
                &[Effort::Off, Effort::Minimal, Effort::Low, Effort::Medium, Effort::High]
            }
            Dialect::Silent => &[],
            Dialect::Qwen => &[Effort::Off, Effort::Low, Effort::Medium, Effort::High, Effort::Max],
            Dialect::Glm => &[Effort::Off],
            Dialect::Anthropic => {
                &[Effort::Minimal, Effort::Low, Effort::Medium, Effort::High, Effort::Max]
            }
        }
    }

    /// What this dialect can say about thinking, in one line for the surface.
    pub fn describe(self) -> String {
        if self.rungs().is_empty() {
            return format!("{}: the model decides", self.as_str());
        }
        let words: Vec<&str> = self.rungs().iter().map(|e| e.as_str()).collect();
        format!("{}: auto, {}", self.as_str(), words.join(", "))
    }
}
