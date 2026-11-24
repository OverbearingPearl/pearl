//! types --- the request and response shapes on the wire.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The role of a chat message, as the OpenAI-compatible wire names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A function the model may call: its name and JSON-encoded arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub arguments: String,
}

/// A tool call the model asks for. The `type` is always `function` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_type")]
    pub kind: String,
    pub function: Function,
}

fn function_type() -> String {
    "function".to_string()
}

/// A tool offered to the model: a name, a description, and a JSON schema.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    #[serde(rename = "type", default = "function_type")]
    pub kind: String,
    pub function: FunctionSpec,
}

/// The function half of a tool spec.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            kind: function_type(),
            function: FunctionSpec { name: name.into(), description: description.into(), parameters },
        }
    }
}

/// One chat message. An assistant turn may carry tool calls instead of text; a
/// tool turn carries the result of one call, keyed by its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self { role, content: content.into(), tool_calls: Vec::new(), tool_call_id: None }
    }

    /// An assistant turn that asks for tool calls.
    pub fn assistant_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self { role: Role::Assistant, content: String::new(), tool_calls, tool_call_id: None }
    }

    /// A tool turn carrying one call's result.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: Role::Tool, content: content.into(), tool_calls: Vec::new(), tool_call_id: Some(tool_call_id.into()) }
    }
}

/// A completion request. The model is chosen per request, not stored on the
/// client, so a task can pick its own model.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
    /// How hard to think, when the endpoint has such a knob. Absent means the
    /// endpoint decides, which is what an endpoint without the knob expects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

/// What a request cost, as the provider billed it. `prompt_tokens` is the
/// metered size of everything sent, which is the only honest measure of how
/// full the context is; an estimate is a guess beside it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// A finished completion: text, or tool calls the loop must execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub content: String,
    pub model: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

impl Completion {
    /// A completion with no tool calls and no billed usage recorded.
    pub fn text(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            tool_calls: Vec::new(),
            usage: Usage::default(),
        }
    }

    /// Whether the model is asking for tool calls rather than finishing.
    pub fn wants_tools(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// The subset of the response JSON we read. Kept separate from the public
/// `Completion` so the wire shape can be tested without a network.
#[derive(Debug, Deserialize)]
pub(crate) struct WireResponse {
    pub model: String,
    pub choices: Vec<WireChoice>,
    #[serde(default)]
    pub usage: Option<WireUsage>,
}

/// The `usage` object as the OpenAI-compatible wire spells it. Servers that
/// omit it leave the loop on estimates, which is why it is optional.
#[derive(Debug, Deserialize)]
pub(crate) struct WireUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WireChoice {
    pub message: WireMessage,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WireMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl WireResponse {
    pub fn into_completion(self) -> Option<Completion> {
        let choice = self.choices.into_iter().next()?;
        let usage = self.usage.map_or_else(Usage::default, |u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
        });
        Some(Completion {
            content: choice.message.content.unwrap_or_default(),
            model: self.model,
            tool_calls: choice.message.tool_calls.unwrap_or_default(),
            usage,
        })
    }
}
